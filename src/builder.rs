use std::io;
use std::path::{Path, PathBuf};

use log::Level;
use serde::de::{Deserializer, Error as DeserializeError, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};
use structopt::StructOpt;
use thiserror::Error;

use super::CONFIG_FILE_NAME;
use super::GAMES_DIR_SLUG;
use super::GAMES_LIST_NAME;
use super::{get_dirs, Command, Config};

#[cfg(test)]
mod tests {
    use super::*;

    mod level_serde {}

    mod builder {
        use super::*;

        fn get_default_populated_builder() -> ConfigBuilder {
            ConfigBuilder {
                saves_root: Some(ConfigBuilder::default_saves_root()),
                games_file: Some(ConfigBuilder::default_games_file()),
                config_file: Some(ConfigBuilder::default_config_file()),
                log_level: Some(Level::Info),
                command: Some(Command::Help),
            }
        }

        fn get_non_default_populated_builder() -> ConfigBuilder {
            ConfigBuilder {
                saves_root: Some(PathBuf::from("/testing/saves")),
                games_file: Some(PathBuf::from("/testing/games.toml")),
                config_file: Some(PathBuf::from("/testing/config.toml")),
                log_level: Some(Level::Debug),
                command: Some(Command::Restore),
            }
        }

        #[test]
        fn default_builder_is_new() {
            assert_eq!(ConfigBuilder::new(), ConfigBuilder::default());
        }

        #[test]
        fn builder_default_games_file_with_config_given_valid_config() {
            let config = PathBuf::from("/testing/config.toml");
            let games_file = ConfigBuilder::default_games_list_path_with_config(&config);

            assert_eq!(
                games_file.parent(),
                config.parent(),
                "config and games files should share a parent"
            );
        }

        #[test]
        fn builder_default_games_file_with_config_given_invalid_config() {
            // The root has no parent
            let config = PathBuf::from("/");
            let games_file = ConfigBuilder::default_games_list_path_with_config(&config);

            assert_eq!(
                ConfigBuilder::default_games_file(),
                games_file,
                "games files should fall back to default value"
            );
        }

        #[test]
        fn new_builder_is_all_none() {
            let expected = ConfigBuilder {
                saves_root: None,
                games_file: None,
                config_file: None,
                log_level: None,
                command: None,
            };

            assert_eq!(
                expected,
                ConfigBuilder::new(),
                "ConfigBuild::new() should have all None fields"
            );
        }

        #[test]
        fn layered_builder_prefers_some_over_none() {
            let some = get_default_populated_builder();
            let none = ConfigBuilder::new();

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
            let mut builder = ConfigBuilder::new();
            assert_eq!(None, builder.saves_root, "saves_root should start as None");
            let path = PathBuf::from("/testing/saves");
            builder = builder.set_saves_root(path.clone());
            assert_eq!(
                Some(path),
                builder.saves_root,
                "saves_root should now be set"
            );
        }

        #[test]
        fn builder_config_file_sets_correctly() {
            let mut builder = ConfigBuilder::new();
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
        fn builder_games_file_sets_correctly() {
            let mut builder = ConfigBuilder::new();
            assert_eq!(None, builder.games_file, "games_file should start as None");
            let path = PathBuf::from("/testing/saves");
            builder = builder.set_games_file(path.clone());
            assert_eq!(
                Some(path),
                builder.games_file,
                "games_file should now be set"
            );
        }

        #[test]
        fn builder_log_level_sets_correctly() {
            let mut builder = ConfigBuilder::new();
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
            let mut builder = ConfigBuilder::new();
            assert_eq!(None, builder.command, "command should start as None");
            let cmd = Command::Help;
            builder = builder.set_command(cmd.clone());
            assert_eq!(Some(cmd), builder.command, "command should now be set");
        }

        #[test]
        fn builder_saves_root_unsets_correctly() {
            let mut builder = ConfigBuilder::new();
            let path = PathBuf::from("/testing/saves");
            builder = builder.set_saves_root(path.clone());
            assert_eq!(
                Some(path),
                builder.saves_root,
                "saves_root should start as set"
            );
            builder = builder.unset_saves_root();
            assert_eq!(None, builder.saves_root, "saves_root should now be None");
        }

        #[test]
        fn builder_config_file_unsets_correctly() {
            let mut builder = ConfigBuilder::new();
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
        fn builder_games_file_unsets_correctly() {
            let mut builder = ConfigBuilder::new();
            let path = PathBuf::from("/testing/games.toml");
            builder = builder.set_games_file(path.clone());
            assert_eq!(
                Some(path),
                builder.games_file,
                "games_file should start as set"
            );
            builder = builder.unset_games_file();
            assert_eq!(None, builder.games_file, "games_file should now be None");
        }

        #[test]
        fn builder_log_level_unsets_correctly() {
            let mut builder = ConfigBuilder::new();
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
            let mut builder = ConfigBuilder::new();
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
            let config = ConfigBuilder::new().build();

            assert_eq!(Some(config.saves_root), builder.saves_root);
            assert_eq!(Some(config.config_file), builder.config_file);
            assert_eq!(Some(config.games_file), builder.games_file);
            assert_eq!(Some(config.log_level), builder.log_level);
            assert_eq!(Some(config.command), builder.command);
        }

        #[test]
        fn builder_with_options_set_uses_options() {
            let builder = get_non_default_populated_builder();
            let config = builder.clone().build();

            assert_eq!(Some(config.saves_root), builder.saves_root);
            assert_eq!(Some(config.config_file), builder.config_file);
            assert_eq!(Some(config.games_file), builder.games_file);
            assert_eq!(Some(config.log_level), builder.log_level);
            assert_eq!(Some(config.command), builder.command);
        }

        #[test]
        fn config_from_builder_is_just_build() {
            let builder = get_non_default_populated_builder();

            assert_eq!(builder.clone().build(), Config::from(builder));
        }

        #[test]
        fn builder_from_config_undoes_build() {
            let builder = get_non_default_populated_builder();
            let config = builder.clone().build();
            assert_eq!(builder, ConfigBuilder::from(config));
        }
    }
}

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

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_str(self)
    }

