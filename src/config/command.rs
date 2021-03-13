use super::config::{Command as ConfigCommand, Error as ConfigCmdError};
use super::game::{Command as GameSubcommand, Error as GameError};
use super::{Config, ConfigBuilder, Error as ConfigError};
use crate::backup;

use log::debug;
use structopt::clap::Error as ClapError;
use structopt::StructOpt;
use thiserror::Error;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_command_is_help() {
        // The default command is help if one is not given
        assert_eq!(Command::Help, Command::default());
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to back up save files: {0}")]
    Backup(backup::Error),
    #[error("failed to restore save files: {0}")]
    Restore(backup::Error),
    #[error("failed to read games list: {0}")]
    GetGames(ConfigError),
    #[error("failed to print help output: {0}")]
    PrintHelp(ClapError),
    #[error("config subcommand failed: {0}")]
    ConfigCmd(ConfigCmdError),
    #[error("game subcommand failed: {0}")]
    GameCmd(GameError),
}

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub enum Command {
    Help,
    Backup,
    Restore,
    Config {
        #[structopt(subcommand)]
        command: ConfigCommand,
    },
    Game {
        #[structopt(subcommand)]
        command: GameSubcommand,
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

        let games = config.get_games().map_err(Error::GetGames)?;

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
