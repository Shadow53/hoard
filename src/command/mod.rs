//! See [`Command`].

mod backup_restore;
mod cleanup;
mod diff;
mod edit;
mod list;
mod status;

use structopt::StructOpt;
use thiserror::Error;

pub(crate) use backup_restore::{run_backup, run_restore};
pub(crate) use cleanup::run_cleanup;
pub(crate) use edit::run_edit;
pub(crate) use diff::run_diff;
pub(crate) use list::run_list;
pub(crate) use status::run_status;

/// Errors that can occur while running commands.
#[derive(Debug, Error)]
pub enum Error {
    /// Error occurred while printing the help message.
    #[error("error while printing help message: {0}")]
    PrintHelp(#[from] structopt::clap::Error),
    /// Error occurred while backing up a hoard.
    #[error("failed to back up {name}: {error}")]
    Backup {
        /// The name of the hoard that failed to back up.
        name: String,
        /// The error that occurred.
        #[source]
        error: crate::hoard::Error,
    },
    /// Error occurred while running [`Checkers`](crate::checkers::Checkers).
    #[error("error while running or saving consistency checks: {0}")]
    Checkers(#[from] crate::checkers::Error),
    /// An error occurred while running the cleanup command.
    #[error("error after cleaning up {success_count} log files: {error}")]
    Cleanup {
        /// The number of files successfully cleaned.
        success_count: u32,
        /// The error that occurred.
        #[source]
        error: crate::checkers::history::operation::Error,
    },
    /// Error occurred while running the diff command.
    #[error("error while running hoard diff: {0}")]
    Diff(#[source] crate::hoard::iter::Error),
    /// Error occurred while running the edit command.
    #[error("error while running hoard edit: {0}")]
    Edit(#[from] edit::Error),
    /// Error occurred while restoring a hoard.
    #[error("failed to back up {name}: {error}")]
    Restore {
        /// The name of the hoard that failed to restore.
        name: String,
        /// The error that occurred.
        #[source]
        error: crate::hoard::Error,
    },
    /// Error occurred while running the status command.
    #[error("error while running hoard status: {0}")]
    Status(#[source] crate::hoard::iter::Error),
}

/// The possible subcommands for `hoard`.
#[derive(Clone, PartialEq, Debug, StructOpt)]
pub enum Command {
    /// Loads all configuration for validation.
    /// If the configuration loads and builds, this command succeeds.
    Validate,
    /// Cleans up the operation logs for all known systems.
    Cleanup,
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
    /// List configured hoards.
    List,
    /// Open the configuration file in the system default editor.
    Edit,
    /// Show which files differ for a given hoard. Optionally show unified diffs for text files
    /// too.
    Diff {
        /// The name of the hoard to diff.
        hoard: String,
        /// If true, prints unified diffs for text files.
        #[structopt(long, short)]
        verbose: bool,
    },
    /// Provides a summary of which hoards have changes and if the diffs can be resolved
    /// with a single command.
    Status,
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
    fn default_command_is_validate() {
        // The default command is validate if one is not given
        assert_eq!(Command::Validate, Command::default());
    }
}