    fn visit_none<E: DeserializeError>(self) -> Result<Option<Level>, E> {
        Ok(None)
    }
}

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
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize, StructOpt)]
#[structopt(rename_all = "kebab")]
pub struct ConfigBuilder {
    #[structopt(short, long)]
    saves_root: Option<PathBuf>,
    #[structopt(short, long)]
    games_file: Option<PathBuf>,
    #[structopt(short, long)]
    #[serde(skip)]
    config_file: Option<PathBuf>,
    #[structopt(short, long)]
    #[serde(
        deserialize_with = "deserialize_level",
        serialize_with = "serialize_level",
        default = "default_level"
    )]
    log_level: Option<log::Level>,
    #[serde(skip)]
    #[structopt(subcommand)]
    command: Option<Command>,
}

impl Default for ConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ConfigBuilder {
    fn default_config_file() -> PathBuf {
        get_dirs().config_dir().join(CONFIG_FILE_NAME)
    }

    fn default_saves_root() -> PathBuf {
        get_dirs().data_dir().join(GAMES_DIR_SLUG)
    }

    fn default_games_file() -> PathBuf {
        get_dirs().config_dir().join(GAMES_LIST_NAME)
    }

    fn default_games_list_path_with_config(config_path: &Path) -> PathBuf {
        // Default next to the config file.
        config_path
            .parent()
            .map(|path| path.join(GAMES_LIST_NAME))
            .unwrap_or_else(|| {
                // If parent doesn't exist for some reason
                // It always should, but this is preferable to unwrap(), IMO
                Self::default_games_file()
            })
    }

    /// Create a new `ConfigBuilder`.
    ///
    /// If [`build`](ConfigBuilder::build) is immediately called on this, the returned
    /// [`Config`] will have all default values.
    pub fn new() -> Self {
        Self {
            saves_root: None,
            games_file: None,
            config_file: None,
            log_level: None,
            command: None,
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
        if let Some(path) = other.saves_root {
            self = self.set_saves_root(path);
        }

        if let Some(path) = other.config_file {
            self = self.set_config_file(path);
        }

        if let Some(path) = other.games_file {
            self = self.set_games_file(path);
        }

        if let Some(path) = other.log_level {
            self = self.set_log_level(path);
        }

        if let Some(path) = other.command {
            self = self.set_command(path);
        }

        self
    }

    /// Set the directory that will contain all game save data.
    pub fn set_saves_root(mut self, path: PathBuf) -> Self {
        self.saves_root = Some(path);
        self
    }

    /// Set the file that describes where to find game save data.
    pub fn set_games_file(mut self, path: PathBuf) -> Self {
        self.games_file = Some(path);
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
    pub fn set_log_level(mut self, level: log::Level) -> Self {
        self.log_level = Some(level);
        self
    }

    /// Set the command that will be run.
    pub fn set_command(mut self, cmd: Command) -> Self {
        self.command = Some(cmd);
        self
    }

    /// Unset the directory that will contain all game save data.
    pub fn unset_saves_root(mut self) -> Self {
        self.saves_root = None;
        self
    }

    /// Unset the file that describes where to find game save data.
    pub fn unset_games_file(mut self) -> Self {
        self.games_file = None;
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

    /// Build the `Config`.
    ///
    /// Because specific types are required while setting fields and no extra processing
    /// is done during build, and since all fields have sane defaults, this function
    /// cannot error and will not panic.
    pub fn build(self) -> Config {
        let saves_root = self.saves_root.unwrap_or_else(Self::default_saves_root);
        let config_file = self.config_file.unwrap_or_else(Self::default_config_file);
        let games_file = self
            .games_file
            .unwrap_or_else(|| Self::default_games_list_path_with_config(&config_file));
        let log_level = self.log_level.unwrap_or(log::Level::Info);
        let command = self.command.unwrap_or(Command::Help);

        Config {
            saves_root,
            config_file,
            games_file,
            log_level,
            command,
        }
    }
}

impl From<ConfigBuilder> for Config {
    fn from(builder: ConfigBuilder) -> Config {
        builder.build()
    }
}

impl From<Config> for ConfigBuilder {
    fn from(config: Config) -> ConfigBuilder {
        ConfigBuilder::new()
            .set_saves_root(config.saves_root)
            .set_config_file(config.config_file)
            .set_games_file(config.games_file)
            .set_log_level(config.log_level)
            .set_command(config.command)
    }
}
