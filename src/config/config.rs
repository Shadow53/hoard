use std::{fmt, fs::File, path::PathBuf, str::FromStr};
use std::io::{Write, Error as IOError};
use std::path::Path;

use super::{Config, ConfigBuilder, Error as ConfigError, builder};

use structopt::StructOpt;
use thiserror::Error;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use log::Level;
    use crate::config::Command as TopCommand;

    #[test]
    fn field_fromstr_supports_kebab_case() {
        assert_eq!(ConfigField::SavesRoot, "saves-root".parse::<ConfigField>().expect("failed to parse saves-root"));
        assert_eq!(ConfigField::GamesFile, "games-file".parse::<ConfigField>().expect("failed to parse games-root"));
        assert_eq!(ConfigField::ConfigFile, "config-file".parse::<ConfigField>().expect("failed to parse config-file"));
        assert_eq!(ConfigField::LogLevel, "log-level".parse::<ConfigField>().expect("failed to parse log-level"));
        assert_eq!(ConfigField::Command, "command".parse::<ConfigField>().expect("failed to parse command"));
    }

    #[test]
    fn field_fromstr_supports_snake_case() {
        assert_eq!(ConfigField::SavesRoot, "saves_root".parse::<ConfigField>().expect("failed to parse saves-root"));
        assert_eq!(ConfigField::GamesFile, "games_file".parse::<ConfigField>().expect("failed to parse games-root"));
        assert_eq!(ConfigField::ConfigFile, "config_file".parse::<ConfigField>().expect("failed to parse config-file"));
        assert_eq!(ConfigField::LogLevel, "log_level".parse::<ConfigField>().expect("failed to parse log-level"));
        assert_eq!(ConfigField::Command, "command".parse::<ConfigField>().expect("failed to parse command"));
    }

    #[test]
    fn field_to_string_is_lower_snake_case() {
        assert_eq!(ConfigField::SavesRoot.to_string(), "saves_root");
        assert_eq!(ConfigField::GamesFile.to_string(), "games_file");
        assert_eq!(ConfigField::ConfigFile.to_string(), "config_file");
        assert_eq!(ConfigField::LogLevel.to_string(), "log_level");
        assert_eq!(ConfigField::Command.to_string(), "command");
    }

    #[test]
    fn test_set_config_set_root() {
        let empty = Config::builder().build();
        let field = ConfigField::SavesRoot;
        let value = String::from("/test/root");
        let expected = {
            ConfigBuilder::from(empty.clone())
                .set_saves_root(PathBuf::from(&value))
        };

        let set_config = SetConfig { field, value };

        assert_eq!(expected, set_config.set_config(&empty).expect("setting config should not fail"));
    }

    #[test]
    fn test_set_config_set_games_list() {
        let empty = Config::builder().build();
        let field = ConfigField::GamesFile;
        let value = String::from("/test/games.toml");
        let expected = {
            ConfigBuilder::from(empty.clone())
                .set_games_file(PathBuf::from(&value))
        };

        let set_config = SetConfig { field, value };

        assert_eq!(expected, set_config.set_config(&empty).expect("setting config should not fail"));
    }

    #[test]
    fn test_set_config_set_log_level() {
        let empty = Config::builder().build();
        let field = ConfigField::LogLevel;
        let value = String::from("INFO");
        let expected = {
            ConfigBuilder::from(empty.clone())
                .set_log_level(Level::Info)
        };

        let set_config = SetConfig { field, value };

        assert_eq!(expected, set_config.set_config(&empty).expect("setting config should not fail"));
    }

    #[test]
    fn test_set_config_set_config_should_fail() {
        let empty = Config::builder().build();
        let field = ConfigField::ConfigFile;
        let value = String::from("/test/config.toml");
        let set_config = SetConfig { field, value };
        let err = set_config.set_config(&empty).expect_err("should not be able to set `config-file`");
        match err {
            Error::NotFileField(err_field) => assert_eq!(field, err_field, "setting should fail with NotFileField"),
            _ => panic!("unexpected error: {:?}", err),
        }
    }

    #[test]
    fn test_set_config_set_command_should_fail() {
        let empty = Config::builder().build();
        let field = ConfigField::Command;
        let value = String::from("help");
        let set_config = SetConfig { field, value };
        let err = set_config.set_config(&empty).expect_err("should not be able to set `command`");
        match err {
            Error::NotFileField(err_field) => assert_eq!(field, err_field, "setting should fail with NotFileField"),
            _ => panic!("unexpected error: {:?}", err),
        }
    }

    #[test]
    fn test_unset_config_unset_root() {
        let mut input = Config::builder().build();
        input.saves_root = PathBuf::from("/test/root");
        let field = ConfigField::SavesRoot;
        let expected: ConfigBuilder = {
            ConfigBuilder::from(input.clone())
                .unset_saves_root()
        };

        let unset_config = UnsetConfig { field };

        assert_eq!(expected, unset_config.unset_config(&input).expect("unsetting config should not fail"));
    }


    #[test]
    fn test_unset_config_unset_games_file() {
        let mut input = Config::builder().build();
        input.games_file = PathBuf::from("/test/games.toml");
        let field = ConfigField::GamesFile;
        let expected: ConfigBuilder = {
            ConfigBuilder::from(input.clone())
                .unset_games_file()
        };

        let unset_config = UnsetConfig { field };

        assert_eq!(expected, unset_config.unset_config(&input).expect("unsetting config should not fail"));
    }

    #[test]
    fn test_unset_config_unset_log_level() {
        let mut input = Config::builder().build();
        input.log_level = Level::Trace;
        let field = ConfigField::LogLevel;
        let expected = {
            ConfigBuilder::from(input.clone())
                .unset_log_level()
        };

        let unset_config = UnsetConfig { field };

        assert_eq!(expected, unset_config.unset_config(&input).expect("unsetting config should not fail"));
    }

    #[test]
    fn test_unset_config_unset_config_file_should_fail() {
        let mut input = Config::builder().build();
        input.config_file = PathBuf::from("/test/config.toml");
        let field = ConfigField::ConfigFile;
        let unset_config = UnsetConfig { field };
        let err = unset_config.unset_config(&input).expect_err("should not be able to unset `config-file`");
        match err {
            Error::NotFileField(err_field) => assert_eq!(field, err_field, "unsetting should fail with NotFileField"),
            _ => panic!("unexpected error: {:?}", err),
        }
    }

    #[test]
    fn test_unset_config_unset_command_should_fail() {
        let mut input = Config::builder().build();
        input.command = TopCommand::Help;
        let field = ConfigField::Command;
        let unset_config = UnsetConfig { field };
        let err = unset_config.unset_config(&input).expect_err("should not be able to unset `command`");
        match err {
            Error::NotFileField(err_field) => assert_eq!(field, err_field, "unsetting should fail with NotFileField"),
            _ => panic!("unexpected error: {:?}", err),
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to read configuration file: {0}")]
    ReadConfig(builder::Error),
    #[error("failed to write to configuration file: {0}")]
    WriteConfig(IOError),
    #[error("failed to serialize configuration as TOML: {0}")]
    Serialize(toml::ser::Error),
    #[error("failed to parse log level: {0}")]
    ParseLevel(log::ParseLevelError),
    #[error("unknown configuration file field: {0}")]
    UnknownField(String),
    #[error("field not supported in configuration file: {0}")]
    NotFileField(ConfigField),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ConfigField {
    SavesRoot,
    GamesFile,
    ConfigFile,
    LogLevel,
    Command,
}

impl ConfigField {
    fn ensure_in_file(&self) -> Result<(), Error> {
        match self {
            Self::SavesRoot | Self::GamesFile | Self::LogLevel => Ok(()),
            _ => Err(Error::NotFileField(*self)),
        }
    }
}

impl FromStr for ConfigField {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "saves_root"  | "saves-root"  => Ok(Self::SavesRoot),
            "games_file"  | "games-file"  => Ok(Self::GamesFile),
            "config_file" | "config-file" => Ok(Self::ConfigFile),
            "log_level"   | "log-level"   => Ok(Self::LogLevel),
            "command" => Ok(Self::Command),
            _ => Err(Error::UnknownField(s.to_owned())),
        }
    }
}

