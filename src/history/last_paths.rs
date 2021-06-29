//! Types dedicated to recording which paths were used for each pile in the last
//! operating using a given hoard.
//!
//! See the documentation for [`HoardPaths::enforce_old_and_new_piles_are_same`] for an
//! explanation of why this is useful.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::{fs, io};
use thiserror::Error;

const FILE_NAME: &str = "last_paths.json";

/// Errors that may occur while working with a [`LastPaths`] or related types.
#[derive(Debug, Error)]
pub enum Error {
    /// An error while parsing the JSON file.
    #[error("could not parse {}: {0}", FILE_NAME)]
    Serde(#[from] serde_json::Error),
    /// An error while doing I/O.
    #[error("an I/O error occurred: {0}")]
    IO(#[from] io::Error),
    /// Unexpected differences in hoard paths. Operation must be forced to continue.
    #[error("paths used in current hoard operation do not match previous run")]
    HoardPathsMismatch,
}

/// Collection of the last paths matched per hoard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LastPaths(HashMap<String, HoardPaths>);

fn get_last_paths_file_path() -> Result<PathBuf, io::Error> {
    tracing::debug!("getting lastpaths file path");
    let id = super::get_or_generate_uuid()?;
    Ok(super::get_history_dir_for_id(id).join(FILE_NAME))
}

fn read_last_paths_file() -> Result<fs::File, io::Error> {
    let path = get_last_paths_file_path()?;
    tracing::debug!(?path, "opening lastpaths file at path");
    fs::File::open(path)
}

fn save_last_paths_to_file(paths: &LastPaths) -> Result<(), Error> {
    tracing::debug!("saving lastpaths to disk");
    let path = get_last_paths_file_path()?;
    tracing::trace!("converting lastpaths to JSON");
    let content = serde_json::to_string(paths)?;
    if let Some(parent) = path.parent() {
        tracing::trace!("ensuring parent directories exist");
        fs::create_dir_all(parent)?;
    }
    tracing::trace!("writing lastpaths file");
    fs::write(path, content)?;
    Ok(())
}

impl LastPaths {
    /// Create a new, empty `LastPaths`.
    #[must_use]
    pub fn new() -> Self {
        Self(HashMap::new())
    }

    /// Get the entry for the given hoard, if exists.
    #[must_use]
    pub fn hoard(&self, hoard: &str) -> Option<&HoardPaths> {
        self.0.get(hoard)
    }

    /// Set/overwrite the paths used for the given hoard.
    pub fn set_hoard(&mut self, hoard: String, paths: HoardPaths) {
        self.0.insert(hoard, paths);
    }

    /// Read the last paths from the default file.
    ///
    /// # Errors
    ///
    /// Any I/O or `serde` error that occurs while reading and parsing the file.
    /// The exception is an I/O error with kind `NotFound`, which returns an empty
    /// `LastPaths`.
    pub fn from_default_file() -> Result<Self, Error> {
        tracing::debug!("reading lastpaths from file");
        let reader = match read_last_paths_file() {
            Ok(file) => file,
            Err(err) => {
                if err.kind() == io::ErrorKind::NotFound {
                    tracing::debug!("lastpaths file not found, creating new instance");
                    return Ok(Self::new());
                }
                tracing::error!(error=%err);
                return Err(err.into());
            }
        };

        serde_json::from_reader(reader).map_err(Into::into)
    }

    /// Save this `LastPaths` to the `last_paths` file.
    ///
    /// # Errors
    ///
    /// Any I/O or `serde` error that occurs while saving this `LastPaths`.
    pub fn save_to_disk(&self) -> Result<(), Error> {
        save_last_paths_to_file(self)
    }
}

impl Default for LastPaths {
    fn default() -> Self {
        Self::new()
    }
}

/// An entry for the last time a hoard was processed.
///
/// Contains the timestamp of the last operation on this hoard and a mapping
/// of every file in each of its piles to the corresponding path outside of the
/// hoard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HoardPaths {
    timestamp: chrono::DateTime<chrono::Utc>,
    piles: PilePaths,
}

/// Internal type for [`HoardPaths`] mapping to anonymous or named piles.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum PilePaths {
    /// A single, anonymous pile's path.
    Anonymous(Option<PathBuf>),
    /// One or more named piles and their paths.
    Named(HashMap<String, PathBuf>),
}

impl From<PathBuf> for PilePaths {
    fn from(other: PathBuf) -> Self {
        Self::Anonymous(Some(other))
    }
}

impl From<Option<PathBuf>> for PilePaths {
    fn from(other: Option<PathBuf>) -> Self {
        Self::Anonymous(other)
    }
}

impl From<HashMap<String, PathBuf>> for PilePaths {
    fn from(other: HashMap<String, PathBuf>) -> Self {
        Self::Named(other)
    }
}

impl<T> From<T> for HoardPaths
where
    T: Into<PilePaths>,
{
    fn from(val: T) -> Self {
        Self {
            timestamp: chrono::offset::Utc::now(),
            piles: val.into(),
        }
    }
}

impl HoardPaths {
    /// Get the timestamp of the last operation on this hoard.
    #[must_use]
    pub fn time(&self) -> &chrono::DateTime<chrono::Utc> {
        &self.timestamp
    }

