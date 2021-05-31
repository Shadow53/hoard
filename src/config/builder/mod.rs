use std::collections::BTreeMap;
use std::convert::TryInto;
use std::io;
use std::path::{Path, PathBuf};

use log::Level;
use serde::de::{Deserializer, Error as DeserializeError, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use thiserror::Error;

use self::hoard::Hoard;
use environment::Environment;

use crate::command::Command;
use crate::CONFIG_FILE_NAME;
use crate::GAMES_DIR_SLUG;

use super::Config;

pub mod environment;
pub mod envtrie;
pub mod hoard;

struct LevelVisitor;

impl<'de> Visitor<'de> for LevelVisitor {
    type Value = Option<Level>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "A valid log level")
    }

    fn visit_str<E: DeserializeError>(self, s: &str) -> Result<Self::Value, E> {
        Ok(Some(
            s.parse::<Level>()
                .map_err(|err| E::custom(err.to_string()))?,
        ))
    }

    fn visit_none<E: DeserializeError>(self) -> Result<Option<Level>, E> {
        Ok(None)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_str(self)
    }
}

#[allow(single_use_lifetimes)]
fn deserialize_level<'de, D>(deserializer: D) -> Result<Option<Level>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_option(LevelVisitor)
}

fn serialize_level<S>(level: &Option<Level>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    match level {
        None => serializer.serialize_none(),
        Some(level) => serializer.serialize_str(level.to_string().as_str()),
    }
}

