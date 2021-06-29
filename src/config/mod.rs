//! See [`Config`].

pub use self::builder::Builder;
use self::hoard::Hoard;
use crate::command::Command;
use crate::history::last_paths::{Error as LastPathsError, HoardPaths, LastPaths};
use directories::ProjectDirs;
use std::collections::BTreeMap;
use std::path::PathBuf;
use thiserror::Error;

pub mod builder;
pub mod hoard;

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
    hoards: BTreeMap<String, Hoard>,
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
        tracing::info!("loading configuration...");
        let config = Builder::from_args_then_file()
            .map(Builder::build)?
            .map_err(Error::Builder)?;
        tracing::info!("loaded configuration.");
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

    #[must_use]
    fn get_hoards<'a>(&'a self, hoards: &'a [String]) -> Vec<&'a str> {
        if hoards.is_empty() {
            tracing::debug!("no hoard names provided, acting on all of them.");
            self.hoards.keys().map(String::as_str).collect()
        } else {
            tracing::debug!("using hoard names provided on cli");
            tracing::trace!(?hoards);
            hoards.iter().map(String::as_str).collect()
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

    /// Checks for inconsistencies between previous run and current run's paths.
    ///
    /// Returns error if inconsistencies are found. If none are found or `self.force == true`,
    /// the paths are overwritten in `last_paths` and persisted to disk.
    ///
    /// This function should be run *before* doing any file operations on a hoard. It persists the
    /// paths used before the fallible file operations to prevent confusion (I would expect the
    /// "last paths used" file to have the paths that caused the error, not the run before).
    fn check_and_set_same_paths(
        &self,
        name: &str,
        last_paths: &mut LastPaths,
    ) -> Result<(), Error> {
        let _span = tracing::info_span!("checking for inconsistencies against previous operation", hoard=%name).entered();
        let hoard_paths = self.get_hoard(name)?.get_paths();
        // If forcing the operation, don't bother checking.
        if !self.force {
            // Previous paths being none means we've never worked with this hoard before.
            if let Some(prev_paths) = last_paths.hoard(name) {
                // This function logs any warnings from differences.
                HoardPaths::enforce_old_and_new_piles_are_same(prev_paths, &hoard_paths)?;
            }
        }

        // If no error occurred, set to new paths
        last_paths.set_hoard(name.to_string(), hoard_paths);
        // Then save to disk.
        last_paths.save_to_disk()?;

        Ok(())
    }

    /// Run the stored [`Command`] using this [`Config`].
    ///
    /// # Errors
    ///
    /// Any [`enum@Error`] that might happen while running the command.
    pub fn run(&self) -> Result<(), Error> {
        tracing::trace!(command = ?self.command, "running command");
        match &self.command {
            Command::Validate => {
                tracing::info!("configuration is valid")
            }
            Command::Backup { hoards } => {
                let hoards = self.get_hoards(&hoards);
                let mut last_paths = LastPaths::from_default_file()?;
                for name in hoards {
                    self.check_and_set_same_paths(name, &mut last_paths)?;
                    let prefix = self.get_prefix(name);
                    let hoard = self.get_hoard(name)?;

                    tracing::info!(hoard = %name, "backing up hoard");
                    let _span = tracing::info_span!("backup", hoard = %name).entered();
                    hoard.backup(&prefix).map_err(|error| Error::Backup {
                        name: name.to_string(),
                        error,
                    })?;
                }
            }
            Command::Restore { hoards } => {
                let hoards = self.get_hoards(&hoards);
                let mut last_paths = LastPaths::from_default_file()?;
                for name in hoards {
                    self.check_and_set_same_paths(name, &mut last_paths)?;
                    let prefix = self.get_prefix(name);
                    let hoard = self.get_hoard(name)?;

                    tracing::info!(hoard = %name, "restoring hoard");
                    let _span = tracing::info_span!("restore", hoard = %name).entered();
                    hoard.restore(&prefix).map_err(|error| Error::Restore {
                        name: name.to_string(),
                        error,
                    })?;
                }
            }
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
