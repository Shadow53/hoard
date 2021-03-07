use directories::ProjectDirs;
use log::{Level, debug};
use serde::{Deserialize, Serialize, Serializer, de::{Deserializer, Error as DeserializeError, Visitor}};
use std::{io, ops::BitOr, path::PathBuf};
use structopt::StructOpt;

use crate::games::Games;

pub mod builder;
pub mod command;
pub mod config;
pub mod game;

pub use builder::ConfigBuilder;

use command::Command;

pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const GAMES_LIST_NAME: &str = "games.toml";
pub const GAMES_DIR_SLUG: &str = "saves";

pub fn get_dirs() -> ProjectDirs {
    ProjectDirs::from("com", "shadow53", "backup-game-saves")
        .expect("could not detect user home directory to place program files")
}

#[derive(Debug)]
pub enum Error {
    Builder(builder::Error),
    DeserializeGames(toml::de::Error),
    NoSuchKey(String),
    ParseLogLevel(log::ParseLevelError),
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
