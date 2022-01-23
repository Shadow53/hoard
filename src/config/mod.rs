//! See [`Config`].

pub use self::builder::Builder;
use crate::checkers::history::last_paths::{Error as LastPathsError, LastPaths};
use crate::checkers::history::operation::{Error as HoardOperationError, HoardOperation};
use crate::checkers::Checker;
use crate::command::{Command, EditError};
use crate::hoard::iter::{HoardDiff, HoardFilesIter};
use crate::hoard::{self, Direction, Hoard};
use directories::ProjectDirs;
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub mod builder;

/// Get the project directories for this project.
#[must_use]
pub fn get_dirs() -> ProjectDirs {
    tracing::trace!("determining project default folders");
    ProjectDirs::from("com", "shadow53", "hoard")
        .expect("could not detect user home directory to place program files")
}

/// Errors that can occur while working with a [`Config`].
#[derive(Debug, Error)]
pub enum Error {
    /// Error occurred while backing up a hoard.
    #[error("failed to back up {name}: {error}")]
    Backup {
        /// The name of the hoard that failed to back up.
        name: String,
        /// The error that occurred.
        #[source]
        error: hoard::Error,
    },
    /// Error occurred while building the configuration.
    #[error("error while building the configuration: {0}")]
    Builder(#[from] builder::Error),
    /// Error occurred while editing the config file
    #[error("error while editing the config file: {0}")]
    Edit(#[from] EditError),
    /// The requested hoard does not exist.
    #[error("no such hoard is configured: {0}")]
    NoSuchHoard(String),
    /// Error occurred while restoring a hoard.
    #[error("failed to back up {name}: {error}")]
    Restore {
        /// The name of the hoard that failed to restore.
        name: String,
        /// The error that occurred.
        #[source]
        error: hoard::Error,
    },
    /// An error occurred while comparing paths for this run to the previous one.
    #[error("error while comparing previous run to current run: {0}")]
    LastPaths(#[from] LastPathsError),
    /// An error occurred while checking against remote operations.
    #[error("error while checking against recent remote operations: {0}")]
    Operation(#[from] HoardOperationError),
    /// An error occurred while cleaning up log files.
    #[error("error after cleaning up {success_count} log files: {error}")]
    Cleanup {
        /// The number of files successfully cleaned.
        success_count: u32,
        /// The error that occurred.
        #[source]
        error: crate::checkers::history::operation::Error,
    },
    /// An error occurred while diffing files.
    #[error("error while diffing files: {0}")]
    Diff(#[from] crate::hoard::iter::Error),
    /// An error occurred while creating the [`HoardFilesIter`].
    #[error("error creating file iterator: {0}")]
    Iterator(#[from] crate::filters::Error),
}

/// A (processed) configuration.
///
/// To create a configuration, use [`Builder`] instead.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// The command to run.
    pub command: Command,
    /// The root directory to backup/restore hoards from.
    hoards_root: PathBuf,
    /// Path to a configuration file.
    config_file: PathBuf,
    /// All of the configured hoards.
    hoards: HashMap<String, Hoard>,
    /// Whether to force the operation to continue despite possible inconsistencies.
    force: bool,
}

impl Default for Config {
    fn default() -> Self {
        tracing::trace!("creating default config");
        // Calling [`Builder::unset_hoards`] to ensure there is no panic
        // when `expect`ing
        Self::builder()
            .unset_hoards()
            .build()
            .expect("failed to create default config")
    }
}

impl Config {
    /// Create a new [`Builder`].
    #[must_use]
    pub fn builder() -> Builder {
        Builder::new()
    }

    /// Load a [`Config`] from CLI arguments and then configuration file.
    ///
    /// Alias for [`Builder::from_args_then_file`] that then builds the builder into
    /// a [`Config`].
    ///
    /// # Errors
    ///
    /// The error returned by [`Builder::from_args_then_file`], wrapped in [`Error::Builder`].
    pub fn load() -> Result<Self, Error> {
        tracing::debug!("loading configuration...");
        let config = Builder::from_args_then_file()
            .map(Builder::build)?
            .map_err(Error::Builder)?;
        tracing::debug!("loaded configuration.");
        Ok(config)
    }

    /// The path to the configured configuration file.
    #[must_use]
    pub fn get_config_file_path(&self) -> PathBuf {
        self.config_file.clone()
    }

    /// The path to the configured hoards root.
    #[must_use]
    pub fn get_hoards_root_path(&self) -> PathBuf {
        self.hoards_root.clone()
    }

    fn get_hoards<'a>(
        &'a self,
        hoards: &'a [String],
    ) -> Result<HashMap<&'a str, &'a Hoard>, Error> {
        if hoards.is_empty() {
            tracing::debug!("no hoard names provided, acting on all of them.");
            Ok(self
                .hoards
                .iter()
                .map(|(key, val)| (key.as_str(), val))
                .collect())
        } else {
            tracing::debug!("using hoard names provided on cli");
            tracing::trace!(?hoards);
            hoards
                .iter()
                .map(|key| self.get_hoard(key).map(|hoard| (key.as_str(), hoard)))
                .collect()
        }
    }

    #[must_use]
    fn get_prefix(&self, name: &str) -> PathBuf {
        self.hoards_root.join(name)
    }

    fn get_hoard<'a>(&'a self, name: &'_ str) -> Result<&'a Hoard, Error> {
        self.hoards
            .get(name)
            .ok_or_else(|| Error::NoSuchHoard(name.to_owned()))
    }

    #[allow(dead_code)]
    fn iter_hoard_files(&self, name: &str, direction: Direction) -> Result<HoardFilesIter, Error> {
        let hoard = self.get_hoard(name)?;
        let hoards_root = self.get_hoards_root_path();

        HoardFilesIter::new(&hoards_root, direction, name, hoard).map_err(Error::from)
    }

    fn hoard_file_diffs(&self, name: &str) -> Result<Vec<HoardDiff>, Error> {
        let hoard = self.get_hoard(name)?;
        let hoards_root = self.get_hoards_root_path();

        HoardFilesIter::file_diffs(&hoards_root, name, hoard).map_err(Error::from)
    }

    /// Run the stored [`Command`] using this [`Config`].
    ///
    /// # Errors
    ///
    /// Any [`enum@Error`] that might happen while running the command.
    pub fn run(&self) -> Result<(), Error> {
        #![allow(clippy::too_many_lines)]
        tracing::trace!(command = ?self.command, "running command");
        match &self.command {
            Command::Diff { hoard, verbose } => {
                for hoard_diff in self.hoard_file_diffs(hoard)? {
                    match hoard_diff {
                        HoardDiff::BinaryModified { path, diff_source } => {
                            tracing::info!("{}: binary file changed {}", path.display(), diff_source);
                        }
                        HoardDiff::TextModified { path, unified_diff, diff_source } => {
                            tracing::info!("{}: text file changed {}", path.display(), diff_source);
                            if *verbose {
                                tracing::info!("{}", unified_diff);
                            }
                        }
                        HoardDiff::PermissionsModified { path, hoard_perms, system_perms, diff_source } => {
                            #[cfg(unix)]
                            tracing::info!(
                                "{}: permissions changed {}: hoard ({:o}), system ({:o})",
                                path.display(),
                                diff_source,
                                hoard_perms.mode(),
                                system_perms.mode(),
                            );
                            #[cfg(not(unix))]
                            tracing::info!(
                                "{}: permissions changed {}: hoard ({}), system ({})",
                                path.display(),
                                diff_source,
                                if hoard_perms.readonly() { "readonly" } else { "writable" },
                                if system_perms.readonly() { "readonly" } else { "writable" },
                            );
                        }
                        HoardDiff::Created { path, diff_source } => {
                            tracing::info!("{}: created {}", path.display(), diff_source);
                        }
                        HoardDiff::Recreated { path, diff_source } => {
                            tracing::info!("{}: recreated {}", path.display(), diff_source);
                        }
                        HoardDiff::Deleted { path, diff_source } => {
                            tracing::info!("{}: deleted {}", path.display(), diff_source);
                        }
                    }
                }
            }
            Command::Edit => {
                if let Err(error) = crate::command::edit(&self.config_file) {
                    tracing::error!(%error, "error while editing config file");
                    return Err(Error::Edit(error));
                }
            }
            Command::Validate => {
                tracing::info!("configuration is valid");
            }
            Command::List => {
                let mut hoards: Vec<&str> = self.hoards.keys().map(String::as_str).collect();
                hoards.sort_unstable();
                let list = hoards.join("\n");
                tracing::info!("{}", list);
            }
            Command::Cleanup => match crate::checkers::history::operation::cleanup_operations() {
                Ok(count) => tracing::info!("cleaned up {} log files", count),
                Err((count, error)) => {
                    return Err(Error::Cleanup {
                        success_count: count,
                        error,
                    });
                }
            },
            Command::Backup { hoards } | Command::Restore { hoards } => {
                let hoards = self.get_hoards(hoards)?;
                let direction = match self.command {
                    Command::Backup { .. } => Direction::Backup,
                    Command::Restore { .. } => Direction::Restore,
                    // Only Command::Backup and Command::Restore should be possible
                    _ => return Ok(()),
                };

                let mut checkers = Checkers::new(&hoards, direction)?;
                if !self.force {
                    checkers.check()?;
                }

                for (name, hoard) in hoards {
                    let prefix = self.get_prefix(name);

                    match direction {
                        Direction::Backup => {
                            tracing::info!(hoard = %name, "backing up");
                            let _span = tracing::info_span!("backup", hoard = %name).entered();
                            hoard.backup(&prefix).map_err(|error| Error::Backup {
                                name: name.to_string(),
                                error,
                            })?;
                        }
                        Direction::Restore => {
                            tracing::info!(hoard = %name, "restoring");
                            let _span = tracing::info_span!("restore", hoard = %name).entered();
                            hoard.restore(&prefix).map_err(|error| Error::Restore {
                                name: name.to_string(),
                                error,
                            })?;
                        }
                    }
                }

                checkers.commit_to_disk()?;
            }
        }

        Ok(())
    }
}

struct Checkers {
    last_paths: HashMap<String, LastPaths>,
    operations: HashMap<String, HoardOperation>,
}

impl Checkers {
    fn new(hoard_map: &HashMap<&str, &Hoard>, direction: Direction) -> Result<Self, Error> {
        let mut last_paths = HashMap::new();
        let mut operations = HashMap::new();

        for (name, hoard) in hoard_map {
            let lp = LastPaths::new(name, hoard, direction)?;
            let op = HoardOperation::new(name, hoard, direction)?;
            last_paths.insert((*name).to_string(), lp);
            operations.insert((*name).to_string(), op);
        }

        Ok(Self {
            last_paths,
            operations,
        })
    }

    fn check(&mut self) -> Result<(), Error> {
        let _span = tracing::info_span!("running_checks").entered();
        for last_path in &mut self.last_paths.values_mut() {
            last_path.check()?;
        }
        for operation in self.operations.values_mut() {
            operation.check()?;
        }
        Ok(())
    }

    fn commit_to_disk(self) -> Result<(), Error> {
        let Self {
            last_paths,
            operations,
            ..
        } = self;
        for (_, last_path) in last_paths {
            last_path.commit_to_disk()?;
        }
        for (_, operation) in operations {
            operation.commit_to_disk()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_builder_returns_new_builder() {
        assert_eq!(
            Config::builder(),
            Builder::new(),
            "Config::builder should return an unmodified new Builder"
        );
    }

    #[test]
    fn test_config_default_builds_from_new_builder() {
        assert_eq!(
            Some(Config::default()),
            Builder::new().build().ok(),
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
