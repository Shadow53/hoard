use std::fs::File;
use std::io::{Write, Error as IOError};
use std::path::Path;

use super::{Config, Error as ConfigError};

use structopt::StructOpt;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use log::Level;
    use super::*;
    use crate::config::Command as TopCommand;

    #[test]
    fn test_set_config_set_root() {
        let empty = Config::empty();
        let key = String::from("root");
        let value = String::from("/test/root");
        let expected = {
            let mut config = empty.clone();
            config.user_set(&key, &value).expect("should be able to set `root`");
            config
        };

        let set_config = SetConfig { key, value };

        assert_eq!(expected, set_config.set_config(&empty).expect("setting config should not fail"));
    }

    #[test]
    fn test_set_config_set_games_list() {
        let empty = Config::empty();
        let key = String::from("games-file");
        let value = String::from("/test/games.toml");
        let expected = {
            let mut config = empty.clone();
            config.user_set(&key, &value).expect("should be able to set `games-file`");
            config
        };

        let set_config = SetConfig { key, value };

        assert_eq!(expected, set_config.set_config(&empty).expect("setting config should not fail"));
    }

    #[test]
    fn test_set_config_set_log_level() {
        let empty = Config::empty();
        let key = String::from("log-level");
        let value = String::from("INFO");
        let expected = {
            let mut config = empty.clone();
            config.user_set(&key, &value).expect("should be able to set `log-level`");
            config
        };

        let set_config = SetConfig { key, value };

        assert_eq!(expected, set_config.set_config(&empty).expect("setting config should not fail"));
    }

    #[test]
    fn test_set_config_set_config_should_fail() {
        let empty = Config::empty();
        let key = String::from("config-file");
        let value = String::from("/test/config.toml");
        let set_config = SetConfig { key: key.clone(), value };
        let err = set_config.set_config(&empty).expect_err("should not be able to set `config-file`");
        match err {
            Error::SetConfig(ConfigError::NoSuchKey(err_key)) => assert_eq!(key, err_key, "setting should fail with NoSuchKey"),
            _ => panic!("unexpected error: {:?}", err),
        }
    }

    #[test]
    fn test_set_config_set_command_should_fail() {
        let empty = Config::empty();
        let key = String::from("command");
        let value = String::from("help");
        let set_config = SetConfig { key: key.clone(), value };
        let err = set_config.set_config(&empty).expect_err("should not be able to set `command`");
        match err {
            Error::SetConfig(ConfigError::NoSuchKey(err_key)) => assert_eq!(key, err_key, "setting should fail with NoSuchKey"),
            _ => panic!("unexpected error: {:?}", err),
        }
    }

    #[test]
    fn test_unset_config_unset_root() {
        let mut input = Config::empty();
        input.root = Some(PathBuf::from("/test/root"));
        let key = String::from("root");
        let expected = {
            let mut config = input.clone();
            config.user_unset(&key).expect("should be able to unset `root`");
            config
        };

        let unset_config = UnsetConfig { key };

        assert_eq!(Config::empty(), unset_config.unset_config(&input).expect("unsetting config should not fail"));
    }


    #[test]
    fn test_unset_config_unset_games_file() {
        let mut input = Config::empty();
        input.games_file = Some(PathBuf::from("/test/games.toml"));
        let key = String::from("games-file");
        let expected = {
            let mut config = input.clone();
            config.user_unset(&key).expect("should be able to unset `games-file`");
            config
        };

        let unset_config = UnsetConfig { key };

        assert_eq!(Config::empty(), unset_config.unset_config(&input).expect("unsetting config should not fail"));
    }

    #[test]
    fn test_unset_config_unset_log_level() {
        let mut input = Config::empty();
        input.log_level = Some(Level::Trace);
        let key = String::from("root");
        let expected = {
            let mut config = input.clone();
            config.user_unset(&key).expect("should be able to unset `log-level`");
            config
        };

        let unset_config = UnsetConfig { key };

        assert_eq!(expected, unset_config.unset_config(&input).expect("unsetting config should not fail"));
    }

    #[test]
    fn test_unset_config_unset_config_file_should_fail() {
        let mut input = Config::empty();
        input.config_file = Some(PathBuf::from("/test/config.toml"));
        let key = String::from("config-file");
        let unset_config = UnsetConfig { key: key.clone() };
        let err = unset_config.unset_config(&input).expect_err("should not be able to unset `config-file`");
        match err {
            Error::UnsetConfig(ConfigError::NoSuchKey(err_key)) => assert_eq!(key, err_key, "unsetting should fail with NoSuchKey"),
            _ => panic!("unexpected error: {:?}", err),
        }
    }

    #[test]
    fn test_unset_config_unset_command_should_fail() {
        let mut input = Config::empty();
        input.command = Some(TopCommand::Help);
        let key = String::from("command");
        let unset_config = UnsetConfig { key: key.clone() };
        let err = unset_config.unset_config(&input).expect_err("should not be able to unset `command`");
        match err {
            Error::UnsetConfig(ConfigError::NoSuchKey(err_key)) => assert_eq!(key, err_key, "unsetting should fail with NoSuchKey"),
            _ => panic!("unexpected error: {:?}", err),
        }
    }
}

#[derive(Debug)]
pub enum Error {
    WriteConfig(IOError),
    Serialize(toml::ser::Error),
    SetConfig(ConfigError),
    UnsetConfig(ConfigError),
}

#[derive(PartialEq, Eq, Debug, Clone, StructOpt)]
pub struct SetConfig {
    pub key: String,
    pub value: String,
}

impl SetConfig {
    fn set_config(&self, config: &Config) -> Result<Config, Error> {
        let mut config = config.clone();
        config.user_set(&self.key, &self.value).map_err(Error::SetConfig)?;
        Ok(config)
    }
}

#[derive(PartialEq, Eq, Debug, Clone, StructOpt)]
pub struct UnsetConfig {
    pub key: String,
}

impl UnsetConfig {
    fn unset_config(&self, config: &Config) -> Result<Config, Error> {
        let mut config = config.clone();
        config.user_unset(&self.key).map_err(Error::UnsetConfig)?;
        Ok(config)
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
    fn save_config(path: &Path, config: &Config) -> Result<(), Error> {
        let mut file = File::create(path).map_err(Error::WriteConfig)?;
        let content = toml::to_string_pretty(config).map_err(Error::Serialize)?;
        file.write_all(content.as_bytes()).map_err(Error::WriteConfig)?;

        Ok(())
    }

    pub fn init(config: &Config) -> Option<Config> {
        let path = config.get_config_path();
        if !path.exists() {
            Some(Config::default())
        } else {
            None
        }
    }

    pub fn run(&self, config: &Config) -> Result<(), Error> {
        let path = config.get_config_path();
        let new_config = match self {
            Self::Init => match Self::init(config) {
                // None == config already exists, no need to write anything
                None => return Ok(()),
                Some(new_config) => new_config,
            },
            Self::Reset => Config::default(),
            Self::Set(set_config) => set_config.set_config(config)?,
            Self::Unset(unset_config) => unset_config.unset_config(config)?,
        };

        Self::save_config(&path, &new_config)
    }
}
