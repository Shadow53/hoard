//! See [`Command`].
use structopt::StructOpt;
use thiserror::Error;

/// Errors that can occur while running commands.
#[derive(Debug, Error)]
pub enum Error {
    /// Error occurred while printing the help message.
    #[error("error while printing help message: {0}")]
    PrintHelp(#[from] structopt::clap::Error),
}

/// The possible subcommands for `hoard`.
#[derive(Clone, PartialEq, Debug, StructOpt)]
pub enum Command {
    /// The autogenerated help command from `structopt`.
    Help,
    /// Back up the given hoard.
    Backup,
    /// Restore the files from the given hoard to the filesystem.
    Restore,
}

impl Default for Command {
    fn default() -> Self {
        Self::Help
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
