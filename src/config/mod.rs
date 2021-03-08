use directories::ProjectDirs;
use log::{Level, debug};
use std::{io, ops::BitOr, path::PathBuf};

use crate::games::Games;
use thiserror::Error;

pub mod builder;
pub mod command;
pub mod config;
pub mod game;

pub use builder::ConfigBuilder;

use command::Command;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder_returns_new_builder() {
        assert_eq!(Config::builder(), ConfigBuilder::new(), "Config::builder should return an unmodified new ConfigBuilder");
    }

    #[test]
    fn test_config_default_builds_from_new_builder() {
        assert_eq!(Config::default(), ConfigBuilder::new().build(), "Config::default should be the same as a built unmodified Builder");
    }

    #[test]
    fn test_config_get_config_file_returns_config_file_path() {
        let config = Config::default();
        assert_eq!(config.get_config_file_path(), config.config_file, "should return config file path");
    }

    #[test]
    fn test_config_get_saves_root_returns_saves_root_path() {
        let config = Config::default();
        assert_eq!(config.get_saves_root_path(), config.saves_root, "should return saves root path");
    }

    #[test]
    fn test_config_get_games_file_returns_games_file_path() {
        let config = Config::default();
        assert_eq!(config.get_games_file_path(), config.games_file, "should return games file path");
    }
}

pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const GAMES_LIST_NAME: &str = "games.toml";
pub const GAMES_DIR_SLUG: &str = "saves";

pub fn get_dirs() -> ProjectDirs {
    ProjectDirs::from("com", "shadow53", "backup-game-saves")
        .expect("could not detect user home directory to place program files")
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to build configuration: {0}")]
    Builder(builder::Error),
    #[error("failed to deserialize games file: {0}")]
    DeserializeGames(toml::de::Error),
    //NoSuchKey(String),
    #[error("failed to open games file for reading: {0}")]
    ReadGames(io::Error),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    saves_root: PathBuf,
    config_file: PathBuf,
    games_file: PathBuf,
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

    pub fn get_games(&self) -> Result<Games, Error> {
        let games_path = self.get_games_file_path();
        debug!(
            "Reading games entries from {}",
            games_path.to_string_lossy()
        );
        let s = std::fs::read_to_string(&games_path).map_err(Error::ReadGames)?;
        toml::from_str(&s).map_err(Error::DeserializeGames)
    }

    pub fn get_config_file_path(&self) -> PathBuf {
        self.config_file.clone()
    }

    pub fn get_saves_root_path(&self) -> PathBuf {
        self.saves_root.clone()
    }

    pub fn get_games_file_path(&self) -> PathBuf {
        self.games_file.clone()
    }
}
