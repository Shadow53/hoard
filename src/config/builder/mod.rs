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
use crate::CONFIG_FILE_STEM;
use crate::HOARDS_DIR_SLUG;

use super::Config;
use crate::hoard::PileConfig;

pub mod environment;
pub mod envtrie;
pub mod hoard;

const CONFIG_KEY: &str = "config";
const HOARDS_KEY: &str = "hoards";
const DEFAULT_CONFIG_EXT: &str = "toml";
/// The items are listed in descending order of precedence
const SUPPORTED_CONFIG_EXTS: [&str; 3] = ["toml", "yaml", "yml"];

/// Errors that can happen when using a [`Builder`].
#[derive(Debug, Error)]
pub enum Error {
    /// Error while parsing a TOML configuration file.
    #[error("failed to parse TOML configuration file: {0}")]
    DeserializeTOML(toml::de::Error),
    /// Error while parsing a YAML configuration file.
    #[error("failed to parse YAML configuration file: {0}")]
    DeserializeYAML(serde_yaml::Error),
    /// Error while reading from a configuration file.
    #[error("failed to read configuration file: {0}")]
    ReadConfig(io::Error),
    /// Error while determining whether configured environments apply.
    #[error("failed to determine current environment: {0}")]
    Environment(#[from] environment::Error),
    /// Error while determining which paths to use for configured hoards.
    #[error("failed to process hoard configuration: {0}")]
    ProcessHoard(#[from] hoard::Error),
    /// The item "config" is not allowed at the given config location.
    #[error("the name \"config\" is not allowed at: {0:?}")]
    NameConfigNotAllowed(Vec<String>),
    /// The item "config" is allowed but was not the expected type.
    #[error("expected \"config\" to be a pile config: at {0:?}")]
    ConfigWrongType(Vec<String>),
    /// The given file has no or invalid file extension
    #[error("configuration file must have file extension \".toml\", \".yaml\", or \".yml\": {0}")]
    InvalidExtension(PathBuf),
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
    #[serde(skip)]
    #[structopt(short, long)]
    force: bool,
    #[structopt(skip)]
    hoards: Option<BTreeMap<String, Hoard>>,
    #[structopt(skip)]
    #[serde(rename = "config")]
    global_config: Option<PileConfig>,
}

impl Default for Builder {
    fn default() -> Self {
        Self::new()
    }
}

impl Builder {
    /// Returns the default path for the configuration file.
    fn default_config_file() -> PathBuf {
        tracing::debug!("getting default configuration file");
        super::get_dirs()
            .config_dir()
            .join(format!("{}.{}", CONFIG_FILE_STEM, DEFAULT_CONFIG_EXT))
    }

    /// Returns the default location for storing hoards.
    fn default_hoard_root() -> PathBuf {
        tracing::debug!("getting default hoard root");
        super::get_dirs().data_dir().join(HOARDS_DIR_SLUG)
    }

    /// Create a new `Builder`.
    ///
    /// If [`build`](Builder::build) is immediately called on this, the returned
    /// [`Config`] will have all default values.
    #[must_use]
    pub fn new() -> Self {
        tracing::trace!("creating new config builder");
        Self {
            hoards: None,
            hoards_root: None,
            config_file: None,
            command: None,
            environments: None,
            exclusivity: None,
            force: false,
            global_config: None,
        }
    }

    /// Create a new [`Builder`] pre-populated with the contents of the given TOML file.
    ///
    /// # Errors
    ///
    /// Variants of [`enum@Error`] related to reading and parsing the file.
    pub fn from_file(path: &Path) -> Result<Self, Error> {
        let _span = tracing::debug_span!("config_from_file", ?path).entered();
        tracing::debug!("reading configuration");
        let s = std::fs::read_to_string(path).map_err(Error::ReadConfig)?;
        // Necessary because Deserialize on enums erases any errors returned by each variant.
        let result = match path.extension().and_then(std::ffi::OsStr::to_str) {
            None => Err(Error::InvalidExtension(path.to_owned())),
            Some(ext) => match ext {
                "toml" | "TOML" => toml::from_str(&s).map_err(Error::DeserializeTOML),
                "yaml" | "yml" | "YAML" | "YML" => {
                    serde_yaml::from_str(&s).map_err(Error::DeserializeYAML)
                }
                _ => Err(Error::InvalidExtension(path.to_owned())),
            },
        };

        if let Err(err) = &result {
            tracing::error!("failed to read config from file: {}", err);
        }

        result
    }

    /// Reads configuration from the default configuration file.
    ///
    /// Prefers a TOML file, if found, falling back to YAML if present.
    ///
    /// # Errors
    ///
    /// - Any errors from attempting to parse the file.
    /// - A custom not found error if no default file is found.
    pub fn from_default_file() -> Result<Self, Error> {
        SUPPORTED_CONFIG_EXTS
            .iter()
            .find_map(|suffix| {
                let path = PathBuf::from(format!("{}.{}", CONFIG_FILE_STEM, suffix));
                let path = Self::default_config_file()
                    .parent()
                    .expect("default config file should always have a file name")
                    .join(path);
                match Self::from_file(&path) {
                    Err(Error::ReadConfig(err)) => {
                        if let io::ErrorKind::NotFound = err.kind() {
                            None
                        } else {
                            Some(Err(Error::ReadConfig(err)))
                        }
                    }
                    Ok(config) => Some(Ok(config)),
                    Err(err) => Some(Err(err)),
                }
            })
            .ok_or_else(|| {
                let path = PathBuf::from(CONFIG_FILE_STEM);
                Error::ReadConfig(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "could not find any of {name}.toml, {name}.yaml, or {name}.yml in {dir}",
                        name = path
                            .file_stem()
                            .expect("default config should always have a file name")
                            .to_string_lossy(),
                        dir = path
                            .parent()
                            .expect("default config should always have a parent")
                            .to_string_lossy()
                    ),
                ))
            })?
    }

