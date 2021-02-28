use crate::backup;
use super::{Config, Error as ConfigError};
use super::game::{Command as GameSubcommand, Error as GameError};
use super::config::{Command as ConfigCommand, Error as ConfigCmdError};

use log::debug;
use structopt::StructOpt;
use structopt::clap::Error as ClapError;

pub enum Error {
    Backup(backup::Error),
    Restore(backup::Error),
    GetGames(ConfigError),
    PrintHelp(ClapError),
    ConfigCmd(ConfigCmdError),
    GameCmd(GameError),
}

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub enum Command {
    Help,
    Backup,
    Restore,
    Config{ #[structopt(subcommand)] command: ConfigCommand },
    Game{ #[structopt(subcommand)] command: GameSubcommand },
}

impl Default for Command {
    fn default() -> Self {
        Self::Help
    }
}

impl Command {
    pub fn run(&self, config: &Config) -> Result<(), Error> {
        let root = config.get_root_path();
        debug!("Game saves directory: {}", root.to_string_lossy());

        let games = config.get_games().map_err(Error::GetGames)?;

        if let Some(command) = &config.command {
            match command {
                Command::Help => Config::clap().print_long_help().map_err(Error::PrintHelp)?,
                Command::Backup => backup::backup(&root, &games).map_err(Error::Backup)?,
                Command::Restore => backup::restore(&root, &games).map_err(Error::Restore)?,
                Command::Config { command } => command.run(config).map_err(Error::ConfigCmd)?,
                Command::Game{ command } => command.run(config).map_err(Error::GameCmd)?,
            }
        }

        Ok(())
    }
}
