//! See [`Config`].

pub use self::builder::Builder;
use crate::command::{self, Command};
use crate::hoard::{self, Hoard};
use crate::newtypes::HoardName;
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

pub mod builder;

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
    NoSuchHoard(HoardName),
}

/// A (processed) configuration.
///
/// To create a configuration, use [`Builder`] instead.
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// The command to run.
    pub command: Command,
    /// Path to a configuration file.
    pub config_file: PathBuf,
    /// All of the configured hoards.
    pub hoards: HashMap<HoardName, Hoard>,
    /// Whether to force the operation to continue despite possible inconsistencies.
    pub force: bool,
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
    pub async fn load() -> Result<Self, Error> {
        tracing::debug!("loading configuration...");
        let config = Builder::from_args_then_file()
            .await
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

    fn get_hoards<'a>(
        &'a self,
        hoards: &'a [HoardName],
    ) -> Result<HashMap<&'a HoardName, &'a Hoard>, Error> {
        if hoards.is_empty() {
            tracing::debug!("no hoard names provided, acting on all of them.");
            Ok(self.hoards.iter().collect())
        } else {
            tracing::debug!("using hoard names provided on cli");
            tracing::trace!(?hoards);
            hoards
                .iter()
                .map(|key| self.get_hoard(key).map(|hoard| (key, hoard)))
                .collect()
        }
    }

    fn get_hoard<'a>(&'a self, name: &'_ HoardName) -> Result<&'a Hoard, Error> {
        self.hoards
            .get(name)
            .ok_or_else(|| Error::NoSuchHoard(name.clone()))
    }

    /// Run the stored [`Command`] using this [`Config`].
    ///
    /// # Errors
    ///
    /// Any [`enum@Error`] that might happen while running the command.
    pub async fn run(&self) -> Result<(), Error> {
        tracing::trace!(command = ?self.command, "running command");
        match &self.command {
            Command::Status => {
                let iter = self.hoards.iter();
                command::run_status(&crate::paths::hoards_dir(), iter).await?;
            }
            Command::Diff { hoard, verbose } => {
                command::run_diff(
                    self.get_hoard(hoard)?,
                    hoard,
                    &crate::paths::hoards_dir(),
                    *verbose,
                )
                .await?;
            }
            Command::Edit => {
                command::run_edit(&self.config_file).await?;
            }
            Command::Validate => {
                tracing::info!("configuration is valid");
            }
            Command::List => {
                command::run_list(self.hoards.keys());
            }
            Command::Cleanup => {
                command::run_cleanup().await?;
            }
            Command::Backup { hoards } => {
                let data_dir = crate::paths::hoards_dir();
                let hoards = self.get_hoards(hoards)?;
                command::run_backup(&data_dir, hoards, self.force).await?;
            }
            Command::Restore { hoards } => {
                let data_dir = crate::paths::hoards_dir();
                let hoards = self.get_hoards(hoards)?;
                command::run_restore(&data_dir, hoards, self.force).await?;
            }
            Command::Upgrade => {
                command::run_upgrade().await?;
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
}
