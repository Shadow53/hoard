#![deny(clippy::all)]
#![deny(clippy::correctness)]
#![deny(clippy::style)]
#![deny(clippy::complexity)]
#![deny(clippy::perf)]

pub mod backup;
pub mod builder;
pub mod config;
pub mod game;

use directories::ProjectDirs;
use log::{debug, Level};
use std::path::PathBuf;
use structopt::{clap::Error as ClapError, StructOpt};
use thiserror::Error;

pub use builder::ConfigBuilder;
pub use game::{Game, Games, GameType};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_command_is_help() {
        // The default command is help if one is not given
        assert_eq!(Command::Help, Command::default());
    }

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
            config.get_saves_root_path(),
            config.saves_root,
            "should return saves root path"
        );
    }

    #[test]
    fn test_config_get_games_file_returns_games_file_path() {
        let config = Config::default();
        assert_eq!(
            config.get_games_file_path(),
            config.games_file,
            "should return games file path"
        );
    }
}

pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const GAMES_LIST_NAME: &str = "games.toml";
pub const GAMES_DIR_SLUG: &str = "saves";

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to build configuration: {0}")]
    Builder(builder::Error),
    #[error("failed to open games file for reading: {0}")]
    ReadGames(game::Error),
    #[error("failed to back up save files: {0}")]
    Backup(backup::Error),
    #[error("failed to restore save files: {0}")]
    Restore(backup::Error),
    #[error("failed to print help output: {0}")]
    PrintHelp(ClapError),
    #[error("config subcommand failed: {0}")]
    ConfigCmd(config::Error),
    #[error("game subcommand failed: {0}")]
    GameCmd(game::Error),
}

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub enum Command {
    Help,
    Backup,
    Restore,
    Config {
        #[structopt(subcommand)]
        command: config::Command,
    },
    Game {
        #[structopt(subcommand)]
        command: game::Command,
    },
}

impl Default for Command {
    fn default() -> Self {
        Self::Help
    }
}

impl Command {
    pub fn run(&self, config: &Config) -> Result<(), Error> {
        let root = config.get_saves_root_path();
        debug!("Game saves directory: {}", root.to_string_lossy());

        let games = game::read_games_file(&config.games_file).map_err(Error::ReadGames)?;

        match &config.command {
            Command::Help => ConfigBuilder::clap()
                .print_long_help()
                .map_err(Error::PrintHelp)?,
            Command::Backup => backup::backup(&root, &games).map_err(Error::Backup)?,
            Command::Restore => backup::restore(&root, &games).map_err(Error::Restore)?,
            Command::Config { command } => command.run(config).map_err(Error::ConfigCmd)?,
            Command::Game { command } => command.run(config).map_err(Error::GameCmd)?,
        }

        Ok(())
    }
}

pub fn get_dirs() -> ProjectDirs {
    ProjectDirs::from("com", "shadow53", "backup-game-saves")
        .expect("could not detect user home directory to place program files")
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
