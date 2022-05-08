//! The [`Builder`] struct serves as an intermediate step between raw configuration and the
//! [`Config`] type that is used by `hoard`.
use std::collections::BTreeMap;
use std::convert::TryInto;
use std::path::{Path, PathBuf};

use clap::Parser;
use futures::TryStreamExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::{fs, io};

use environment::Environment;

use crate::command::Command;
use crate::hoard::PileConfig;
use crate::newtypes::{EnvironmentName, HoardName};
use crate::CONFIG_FILE_STEM;

use super::Config;

use self::hoard::Hoard;

pub mod environment;
pub mod envtrie;
pub mod hoard;

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
    /// The given file has no or invalid file extension
    #[error(
        "configuration file does not have file extension \".toml\", \".yaml\", or \".yml\": {0}"
    )]
    InvalidExtension(PathBuf),
}

/// Intermediate data structure to build a [`Config`](crate::config::Config).
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, Parser)]
#[clap(author, version, about, long_about = None, rename_all = "kebab")]
#[serde(rename_all = "snake_case")]
#[serde(deny_unknown_fields)]
pub struct Builder {
    #[clap(skip)]
    #[serde(rename = "envs")]
    environments: Option<BTreeMap<EnvironmentName, Environment>>,
    #[clap(skip)]
    exclusivity: Option<Vec<Vec<EnvironmentName>>>,
    #[clap(long)]
    data_dir: Option<PathBuf>,
    #[clap(long)]
    config_dir: Option<PathBuf>,
    /// Override the configuration file used.
    #[clap(short, long)]
    #[serde(skip)]
    config_file: Option<PathBuf>,
    #[serde(skip)]
    #[clap(subcommand)]
    command: Option<Command>,
    /// Force commands to run (skips consistency checks).
    #[serde(skip)]
    #[clap(short, long)]
    force: bool,
    #[clap(skip)]
    hoards: Option<BTreeMap<HoardName, Hoard>>,
    #[clap(skip)]
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
    #[tracing::instrument]
    fn default_config_file() -> PathBuf {
        tracing::debug!("getting default configuration file");
        crate::dirs::config_dir().join(format!("{}.{}", CONFIG_FILE_STEM, DEFAULT_CONFIG_EXT))
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
            config_dir: None,
            data_dir: None,
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
    #[tracing::instrument(level = "debug", name = "config_builder_from_file")]
    pub async fn from_file(path: &Path) -> Result<Self, Error> {
        tracing::debug!("reading configuration");
        let s = fs::read_to_string(path)
            .await
            .map_err(crate::map_log_error(Error::ReadConfig))?;
        // Necessary because Deserialize on enums erases any errors returned by each variant.
        match path.extension().and_then(std::ffi::OsStr::to_str) {
            None => crate::create_log_error(Error::InvalidExtension(path.to_owned())),
            Some(ext) => match ext {
                "toml" | "TOML" => toml::from_str(&s).map_err(crate::map_log_error_msg(
                    &format!("failed to parse TOML from {}", path.display()),
                    Error::DeserializeTOML,
                )),
                "yaml" | "yml" | "YAML" | "YML" => {
                    serde_yaml::from_str(&s).map_err(crate::map_log_error_msg(
                        &format!("failed to parse YAML from {}", path.display()),
                        Error::DeserializeYAML,
                    ))
                }
                _ => crate::create_log_error(Error::InvalidExtension(path.to_owned())),
            },
        }
    }

    /// Reads configuration from the default configuration file.
    ///
    /// Prefers a TOML file, if found, falling back to YAML if present.
    ///
    /// # Errors
    ///
    /// - Any errors from attempting to parse the file.
    /// - A custom not found error if no default file is found.
    #[tracing::instrument(level = "debug", name = "config_builder_from_default_file")]
    pub async fn from_default_file() -> Result<Self, Error> {
        let error_closure = || {
            let path = Self::default_config_file();
            let error = Error::ReadConfig(io::Error::new(
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
            ));
            crate::tap_log_error(&error);
            error
        };

        let parent = Self::default_config_file()
            .parent()
            .expect("default config file should always have a file name")
            .canonicalize()
            .map_err(crate::map_log_error(Error::ReadConfig))?;

        Box::pin(
            tokio_stream::iter(
                SUPPORTED_CONFIG_EXTS
                    .iter()
                    .map(|suffix| Ok((suffix, parent.clone()))),
            )
            .try_filter_map(|(suffix, parent)| async move {
                let path = PathBuf::from(format!("{}.{}", CONFIG_FILE_STEM, suffix));
                let path = parent.join(path);
                match Self::from_file(&path).await {
                    Err(Error::ReadConfig(err)) => {
                        if let io::ErrorKind::NotFound = err.kind() {
                            Ok(None)
                        } else {
                            crate::create_log_error(Error::ReadConfig(err))
                        }
                    }
                    Ok(config) => Ok(Some(config)),
                    Err(err) => crate::create_log_error(err),
                }
            }),
        )
        .try_next()
        .await?
        .ok_or_else(error_closure)
    }

    /// Helper method to process command-line arguments and the config file specified on CLI
    /// (or the default).
    ///
    /// # Errors
    ///
    /// See [`Builder::from_file`]
    #[tracing::instrument(level = "debug", name = "config_builder_from_args_then_file")]
    pub async fn from_args_then_file() -> Result<Self, Error> {
        tracing::debug!("loading configuration from cli arguments");
        let from_args = Self::parse();

        tracing::trace!("attempting to get configuration file from cli arguments or use default");
        let from_file = match from_args.config_file.as_ref() {
            Some(config_file) => {
                tracing::trace!(
                    ?config_file,
                    "configuration file is \"{}\"",
                    config_file.to_string_lossy()
                );

                Self::from_file(config_file).await?
            }
            None => Self::from_default_file().await?,
        };

        tracing::debug!("merging configuration file and cli arguments");
        Ok(from_file.layer(from_args))
    }

    /// Applies all configured values in `other` over those in *this* `ConfigBuilder`.
    #[must_use]
    #[tracing::instrument(level = "trace")]
    pub fn layer(mut self, other: Self) -> Self {
        if let Some(path) = other.config_dir {
            self = self.set_config_dir(path);
        }

        if let Some(path) = other.data_dir {
            self = self.set_data_dir(path);
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

    /// Set the config directory.
    #[must_use]
    pub fn set_config_dir(mut self, config_dir: PathBuf) -> Self {
        tracing::trace!(?config_dir, "setting config dir");
        self.config_dir = Some(config_dir);
        self
    }

    /// Set the data directory.
    #[must_use]
    pub fn set_data_dir(mut self, data_dir: PathBuf) -> Self {
        tracing::trace!(?data_dir, "setting data dir");
        self.data_dir = Some(data_dir);
        self
    }

    /// Set the hoards map.
    #[must_use]
    pub fn set_hoards(mut self, hoards: BTreeMap<HoardName, Hoard>) -> Self {
        tracing::trace!(?hoards, "setting hoards");
        self.hoards = Some(hoards);
        self
    }

    /// Set the environments map for this `Builder`.
    ///
    /// The map associates an environment name with the [`Environment`] definition.
    #[must_use]
    pub fn set_environments(
        mut self,
        environments: BTreeMap<EnvironmentName, Environment>,
    ) -> Self {
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
    #[tracing::instrument(level = "trace")]
    fn evaluated_environments(&self) -> Result<BTreeMap<EnvironmentName, bool>, Error> {
        if let Some(envs) = &self.environments {
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
            .map_err(crate::map_log_error(Error::Environment))
    }

    /// Build this [`Builder`] into a [`Config`].
    ///
    /// # Errors
    ///
    /// Any [`enum@Error`] that occurs while evaluating environment or hoard definitions.
    #[tracing::instrument(name = "build_config")]
    pub fn build(mut self) -> Result<Config, Error> {
        tracing::debug!("building configuration from builder");
        let environments = self.evaluated_environments()?;
        tracing::debug!(?environments);
        let exclusivity = self.exclusivity.unwrap_or_default();
        tracing::debug!(?exclusivity);
        let config_file = self.config_file.unwrap_or_else(Self::default_config_file);
        tracing::debug!(?config_file);
        let command = self.command.unwrap_or_default();
        tracing::debug!(?command);
        let force = self.force;
        tracing::debug!(?force);

        if let Some(path) = self.config_dir {
            crate::dirs::set_config_dir(&path);
        }

        if let Some(path) = self.data_dir {
            crate::dirs::set_data_dir(&path);
        }

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
                hoard
                    .process_with(&environments, &exclusivity)
                    .map(|hoard| (name, hoard))
            })
            .collect::<Result<_, Error>>()?;
        tracing::debug!("processed hoards");

        Ok(Config {
            command,
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
                config_file: Some(Builder::default_config_file()),
                command: Some(Command::Validate),
                config_dir: Some(PathBuf::from("/config/dir")),
                data_dir: Some(PathBuf::from("/data/dir")),
                environments: None,
                exclusivity: None,
                hoards: None,
                force: false,
                global_config: None,
            }
        }

        fn get_non_default_populated_builder() -> Builder {
            Builder {
                config_dir: Some(PathBuf::from("/other/config/dir")),
                data_dir: Some(PathBuf::from("/other/data/dir")),
                config_file: Some(PathBuf::from("/testing/config.toml")),
                command: Some(Command::Restore {
                    hoards: vec!["test".parse().unwrap()],
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
                config_dir: None,
                data_dir: None,
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

            assert_eq!(Some(config.config_file), builder.config_file);
            assert_eq!(Some(config.command), builder.command);
        }

        #[test]
        fn builder_with_options_set_uses_options() {
            let builder = get_non_default_populated_builder();
            let config = builder.clone().build().expect("failed to build config");

            assert_eq!(Some(config.config_file), builder.config_file);
            assert_eq!(Some(config.command), builder.command);
        }
    }
}