impl fmt::Display for ConfigField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SavesRoot => write!(f, "saves_root"),
            Self::GamesFile => write!(f, "games_file"),
            Self::ConfigFile => write!(f, "config_file"),
            Self::LogLevel => write!(f, "log_level"),
            Self::Command => write!(f, "command"),
        }
    }
}

#[derive(PartialEq, Eq, Debug, Clone, StructOpt)]
pub struct SetConfig {
    pub field: ConfigField,
    pub value: String,
}

impl SetConfig {
    fn set_config(&self, config: &Config) -> Result<ConfigBuilder, Error> {
        self.field.ensure_in_file()?;
        let mut builder = ConfigBuilder::from(config.to_owned());

        // The `ensure_in_file` check prevents certain variants from
        // ever being matched. That said, they are included here anyways
        // for completeness' sake.
        builder = match self.field {
            ConfigField::SavesRoot => builder.set_saves_root(PathBuf::from(&self.value)),
            ConfigField::GamesFile => builder.set_games_file(PathBuf::from(&self.value)),
            ConfigField::LogLevel => builder.set_log_level(self.value.parse().map_err(Error::ParseLevel)?),
            ConfigField::ConfigFile | ConfigField::Command => builder,
        };

        Ok(builder)
    }
}

#[derive(PartialEq, Eq, Debug, Clone, StructOpt)]
pub struct UnsetConfig {
    pub field: ConfigField,
}

