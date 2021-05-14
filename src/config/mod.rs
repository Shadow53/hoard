use std::collections::HashMap;
use std::path::PathBuf;

use directories::ProjectDirs;
use log::Level;

use environment::Environment;

use crate::combinator::Combinator;
use crate::command::Command;

pub mod builder;
pub mod environment;
pub mod envtrie;
pub mod hoard;

pub use self::builder::Builder as ConfigBuilder;

pub fn get_dirs() -> ProjectDirs {
    ProjectDirs::from("com", "shadow53", "backup-game-saves")
        .expect("could not detect user home directory to place program files")
}

pub enum Error {
    Builder(builder::Error),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    environments: HashMap<String, Combinator<Environment>>,
    hoards_root: PathBuf,
    config_file: PathBuf,
    pub log_level: Level,
    pub command: Command,
}

impl Default for Config {
    fn default() -> Self {
        Self::builder().build()
    }
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    pub fn load() -> Result<Self, Error> {
        ConfigBuilder::from_args_then_file()
            .map_err(Error::Builder)
            .map(ConfigBuilder::build)
    }

    pub fn get_config_file_path(&self) -> PathBuf {
        self.config_file.clone()
    }

    pub fn get_hoards_root_path(&self) -> PathBuf {
        self.hoards_root.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder_returns_new_builder() {
        assert_eq!(
            Config::builder(),
            ConfigBuilder::new(),
            "Config::builder should return an unmodified new ConfigBuilder"
        );
    }

    #[test]
    fn test_config_default_builds_from_new_builder() {
        assert_eq!(
            Config::default(),
            ConfigBuilder::new().build(),
            "Config::default should be the same as a built unmodified Builder"
        );
    }

    #[test]
    fn test_config_get_config_file_returns_config_file_path() {
        let config = Config::default();
        assert_eq!(
            config.get_config_file_path(),
            config.config_file,
            "should return config file path"
        );
    }

    #[test]
    fn test_config_get_saves_root_returns_saves_root_path() {
        let config = Config::default();
        assert_eq!(
            config.get_hoards_root_path(),
            config.hoards_root,
            "should return saves root path"
        );
    }
}
