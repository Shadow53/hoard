//! The [`Builder`] struct serves as an intermediate step between raw configuration and the
//! [`Config`] type that is used by `hoard`.
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use thiserror::Error;

use self::hoard::Hoard;
use environment::Environment;

use crate::command::Command;
use crate::CONFIG_FILE_NAME;
use crate::HOARDS_DIR_SLUG;

use super::Config;

pub mod environment;
pub mod envtrie;
pub mod hoard;

/// Errors that can happen when using a [`Builder`].
#[derive(Debug, Error)]
pub enum Error {
    /// Error while parsing a TOML configuration file.
    #[error("failed to parse configuration file: {0}")]
    DeserializeConfig(toml::de::Error),
    /// Error while reading from a configuration file.
    #[error("failed to read configuration file: {0}")]
    ReadConfig(io::Error),
    /// Error while determining whether configured environments apply.
    #[error("failed to determine current environment: {0}")]
    Environment(#[from] environment::Error),
    /// Error while determining which paths to use for configured hoards.
    #[error("failed to process hoard configuration: {0}")]
    ProcessHoard(#[from] hoard::Error),
}

/// Intermediate data structure to build a [`Config`](crate::config::Config).
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, StructOpt)]
#[structopt(rename_all = "kebab")]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub struct Builder {
    #[structopt(skip)]
    #[serde(rename = "envs")]
    environments: Option<BTreeMap<String, Environment>>,
    #[structopt(skip)]
    exclusivity: Option<Vec<Vec<String>>>,
    #[structopt(short, long)]
    hoards_root: Option<PathBuf>,
    #[structopt(short, long)]
    #[serde(skip)]
    config_file: Option<PathBuf>,
    #[serde(skip)]
    #[structopt(subcommand)]
    command: Option<Command>,
    #[structopt(skip)]
    hoards: Option<BTreeMap<String, Hoard>>,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    /// Returns the default path for the configuration file.
    fn default_config_file() -> PathBuf {
        super::get_dirs().config_dir().join(CONFIG_FILE_NAME)
    }

    /// Returns the default location for storing hoards.
    fn default_hoard_root() -> PathBuf {
        super::get_dirs().data_dir().join(HOARDS_DIR_SLUG)
    }

    /// Create a new `Builder`.
    ///
    /// If [`build`](Builder::build) is immediately called on this, the returned
    /// [`Config`] will have all default values.
    #[must_use]
    pub fn new() -> Self {
        tracing::trace!("Creating new config builder");
        Self {
            hoards: None,
            hoards_root: None,
            config_file: None,
            command: None,
            environments: None,
            exclusivity: None,
        }
    }

    /// Create a new [`Builder`] pre-populated with the contents of the given TOML file.
    ///
    /// # Errors
    ///
    /// Variants of [`enum@Error`] related to reading and parsing the file.
    pub fn from_file(path: &Path) -> Result<Self, Error> {
        tracing::debug!("Reading configuration from \"{}\"", path.to_string_lossy());
        let s = std::fs::read_to_string(path).map_err(Error::ReadConfig)?;
        toml::from_str(&s).map_err(Error::DeserializeConfig)
    }

    /// Helper method to process command-line arguments and the config file specified on CLI
    /// (or the default).
    ///
    /// # Errors
    ///
    /// See [`Builder::from_file`]
    pub fn from_args_then_file() -> Result<Self, Error> {
        tracing::debug!("Loading configuration from CLI arguments");
        let from_args = Self::from_args();

        tracing::trace!("Attempting to get configuration file from CLI arguments or use default");
        let config_file = from_args
            .config_file
            .clone()
            .unwrap_or_else(Self::default_config_file);

        tracing::trace!(
            "Configuration file is \"{}\"",
            config_file.to_string_lossy()
        );

        let from_file = Self::from_file(&config_file)?;

        tracing::debug!("Merging configuration file and CLI arguments");
        Ok(from_file.layer(from_args))
    }

    /// Applies all configured values in `other` over those in *this* `ConfigBuilder`.
    #[must_use]
    pub fn layer(mut self, other: Self) -> Self {
        if let Some(path) = other.hoards_root {
            self = self.set_hoards_root(path);
        }

        if let Some(path) = other.config_file {
            self = self.set_config_file(path);
        }

        if let Some(path) = other.command {
            self = self.set_command(path);
        }

        self
    }

    /// Set the hoards map.
    #[must_use]
    pub fn set_hoards(mut self, hoards: BTreeMap<String, Hoard>) -> Self {
        self.hoards = Some(hoards);
        self
    }

    /// Set the directory that will contain all game save data.
    #[must_use]
    pub fn set_hoards_root(mut self, path: PathBuf) -> Self {
        self.hoards_root = Some(path);
        self
    }

    /// Set the file that contains configuration.
    ///
    /// This currently only exists for completeness. You probably want [`Builder::from_file`]
    /// instead, which will actually read and parse the file.
    #[must_use]
    pub fn set_config_file(mut self, path: PathBuf) -> Self {
        self.config_file = Some(path);
        self
    }

    /// Set the command that will be run.
    #[must_use]
    pub fn set_command(mut self, cmd: Command) -> Self {
        self.command = Some(cmd);
        self
    }

    /// Unset the hoards map
    #[must_use]
    pub fn unset_hoards(mut self) -> Self {
        self.hoards = None;
        self
    }

    /// Unset the directory that will contain all game save data.
    #[must_use]
    pub fn unset_hoards_root(mut self) -> Self {
        self.hoards_root = None;
        self
    }

    /// Unset the file that contains configuration.
    #[must_use]
    pub fn unset_config_file(mut self) -> Self {
        self.config_file = None;
        self
    }

    /// Unset the command that will be run.
    #[must_use]
    pub fn unset_command(mut self) -> Self {
        self.command = None;
        self
    }

    /// Evaluates the stored environment definitions and returns a mapping of
    /// environment name to (boolean) whether that environment applies.
    ///
    /// # Errors
    ///
    /// Any error that occurs while evaluating the environments.
    fn evaluated_environments(
        &self,
    ) -> Result<BTreeMap<String, bool>, <Environment as TryInto<bool>>::Error> {
        tracing::trace!("Evaluating raw environments: {:#?}", self.environments);
        self.environments.as_ref().map_or_else(
            || Ok(BTreeMap::new()),
            |map| {
                map.iter()
                    .map(|(key, env)| Ok((key.clone(), env.clone().try_into()?)))
                    .collect()
            },
        )
    }

    /// Build this [`Builder`] into a [`Config`].
    ///
    /// # Errors
    ///
    /// Any [`enum@Error`] that occurs while evaluating environment or hoard definitions.
    pub fn build(self) -> Result<Config, Error> {
        tracing::trace!("Building configuration from {:#?}", self);
        let environments = self.evaluated_environments()?;
        tracing::trace!("--> environments: {:#?}", environments);
        let exclusivity = self.exclusivity.unwrap_or_else(Vec::new);
        tracing::trace!("--> exclusivity: {:#?}", exclusivity);
        let hoards_root = self.hoards_root.unwrap_or_else(Self::default_hoard_root);
        tracing::trace!("--> config file: {}", hoards_root.to_string_lossy());
        let config_file = self.config_file.unwrap_or_else(Self::default_config_file);
        tracing::trace!("--> config file: {}", config_file.to_string_lossy());
        let command = self.command.unwrap_or_else(Command::default);
        let hoards = self
            .hoards
            .unwrap_or_else(BTreeMap::new)
            .into_iter()
            .map(|(name, hoard)| Ok((name, hoard.process_with(&environments, &exclusivity)?)))
            .collect::<Result<_, Error>>()?;

        Ok(Config {
            command,
            hoards_root,
            config_file,
            hoards,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod builder {
        use super::*;

        fn get_default_populated_builder() -> Builder {
            Builder {
                hoards_root: Some(Builder::default_hoard_root()),
                config_file: Some(Builder::default_config_file()),
                command: Some(Command::Validate),
                environments: None,
                exclusivity: None,
                hoards: None,
            }
        }

        fn get_non_default_populated_builder() -> Builder {
            Builder {
                hoards_root: Some(PathBuf::from("/testing/saves")),
                config_file: Some(PathBuf::from("/testing/config.toml")),
                command: Some(Command::Restore {
                    hoards: vec!["test".into()],
                }),
                environments: None,
                exclusivity: None,
                hoards: None,
            }
        }

        #[test]
        fn default_builder_is_new() {
            assert_eq!(Builder::new(), Builder::default());
        }

        #[test]
        fn new_builder_is_all_none() {
            let expected = Builder {
                hoards_root: None,
                config_file: None,
                command: None,
                environments: None,
                hoards: None,
                exclusivity: None,
            };

            assert_eq!(
                expected,
                Builder::new(),
                "ConfigBuild::new() should have all None fields"
            );
        }

        #[test]
        fn layered_builder_prefers_some_over_none() {
            let some = get_default_populated_builder();
            let none = Builder::new();

            assert_ne!(some, none, "both builders cannot be identical");

            assert_eq!(
                some,
                none.clone().layer(some.clone()),
                "Some fields atop None prefers Some"
            );
            assert_eq!(
                some,
                some.clone().layer(none),
                "None fields atop Some prefers Some"
            );
        }

        #[test]
        fn layered_builder_prefers_argument_to_self() {
            let layer1 = get_default_populated_builder();
            let layer2 = get_non_default_populated_builder();

            assert_eq!(
                layer2,
                layer1.clone().layer(layer2.clone()),
                "layer() should prefer the argument"
            );
            assert_eq!(
                layer1,
                layer2.layer(layer1.clone()),
                "layer() should prefer the argument"
            );
        }

        #[test]
        fn builder_saves_root_sets_correctly() {
            let mut builder = Builder::new();
            assert_eq!(None, builder.hoards_root, "saves_root should start as None");
            let path = PathBuf::from("/testing/saves");
            builder = builder.set_hoards_root(path.clone());
            assert_eq!(
                Some(path),
                builder.hoards_root,
                "saves_root should now be set"
            );
        }

        #[test]
        fn builder_config_file_sets_correctly() {
            let mut builder = Builder::new();
            assert_eq!(
                None, builder.config_file,
                "config_file should start as None"
            );
            let path = PathBuf::from("/testing/config.toml");
            builder = builder.set_config_file(path.clone());
            assert_eq!(
                Some(path),
                builder.config_file,
                "config_file should now be set"
            );
        }

        #[test]
        fn builder_command_sets_correctly() {
            let mut builder = Builder::new();
            assert_eq!(None, builder.command, "command should start as None");
            let cmd = Command::Validate;
            builder = builder.set_command(cmd.clone());
            assert_eq!(Some(cmd), builder.command, "command should now be set");
        }

        #[test]
        fn builder_saves_root_unsets_correctly() {
            let mut builder = Builder::new();
            let path = PathBuf::from("/testing/saves");
            builder = builder.set_hoards_root(path.clone());
            assert_eq!(
                Some(path),
                builder.hoards_root,
                "saves_root should start as set"
            );
            builder = builder.unset_hoards_root();
            assert_eq!(None, builder.hoards_root, "saves_root should now be None");
        }

        #[test]
        fn builder_config_file_unsets_correctly() {
            let mut builder = Builder::new();
            let path = PathBuf::from("/testing/config.toml");
            builder = builder.set_config_file(path.clone());
            assert_eq!(
                Some(path),
                builder.config_file,
                "config_file should start as set"
            );
            builder = builder.unset_config_file();
            assert_eq!(None, builder.config_file, "config_file should now be None");
        }

        #[test]
        fn builder_command_unsets_correctly() {
            let mut builder = Builder::new();
            let cmd = Command::Validate;
            builder = builder.set_command(cmd.clone());
            assert_eq!(Some(cmd), builder.command, "command should start as set");
            builder = builder.unset_command();
            assert_eq!(None, builder.command, "command should now be None");
        }

        #[test]
        fn builder_with_nothing_set_uses_defaults() {
            // get_default_populated_builder is assumed to use all default values
            // for the purposes of this test.
            let builder = get_default_populated_builder();
            let config = Builder::new().build().expect("failed to build config");

            assert_eq!(Some(config.hoards_root), builder.hoards_root);
            assert_eq!(Some(config.config_file), builder.config_file);
            assert_eq!(Some(config.command), builder.command);
        }

        #[test]
        fn builder_with_options_set_uses_options() {
            let builder = get_non_default_populated_builder();
            let config = builder.clone().build().expect("failed to build config");

            assert_eq!(Some(config.hoards_root), builder.hoards_root);
            assert_eq!(Some(config.config_file), builder.config_file);
            assert_eq!(Some(config.command), builder.command);
        }
    }
}
