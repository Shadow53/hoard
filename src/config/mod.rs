pub use self::builder::Builder as ConfigBuilder;
use self::hoard::Hoard;
use crate::command::Command;
use directories::ProjectDirs;
use log::Level;
use std::collections::BTreeMap;
use std::path::PathBuf;
use thiserror::Error;

pub mod builder;
pub mod hoard;

pub fn get_dirs() -> ProjectDirs {
    ProjectDirs::from("com", "shadow53", "backup-game-saves")
        .expect("could not detect user home directory to place program files")
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("error while building the configuration")]
    Builder(#[from] builder::Error),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub log_level: Level,
    pub command: Command,
    hoards_root: PathBuf,
    config_file: PathBuf,
    hoards: BTreeMap<String, Hoard>,
}

impl Default for Config {
    fn default() -> Self {
        // Calling [`Builder::unset_hoards`] to ensure there is no panic
        // when `expect`ing
        Self::builder()
            .unset_hoards()
            .build()
            .expect("failed to create default config")
    }
}

impl Config {
    pub fn builder() -> ConfigBuilder {
        ConfigBuilder::new()
    }

    pub fn load() -> Result<Self, Error> {
        ConfigBuilder::from_args_then_file()
            .map(ConfigBuilder::build)?
            .map_err(Error::Builder)
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
            Some(Config::default()),
            ConfigBuilder::new().build().ok(),
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
