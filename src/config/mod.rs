//! See [`Config`].

pub use self::builder::Builder;
use crate::command::{self, Command};
use crate::hoard::{self, Hoard};
use directories::ProjectDirs;
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

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
    /// Error while running a [`Command`].
    #[error("command failed: {0}")]
    Command(#[from] command::Error),
    /// Error occurred while building the configuration.
    #[error("error while building the configuration: {0}")]
    Builder(#[from] builder::Error),
    /// The requested hoard does not exist.
    #[error("no such hoard is configured: {0}")]
    NoSuchHoard(String),
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

    fn get_hoard<'a>(&'a self, name: &'_ str) -> Result<&'a Hoard, Error> {
        self.hoards
            .get(name)
            .ok_or_else(|| Error::NoSuchHoard(name.to_owned()))
    }

    /// Run the stored [`Command`] using this [`Config`].
    ///
    /// # Errors
    ///
    /// Any [`enum@Error`] that might happen while running the command.
    pub fn run(&self) -> Result<(), Error> {
        tracing::trace!(command = ?self.command, "running command");
        match &self.command {
            Command::Status => {
                let iter = self
                    .hoards
                    .iter()
                    .map(|(name, hoard)| (name.as_str(), hoard));
                command::run_status(&self.get_hoards_root_path(), iter)?;
            }
            Command::Diff { hoard, verbose } => {
                command::run_diff(
                    self.get_hoard(hoard)?,
                    hoard,
                    &self.get_hoards_root_path(),
                    *verbose,
                )?;
            }
            Command::Edit => {
                command::run_edit(&self.config_file)?;
            }
            Command::Validate => {
                tracing::info!("configuration is valid");
            }
            Command::List => {
                command::run_list(self.hoards.keys().map(String::as_str));
            }
            Command::Cleanup => {
                command::run_cleanup()?;
            }
            Command::Backup { hoards } => {
                let hoards_root = self.get_hoards_root_path();
                let hoards = self.get_hoards(hoards)?;
                command::run_backup(&hoards_root, hoards, self.force)?;
            }
            Command::Restore { hoards } => {
                let hoards_root = self.get_hoards_root_path();
                let hoards = self.get_hoards(hoards)?;
                command::run_restore(&hoards_root, hoards, self.force)?;
            }
            Command::Upgrade => {
                command::run_upgrade()?;
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