    /// Helper method to process command-line arguments and the config file specified on CLI
    /// (or the default).
    ///
    /// # Errors
    ///
    /// See [`Builder::from_file`]
    pub fn from_args_then_file() -> Result<Self, Error> {
        tracing::debug!("loading configuration from cli arguments");
        let from_args = Self::from_args();

        tracing::trace!("attempting to get configuration file from cli arguments or use default");
        let from_file =
            from_args
                .config_file
                .as_ref()
                .map_or_else(Self::from_default_file, |config_file| {
                    tracing::trace!(
                        ?config_file,
                        "configuration file is \"{}\"",
                        config_file.to_string_lossy()
                    );

                    Self::from_file(config_file)
                })?;

        tracing::debug!("merging configuration file and cli arguments");
        Ok(from_file.layer(from_args))
    }

    /// Applies all configured values in `other` over those in *this* `ConfigBuilder`.
    #[must_use]
    pub fn layer(mut self, other: Self) -> Self {
        let _span = tracing::trace_span!(
            "layering_config_builders",
            top_layer = ?other,
            bottom_layer = ?self
        )
        .entered();

        if let Some(path) = other.hoards_root {
            self = self.set_hoards_root(path);
        }

        if let Some(path) = other.config_file {
            self = self.set_config_file(path);
        }

        if let Some(path) = other.command {
            self = self.set_command(path);
        }

        self.force = self.force || other.force;

        self
    }

    /// Set the hoards map.
    #[must_use]
    pub fn set_hoards(mut self, hoards: BTreeMap<String, Hoard>) -> Self {
        tracing::trace!(?hoards, "setting hoards");
        self.hoards = Some(hoards);
        self
    }

    /// Set the directory that will contain all game save data.
    #[must_use]
    pub fn set_hoards_root(mut self, path: PathBuf) -> Self {
        // grcov: ignore-start
        tracing::trace!(
            hoards_root = ?path,
            "setting hoards root",
        );
        // grcov: ignore-end
        self.hoards_root = Some(path);
        self
    }

    #[must_use]
    pub fn set_environments(mut self, environments: BTreeMap<String, Environment>) -> Self {
        // grcov: ignore-start
        tracing::trace!(?environments, "setting environments");
        // grcov: ignore-end
        self.environments = Some(environments);
        self
    }

    /// Set the file that contains configuration.
    ///
    /// This currently only exists for completeness. You probably want [`Builder::from_file`]
    /// instead, which will actually read and parse the file.
    #[must_use]
    pub fn set_config_file(mut self, path: PathBuf) -> Self {
        // grcov: ignore-start
        tracing::trace!(
            config_file = ?path,
            "setting config file",
        );
        // grcov: ignore-end
        self.config_file = Some(path);
        self
    }

    /// Set the command that will be run.
    #[must_use]
    pub fn set_command(mut self, cmd: Command) -> Self {
        tracing::trace!(command = ?cmd, "setting command");
        self.command = Some(cmd);
        self
    }

    /// Unset the hoards map
    #[must_use]
    pub fn unset_hoards(mut self) -> Self {
        tracing::trace!("unsetting hoards");
        self.hoards = None;
        self
    }

    /// Evaluates the stored environment definitions and returns a mapping of
    /// environment name to (boolean) whether that environment applies.
    ///
    /// # Errors
    ///
    /// Any error that occurs while evaluating the environments.
    fn evaluated_environments(&self) -> Result<BTreeMap<String, bool>, Error> {
        let _span = tracing::trace_span!("eval_env").entered();
        if let Some(envs) = &self.environments {
            if envs.contains_key(CONFIG_KEY) {
                return Err(Error::NameConfigNotAllowed(vec![
                    String::from("envs"),
                    CONFIG_KEY.to_string(),
                ]));
            }

            for (key, env) in envs {
                tracing::trace!(%key, %env);
            }
        }

        self.environments
            .as_ref()
            .map_or_else(
                || Ok(BTreeMap::new()),
                |map| {
                    map.iter()
                        .map(|(key, env)| Ok((key.clone(), env.clone().try_into()?)))
                        .collect()
                },
            )
            .map_err(Error::Environment)
    }