impl UnsetConfig {
    fn unset_config(&self, config: &Config) -> Result<ConfigBuilder, Error> {
        self.field.ensure_in_file()?;
        let mut builder = ConfigBuilder::from(config.to_owned());

        // The `ensure_in_file` check prevents certain variants from
        // ever being matched. That said, they are included here anyways
        // for completeness' sake.
        builder = match self.field {
            ConfigField::SavesRoot => builder.unset_saves_root(),
            ConfigField::GamesFile => builder.unset_games_file(),
            ConfigField::LogLevel => builder.unset_log_level(),
            ConfigField::ConfigFile | ConfigField::Command => builder,
        };

        Ok(builder)
    }
}

#[derive(PartialEq, Eq, Debug, Clone, StructOpt)]
pub enum Command {
    Init,
    Reset,
    Set(SetConfig),
    Unset(UnsetConfig),
}

impl Command {
    fn save_config(path: &Path, config: &ConfigBuilder) -> Result<(), Error> {
        let mut file = File::create(path).map_err(Error::WriteConfig)?;
        let builder = ConfigBuilder::from(config.to_owned());
        let content = toml::to_string_pretty(&builder).map_err(Error::Serialize)?;
        file.write_all(content.as_bytes()).map_err(Error::WriteConfig)?;

        Ok(())
    }

    fn init(config: &Config) -> Result<ConfigBuilder, Error> {
        let path = config.get_config_file_path();
        let builder = ConfigBuilder::from(config.to_owned());
        if !path.exists() {
            Ok(builder)
        } else {
            ConfigBuilder::from_file(&path)
                .map_err(Error::ReadConfig)
                .map(|inner| inner.layer(builder))
        }
    }

    pub fn run(&self, config: &Config) -> Result<(), Error> {
        let path = config.get_config_file_path();
        let new_config = match self {
            Self::Init => Self::init(config)?,
            Self::Reset => ConfigBuilder::from(Config::builder().set_config_file(path.clone()).build()),
            Self::Set(set_config) => set_config.set_config(config)?,
            Self::Unset(unset_config) => unset_config.unset_config(config)?,
        };

        Self::save_config(&path, &new_config)
    }
}
