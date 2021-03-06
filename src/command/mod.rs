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
    /// Loads all configuration for validation.
    /// If the configuration loads and builds, this command succeeds.
    Validate,
    /// Back up the given hoard(s).
    Backup {
        /// The name(s) of the hoard(s) to back up. Will back up all hoards if empty.
        hoards: Vec<String>,
    },
    /// Restore the files from the given hoard to the filesystem.
    Restore {
        /// The name(s) of the hoard(s) to restore. Will restore all hoards if empty.
        hoards: Vec<String>,
    },
}

impl Default for Command {
    fn default() -> Self {
        Self::Validate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_command_is_help() {
        // The default command is validate if one is not given
        assert_eq!(Command::Validate, Command::default());
    }
}