    /// Get the entries for a pile by name.
    ///
    /// Returns `None` if the named pile is not found or if the hoard contains an
    /// anonymous pile.
    #[must_use]
    pub fn named_pile(&self, name: &str) -> Option<&PathBuf> {
        if let PilePaths::Named(named) = &self.piles {
            named.get(name)
        } else {
            None
        }
    }

    /// Get the entries for the anonymous pile.
    ///
    /// Returns `None` if the hoard contains named piles.
    #[must_use]
    pub fn anonymous_pile(&self) -> Option<&PathBuf> {
        if let PilePaths::Anonymous(path) = &self.piles {
            path.as_ref()
        } else {
            None
        }
    }

    /// Logs any inconsistencies and returns an error if any are found.
    ///
    /// This check basically returns an error if `old != new`, but does some extra checking to
    /// provided better logged warnings depending on what the mismatch is.
    ///
    /// Any mismatch is considered an inconsistency because it may mean that one of the following
    /// occurs because of an unexpected change in the system configuration:
    ///
    /// - Backing up an empty/non-existent directory unintentionally deletes the current backup.
    /// - Restoring to an unexpected directory causes nothing to change in the intended one.
    /// - Files don't get backed up or restored because the system configuration did not match a
    ///   pile's environment string.
    ///
    /// # Example
    ///
    /// I sometimes run a normal Steam installation on a Linux machine, sometimes the flatpak version.
    /// If the flatpak install takes priority over the normal install, the following set of events
    /// might occur:
    ///
    /// - Install flatpak Steam to try it out.
    /// - Realize I should back up my normal Steam saves so I can restore them to the flatpak locations.
    ///   - Alternatively, I told `hoard` to do this already and assumed it finished, but a different
    ///     hoard took longer than expected to back up.
    ///   - Either way, a backup happens after installing flatpak Steam.
    /// - Existing backup is erased so the new one is a clean backup.
    /// - The associated directories for flatpak are empty or don't exist, so nothing gets backed up.
    /// - My backups are all deleted.
    ///
    /// There are ways to recover from this, for example by uninstalling flatpak Steam and doing
    /// another backup, but the situation gets more complex when considering multiple devices
    /// each synchronizing files to each other. It's much easier to make this check and, if the
    /// changes are intended, have the user indicate as much.
    ///
    /// # Errors
    ///
    /// [`Error::HoardPathsMismatch`] if there is a difference between `old` and `new`.
    pub fn enforce_old_and_new_piles_are_same(old: &Self, new: &Self) -> Result<(), Error> {
        tracing::debug!("comparing old and new piles' paths");
        tracing::trace!(?old, ?new);
        match (&old.piles, &new.piles) {
            (PilePaths::Anonymous(old), PilePaths::Anonymous(new)) => {
                tracing::trace!("both piles are anonymous");
                if old.is_some() && new.is_none() {
                    tracing::warn!(old_path=?old, "anonymous pile no longer has a path");
                    return Err(Error::HoardPathsMismatch);
                } else if old.is_none() && new.is_some() {
                    // TODO: This case may be necessary when restoring: consider.
                    tracing::warn!(new_path=?new, "anonymous pile matches a path but previously did not");
                    return Err(Error::HoardPathsMismatch);
                } else if let (Some(old), Some(new)) = (old, new) {
                    // If both are None, they are the same. So check only for both as Some(_).
                    // Then check if the paths match.
                    if old != new {
                        tracing::warn!(?old, ?new, "anonymous pile path changed");
                        return Err(Error::HoardPathsMismatch);
                    }
                }

                return Err(Error::HoardPathsMismatch);
            }
            (PilePaths::Anonymous(_), PilePaths::Named(_)) => {
                tracing::warn!("hoard previously with anonymous pile now has named pile(s)");
                return Err(Error::HoardPathsMismatch);
            }
            (PilePaths::Named(_), PilePaths::Anonymous(_)) => {
                tracing::warn!("hoard previously with named pile(s) now has an anonymous pile");
                return Err(Error::HoardPathsMismatch);
            }
            (PilePaths::Named(old), PilePaths::Named(new)) => {
                tracing::trace!("both piles are named");
                let old_set: HashSet<&str> = old.keys().map(String::as_str).collect();
                let new_set: HashSet<&str> = new.keys().map(String::as_str).collect();

                let only_in_old: Vec<&str> = old_set.difference(&new_set).copied().collect();
                let only_in_new: Vec<&str> = new_set.difference(&old_set).copied().collect();

                // Warn about both before returning.
                if !only_in_old.is_empty() {
                    tracing::warn!(piles=?only_in_old, "named piles previously with path no longer have a path");
                }
                if !only_in_new.is_empty() {
                    tracing::warn!(piles=?only_in_new, "named piles previously without path now have a path");
                }

                // Now return if either difference occurred.
                if !only_in_old.is_empty() || !only_in_new.is_empty() {
                    return Err(Error::HoardPathsMismatch);
                }

                // If all of the same piles exist, check if all the paths are the same.
                // Can expect because the above checks for any mismatched keys.
                let mut mismatch = false;
                for (key, old_path) in old {
                    let new_path = new.get(key).expect("key should exist in map");
                    if old_path != new_path {
                        mismatch = true;
                        tracing::warn!(
                            ?old_path,
                            ?new_path,
                            "pile \"{}\" has a different path",
                            key
                        );
                    }
                }

                if mismatch {
                    return Err(Error::HoardPathsMismatch);
                }
            }
        }

        Ok(())
    }
}