    /// Build this [`Builder`] into a [`Config`].
    ///
    /// # Errors
    ///
    /// Any [`enum@Error`] that occurs while evaluating environment or hoard definitions.
    pub fn build(mut self) -> Result<Config, Error> {
        tracing::debug!("building configuration from builder");
        let environments = self.evaluated_environments()?;
        tracing::debug!(?environments);
        let exclusivity = self.exclusivity.unwrap_or_default();
        tracing::debug!(?exclusivity);
        let hoards_root = self.hoards_root.unwrap_or_else(Self::default_hoard_root);
        tracing::debug!(?hoards_root);
        let config_file = self.config_file.unwrap_or_else(Self::default_config_file);
        tracing::debug!(?config_file);
        let command = self.command.unwrap_or_default();
        tracing::debug!(?command);
        let force = self.force;
        tracing::debug!(?force);

        if let Some(hoards) = &mut self.hoards {
            tracing::debug!("layering global config onto hoards");
            for (_, hoard) in hoards.iter_mut() {
                hoard.layer_config(self.global_config.as_ref());
            }
        }

        tracing::debug!("processing hoards...");
        let hoards = self
            .hoards
            .unwrap_or_default()
            .into_iter()
            .map(|(name, hoard)| {
                let _span = tracing::debug_span!("processing_hoard", %name).entered();

                if name == CONFIG_KEY {
                    Err(Error::NameConfigNotAllowed(vec![
                        HOARDS_KEY.to_string(),
                        CONFIG_KEY.to_string(),
                    ]))
                } else {
                    match hoard.process_with(&environments, &exclusivity) {
                        Err(Error::NameConfigNotAllowed(old_list)) => {
                            let mut list = vec![HOARDS_KEY.to_string(), name];
                            list.extend(old_list);
                            Err(Error::NameConfigNotAllowed(list))
                        }
                        result => result.map(|hoard| (name, hoard)),
                    }
                }
            })
            .collect::<Result<_, Error>>()?;
        tracing::debug!("processed hoards");

        Ok(Config {
            command,
            hoards_root,
            config_file,
            hoards,
            force,
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
                force: false,
                global_config: None,
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
                force: false,
                global_config: None,
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
                force: false,
                global_config: None,
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

    mod test_invalid_items_named_config {
        use super::super::hoard::{MultipleEntries, Pile};
        use super::*;

        #[test]
        fn test_env_named_config() {
            let builder = Builder {
                environments: Some(
                    maplit::btreemap! { String::from("config") => Environment::default() },
                ),
                ..Builder::default()
            };

            let error = builder.build().expect_err(
                "expected building to fail because an environment called \"config\" was defined",
            );

            match error {
                Error::NameConfigNotAllowed(list) => {
                    assert_eq!(list, vec![String::from("envs"), String::from("config")])
                }
                _ => panic!("expected NameConfigNotAllowed, got {:?}", error),
            }
        }

        #[test]
        fn test_hoard_named_config() {
            let builder = Builder {
                hoards: Some(maplit::btreemap! {
                    String::from("config") => Hoard::Single(Pile { config: None, items: BTreeMap::new() })
                }),
                ..Builder::default()
            };

            let error = builder.build().expect_err(
                "expected building to fail because a hoard called \"config\" was defined",
            );

            match error {
                Error::NameConfigNotAllowed(list) => {
                    assert_eq!(list, vec![String::from("hoards"), String::from("config")])
                }
                _ => panic!("expected NameConfigNotAllowed, got {:?}", error),
            }
        }

        #[test]
        fn test_pile_named_config() {
            let builder = Builder {
                hoards: Some(maplit::btreemap! {
                    String::from("invalid_pile") => Hoard::Multiple(MultipleEntries {
                        config: None,
                        items: maplit::btreemap! { String::from("config") => Pile { config: None, items: BTreeMap::new() }}
                    })
                }),
                ..Builder::default()
            };

            let error = builder.build().expect_err(
                "expected building to fail because a hoard called \"config\" was defined",
            );

            match error {
                Error::NameConfigNotAllowed(list) => assert_eq!(
                    list,
                    vec![
                        String::from("hoards"),
                        String::from("invalid_pile"),
                        String::from("config")
                    ]
                ),
                _ => panic!("expected NameConfigNotAllowed, got {:?}", error),
            }
        }
    }
}