#[allow(clippy::unnecessary_wraps)]
fn default_level() -> Option<Level> {
    Some(Level::Info)
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to parse configuration file: {0}")]
    DeserializeConfig(toml::de::Error),
    #[error("failed to read configuration file: {0}")]
    ReadConfig(io::Error),
    #[error("failed to determine current environment: {0}")]
    Environment(#[from] environment::Error),
    #[error("failed to process hoard configuration: {0}")]
    ProcessHoard(#[from] hoard::Error),
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, StructOpt)]
#[structopt(rename_all = "kebab")]
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
    #[structopt(short, long)]
    #[serde(
        deserialize_with = "deserialize_level",
        serialize_with = "serialize_level",
        default = "default_level"
    )]
    log_level: Option<Level>,
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
    fn default_config_file() -> PathBuf {
        super::get_dirs().config_dir().join(CONFIG_FILE_NAME)
    }

    fn default_hoard_root() -> PathBuf {
        super::get_dirs().data_dir().join(GAMES_DIR_SLUG)
    }

    /// Create a new `ConfigBuilder`.
    ///
    /// If [`build`](ConfigBuilder::build) is immediately called on this, the returned
    /// [`Config`] will have all default values.
    pub fn new() -> Self {
        Self {
            hoards: None,
            hoards_root: None,
            config_file: None,
            log_level: None,
            command: None,
            environments: None,
            exclusivity: None,
        }
    }

    /// Create a new `ConfigBuilder` pre-populated with the contents of the given TOML file.
    pub fn from_file(path: &Path) -> Result<Self, Error> {
        let s = std::fs::read_to_string(path).map_err(Error::ReadConfig)?;
        toml::from_str(&s).map_err(Error::DeserializeConfig)
    }

    /// Helper method to process command-line arguments and the config file specified on CLI
    /// (or the default).
    pub fn from_args_then_file() -> Result<Self, Error> {
        let from_args = Self::from_args();
        let config_file = from_args
            .config_file
            .clone()
            .unwrap_or_else(Self::default_config_file);
        let from_file = Self::from_file(&config_file)?;

        Ok(from_file.layer(from_args))
    }

    /// Applies all configured values in `other` over those in *this* `ConfigBuilder`.
    pub fn layer(mut self, other: Self) -> Self {
        if let Some(path) = other.hoards_root {
            self = self.set_hoards_root(path);
        }

        if let Some(path) = other.config_file {
            self = self.set_config_file(path);
        }

        if let Some(path) = other.log_level {
            self = self.set_log_level(path);
        }

        if let Some(path) = other.command {
            self = self.set_command(path);
        }

        self
    }

    /// Set the hoards map.
    pub fn set_hoards(mut self, hoards: BTreeMap<String, Hoard>) -> Self {
        self.hoards = Some(hoards);
        self
    }

    /// Set the directory that will contain all game save data.
    pub fn set_hoards_root(mut self, path: PathBuf) -> Self {
        self.hoards_root = Some(path);
        self
    }

    /// Set the file that contains configuration.
    ///
    /// This currently only exists for completeness. You probably want [`ConfigBuilder::from_file`]
    /// instead, which will actually read and parse the file.
    pub fn set_config_file(mut self, path: PathBuf) -> Self {
        self.config_file = Some(path);
        self
    }

    /// Set the log level.
    pub fn set_log_level(mut self, level: Level) -> Self {
        self.log_level = Some(level);
        self
    }

    /// Set the command that will be run.
    pub fn set_command(mut self, cmd: Command) -> Self {
        self.command = Some(cmd);
        self
    }

    /// Unset the hoards map
    pub fn unset_hoards(mut self) -> Self {
        self.hoards = None;
        self
    }

    /// Unset the directory that will contain all game save data.
    pub fn unset_hoards_root(mut self) -> Self {
        self.hoards_root = None;
        self
    }

    /// Unset the file that contains configuration.
    pub fn unset_config_file(mut self) -> Self {
        self.config_file = None;
        self
    }

    /// Unset the log level.
    pub fn unset_log_level(mut self) -> Self {
        self.log_level = None;
        self
    }

    /// Unset the command that will be run.
    pub fn unset_command(mut self) -> Self {
        self.command = None;
        self
    }

    fn evaluated_environments(
        &self,
    ) -> Result<BTreeMap<String, bool>, <Environment as TryInto<bool>>::Error> {
        self.environments
            .as_ref()
            .map(|map| {
                map.iter()
                    .map(|(key, env)| Ok((key.clone(), env.clone().try_into()?)))
                    .collect()
            })
            .unwrap_or_else(|| Ok(BTreeMap::new()))
    }

    pub fn build(self) -> Result<Config, Error> {
        let environments = self.evaluated_environments()?;
        let exclusivity = self.exclusivity.unwrap_or_else(Vec::new);
        let hoards_root = self.hoards_root.unwrap_or_else(Self::default_hoard_root);
        let config_file = self.config_file.unwrap_or_else(Self::default_config_file);
        let log_level = self.log_level.unwrap_or(log::Level::Info);
        let command = self.command.unwrap_or(Command::Help);
        let hoards = self
            .hoards
            .unwrap_or_else(BTreeMap::new)
            .into_iter()
            .map(|(name, hoard)| Ok((name, hoard.process_with(&environments, &exclusivity)?)))
            .collect::<Result<_, Error>>()?;

        Ok(Config {
            log_level,
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

    mod level_serde {}

    mod builder {
        use super::*;

        fn get_default_populated_builder() -> Builder {
            Builder {
                hoards_root: Some(Builder::default_hoard_root()),
                config_file: Some(Builder::default_config_file()),
                log_level: Some(Level::Info),
                command: Some(Command::Help),
                environments: None,
                exclusivity: None,
                hoards: None,
            }
        }

        fn get_non_default_populated_builder() -> Builder {
            Builder {
                hoards_root: Some(PathBuf::from("/testing/saves")),
                config_file: Some(PathBuf::from("/testing/config.toml")),
                log_level: Some(Level::Debug),
                command: Some(Command::Restore),
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
                log_level: None,
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
                some.clone().layer(none.clone()),
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
                layer2.clone().layer(layer1.clone()),
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
        fn builder_log_level_sets_correctly() {
            let mut builder = Builder::new();
            assert_eq!(None, builder.log_level, "log_level should start as None");
            let level = Level::Debug;
            builder = builder.set_log_level(level.clone());
            assert_eq!(
                Some(level),
                builder.log_level,
                "log_level should now be set"
            );
        }

        #[test]
        fn builder_command_sets_correctly() {
            let mut builder = Builder::new();
            assert_eq!(None, builder.command, "command should start as None");
            let cmd = Command::Help;
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
        fn builder_log_level_unsets_correctly() {
            let mut builder = Builder::new();
            let level = Level::Debug;
            builder = builder.set_log_level(level.clone());
            assert_eq!(
                Some(level),
                builder.log_level,
                "log_level should start as set"
            );
            builder = builder.unset_log_level();
            assert_eq!(None, builder.log_level, "log_level should now be None");
        }

        #[test]
        fn builder_command_unsets_correctly() {
            let mut builder = Builder::new();
            let cmd = Command::Help;
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
            assert_eq!(Some(config.log_level), builder.log_level);
            assert_eq!(Some(config.command), builder.command);
        }

        #[test]
        fn builder_with_options_set_uses_options() {
            let builder = get_non_default_populated_builder();
            let config = builder.clone().build().expect("failed to build config");

            assert_eq!(Some(config.hoards_root), builder.hoards_root);
            assert_eq!(Some(config.config_file), builder.config_file);
            assert_eq!(Some(config.log_level), builder.log_level);
            assert_eq!(Some(config.command), builder.command);
        }
    }
}
