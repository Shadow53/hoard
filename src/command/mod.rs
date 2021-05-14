pub mod backup;

use crate::config::{Config, ConfigBuilder};
use log::debug;
use structopt::StructOpt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("error while backing up: {0}")]
    Backup(backup::Error),
    #[error("error while printing help message: {0}")]
    PrintHelp(structopt::clap::Error),
    #[error("error while restoring up: {0}")]
    Restore(backup::Error),
}

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub enum Command {
    Help,
    Backup,
    Restore,
}

impl Default for Command {
    fn default() -> Self {
        Self::Help
    }
}

impl Command {
    pub fn run(&self, config: &Config) -> Result<(), Error> {
        let root = config.get_hoards_root_path();
        debug!("Game saves directory: {}", root.to_string_lossy());

        //let games = game::read_games_file(&config.games_file).map_err(Error::ReadGames)?;

        //match &config.command {
        //    Command::Help => ConfigBuilder::clap()
        //        .print_long_help()
        //        .map_err(Error::PrintHelp)?,
        //    Command::Backup => backup::backup(&root, &games).map_err(Error::Backup)?,
        //    Command::Restore => backup::restore(&root, &games).map_err(Error::Restore)?,
        //}

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_command_is_help() {
        // The default command is help if one is not given
        assert_eq!(Command::Help, Command::default());
    }
}
