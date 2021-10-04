//! Keeping track of all operations.
//!
//! The types in this module are used for logging all operations to disk. This information can be
//! used for debugging purposes, but is more directly used as a [`Checker`] to help prevent
//! synchronized changes from being overwritten.
//!
//! It does this by parsing synchronized logs from this and other systems to determine which system
//! was the last one to touch a file.

use crate::checkers::Checker;
use crate::config::hoard::{Hoard as ConfigHoard, Pile as ConfigPile};
use md5::{Digest, Md5};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::{fs, io};
use thiserror::Error;
use uuid::Uuid;

const TIME_FORMAT_STR: &str = "%Y_%m_%d-%H_%M_%S%.6f";
static LOG_FILE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new("^[0-9]{4}(_[0-9]{2}){2}-([0-9]{2}_){2}([0-9]{2})\\.[0-9]{6}\\.log$")
        .expect("invalid log file regex")
});

/// Errors that may occur while working with operation logs.
#[derive(Debug, Error)]
pub enum Error {
    /// Any I/O error.
    #[error("an I/O error occurred: {0}")]
    IO(#[from] io::Error),
    /// An error occurred while (de)serializing with `serde`.
    #[error("a (de)serialization error occurred: {0}")]
    Serde(#[from] serde_json::Error),
    /// There are unapplied changes from another system. A restore or forced backup is required.
    #[error("found unapplied remote changes - restore this hoard to apply changes or force a backup with --force")]
    RestoreRequired,
}

/// A single operation log.
///
/// This keeps track of the timestamp of the operation (which may include multiple hoards),
/// all hoards involved in the operation (and the related [`HoardOperation`]), and a record
/// of the latest operation log for each external system at the time of invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::module_name_repetitions)]
pub struct HoardOperation {
    /// Timestamp of last operation
    timestamp: chrono::DateTime<chrono::Utc>,
    /// Whether this operation was a backup
    is_backup: bool,
    /// The name of the hoard for this `HoardOperation`.
    hoard_name: String,
    /// Mapping of pile files to checksums
    hoard: Hoard,
}

impl Checker for HoardOperation {
    type Error = Error;

    fn new(name: &str, hoard: &ConfigHoard, is_backup: bool) -> Result<Self, Self::Error> {
        Ok(Self {
            timestamp: chrono::Utc::now(),
            is_backup,
            hoard_name: name.into(),
            hoard: Hoard::try_from(hoard)?,
        })
    }

    fn check(&mut self) -> Result<(), Self::Error> {
        let _span =
            tracing::debug_span!("is_pending_operation_valid", hoard=%self.hoard_name).entered();
        tracing::debug!("checking if the hoard operation is safe to perform");
        let last_local = Self::latest_local(&self.hoard_name)?;
        let last_remote = Self::latest_remote_backup(&self.hoard_name)?;

        if !self.is_backup {
            tracing::debug!("not backing up, is safe to continue");
            return Ok(());
        }

        match (last_local, last_remote) {
            (_, None) => {
                tracing::debug!("no remote operations found for hoard, is safe to continue");
                Ok(())
            }
            (None, Some(last_remote)) => {
                tracing::debug!("no local operations found, is not safe to continue");
                self.check_has_same_files(&last_remote)
            }
            (Some(last_local), Some(last_remote)) => {
                if last_local.timestamp > last_remote.timestamp {
                    // Allow if the last operation on this machine
                    tracing::debug!(
                        local=%last_local.timestamp,
                        remote=%last_remote.timestamp,
                        "latest local operation is more recent than last remote operation"
                    );
                    Ok(())
                } else {
                    tracing::debug!(
                        local=%last_local.timestamp,
                        remote=%last_remote.timestamp,
                        "latest local operation is less recent than last remote operation"
                    );
                    self.check_has_same_files(&last_remote)
                }
            }
        }
    }

    fn commit_to_disk(self) -> Result<(), Self::Error> {
        let _span =
            tracing::trace_span!("commit_operation_to_disk", hoard=%self.hoard_name).entered();
        let id = super::get_or_generate_uuid()?;
        let path = super::get_history_dir_for_id(id)
            .join(&self.hoard_name)
            .join(format!("{}.log", self.timestamp.format(TIME_FORMAT_STR)));
        tracing::trace!(path=%path.display(), "ensuring parent directories for operation log file");
        path.parent().map(fs::create_dir_all).transpose()?;
        let file = fs::File::create(path)?;
        serde_json::to_writer(file, &self)?;
        Ok(())
    }
}

impl HoardOperation {
    fn file_is_log(path: &Path) -> bool {
        let _span = tracing::trace_span!("file_is_log", ?path).entered();
        let result = path.is_file()
            && match path.file_name() {
                None => false, // grcov: ignore
                Some(name) => match name.to_str() {
                    None => false, // grcov: ignore
                    Some(name) => LOG_FILE_REGEX.is_match(name),
                },
            };
        tracing::trace!(result, "determined if file is operation log");
        result
    }

    /// Checks if files in both operations are the same.
    ///
    /// # Errors
    ///
    /// Returns [`Error::RestoreRequired`] if they do not have the same files (and hashes).
    pub fn check_has_same_files(&self, other: &Self) -> Result<(), Error> {
        (self.hoard == other.hoard)
            .then(|| ())
            .ok_or(Error::RestoreRequired)
    }

    fn from_file(path: &Path) -> Result<Self, Error> {
        tracing::trace!(path=%path.display(), "loading operation log from path");
        fs::File::open(path)
            .map(serde_json::from_reader)
            .map_err(Error::from)?
            .map_err(Error::from)
    }

    /// Returns the latest operation for the given hoard from a system history root directory.
    fn latest_hoard_operation_from_system_dir(
        dir: &Path,
        hoard: &str,
        backups_only: bool,
    ) -> Result<Option<Self>, Error> {
        let _span = tracing::trace_span!("get_latest_hoard_operation", ?dir, %hoard).entered();
        tracing::trace!("getting latest operation log for hoard in dir");
        let root = dir.join(hoard);
        if !root.exists() {
            tracing::trace!(dir=?root, "hoard dir does not exist, no logs found");
            return Ok(None);
        }

        root.read_dir()?
            .filter_map(|item| {
                // Only keep errors and anything where path() returns Some
                item.map(|item| {
                    let path = item.path();
                    Self::file_is_log(&path).then(|| path)
                })
                .transpose()
            })
            .map(|path| -> Result<HoardOperation, Error> {
                path.map(|path| HoardOperation::from_file(&path))?
            })
            .filter_map(|operation| match operation {
                Err(err) => Some(Err(err)), // grcov: ignore
                Ok(operation) => (!backups_only || operation.is_backup).then(|| Ok(operation)),
            })
            .reduce(|left, right| {
                let left = left?;
                let right = right?;
                if left.timestamp > right.timestamp {
                    Ok(left)
                } else {
                    Ok(right)
                }
            })
            .transpose()
    }

    /// Returns the latest operation recorded on this machine (by UUID).
    ///
    /// # Errors
    ///
    /// - Any errors that occur while reading from the filesystem
    /// - Any parsing errors from `serde_json` when parsing the file
    pub fn latest_local(hoard: &str) -> Result<Option<Self>, Error> {
        let _span = tracing::debug_span!("latest_local", %hoard).entered();
        tracing::debug!("finding latest Operation file for this machine");
        let uuid = super::get_or_generate_uuid()?;
        let self_folder = super::get_history_dir_for_id(uuid);
        Self::latest_hoard_operation_from_system_dir(&self_folder, hoard, false)
    }

    /// Returns the latest backup operation recorded on any other machine (by UUID).
    ///
    /// # Errors
    ///
    /// - Any errors that occur while reading from the filesystem
    /// - Any parsing errors from `serde_json` when parsing the file
    pub fn latest_remote_backup(hoard: &str) -> Result<Option<Self>, Error> {
        let _span = tracing::debug_span!("latest_remote_backup").entered();
        tracing::debug!("finding latest Operation file from remote machines");
        let uuid = super::get_or_generate_uuid()?;
        let other_folders = super::get_history_dirs_not_for_id(&uuid)?;
        other_folders
            .into_iter()
            .map(|dir| Self::latest_hoard_operation_from_system_dir(&dir, hoard, true))
            .reduce(|left, right| match (left?, right?) {
                (Some(left), None) => Ok(Some(left)),
                (None, Some(right)) => Ok(Some(right)),
                (None, None) => Ok(None),
                (Some(left), Some(right)) => {
                    if left.timestamp.timestamp() > right.timestamp.timestamp() {
                        Ok(Some(left))
                    } else {
                        Ok(Some(right))
                    }
                }
            })
            .transpose()
            .map(Option::flatten)
    }
}

/// Operation log information for a single hoard.
///
/// Really just a wrapper for [`Pile`] because piles may be anonymous or named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Hoard {
    /// Information for a single, anonymous pile.
    Anonymous(Pile),
    /// Information for some number of named piles.
    Named(HashMap<String, Pile>),
}

impl TryFrom<&ConfigHoard> for Hoard {
    type Error = Error;
    fn try_from(hoard: &ConfigHoard) -> Result<Self, Self::Error> {
        let _span = tracing::trace_span!("hoard_to_operation", ?hoard).entered();
        match hoard {
            ConfigHoard::Anonymous(pile) => Pile::try_from(pile).map(Hoard::Anonymous),
            ConfigHoard::Named(map) => map
                .piles
                .iter()
                .map(|(key, pile)| Pile::try_from(pile).map(|pile| (key.clone(), pile)))
                .collect::<Result<HashMap<_, _>, _>>()
                .map(Hoard::Named),
        }
    }
}

/// Enum to differentiate between different types of checksum.
///
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Checksum {
    /// An MD5 checksum. Fast but may have collisions.
    #[serde(rename = "md5")]
    MD5(String),
}

/// A mapping of file path (relative to pile) to file checksum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pile(HashMap<PathBuf, String>);

fn hash_path(path: &Path, root: &Path) -> Result<HashMap<PathBuf, String>, Error> {
    let mut map = HashMap::new();
    if path.is_file() {
        tracing::trace!(file=%path.display(), "Hashing file");
        let bytes = fs::read(path)?;
        let digest = Md5::digest(&bytes);
        let rel_path = path
            .strip_prefix(root)
            .expect("paths in hash_path should always be children of the given root")
            .to_path_buf();
        map.insert(rel_path, format!("{:x}", digest));
    } else if path.is_dir() {
        tracing::trace!(dir=%path.display(), "Hashing all files in dir");
        for item in fs::read_dir(path)? {
            let item = item?;
            let path = item.path();
            map.extend(hash_path(&path, root)?);
        }
    } else {
        tracing::warn!(path=%path.display(), "path is neither file nor directory, skipping");
    }

    Ok(map)
}

impl TryFrom<&ConfigPile> for Pile {
    type Error = Error;
    fn try_from(pile: &ConfigPile) -> Result<Self, Self::Error> {
        let _span = tracing::trace_span!("pile_to_operation", ?pile).entered();
        pile.path.as_ref().map_or_else(
            || Ok(Self(HashMap::new())),
            |path| hash_path(path, path).map(Self),
        )
    }
}

/// Cleans up residual operation logs, leaving the latest per (system, hoard) pair.
///
/// Technically speaking, this function may leave up to two log files behind per pair.
/// If the most recent log file is for a *restore* operation, the most recent *backup* will
/// also be retained. If the most recent log file is a *backup*, it will be the only one
/// retained.
///
/// # Errors
///
/// - Any I/O error from working with and deleting multiple files
/// - Any [`Error`]s from parsing files to determine whether or not to keep them
pub fn cleanup_operations() -> Result<u32, (u32, Error)> {
    // Get hoard history root
    // Iterate over every uuid in the directory
    let root = super::get_history_root_dir();
    fs::read_dir(root)
        .map_err(|err| (0, err.into()))?
        .filter(|entry| {
            entry.as_ref().map_or_else(
                // Propagate errors
                |_err| true,
                // Keep only entries that are directories with UUIDs for names
                |entry| {
                    tracing::trace!("checking if {} is a system directory", entry.path().display());
                    entry.path().is_dir()
                        && entry
                            .file_name()
                            .to_str()
                            .map_or_else(|| false, |s| {
                                tracing::trace!("checking if {} is a valid UUID", s);
                                Uuid::parse_str(s).is_ok()
                            })
                },
            )
        })
        // For each system folder, make a list of all log files, excluding 1 or 2 to keep.
        .map(|entry| {
            let entry = entry?;
            let hoards = fs::read_dir(entry.path())?
                .map(|entry| entry.map(|entry| {
                    let path = entry.path();
                    tracing::trace!("found hoard directory: {}", path.display());
                    path
                }))
                .collect::<Result<Vec<_>, _>>()?;

            // List all log files in a hoard folder for the current iterated system
            hoards.into_iter()
                // Filter out last_paths.json
                .filter(|path| path.is_dir())
                .map(|path| {
                tracing::trace!("checking files in directory: {}", path.display());
                let mut files: Vec<PathBuf> = fs::read_dir(path)?
                    .filter_map(|subentry| {
                        subentry
                            .map(|subentry| {
                                tracing::trace!("checking if {} is a log file", subentry.path().display());
                                HoardOperation::file_is_log(&subentry.path()).then(|| subentry.path())
                            })
                            .map_err(Error::from)
                            .transpose()
                    })
                    .collect::<Result<_, Error>>()?;

                files.sort_unstable();

                // The last item is the latest operation for this hoard, so keep it.
                let recent = files.pop();

                // Make sure the most recent backup is (also) retained.
                if let Some(recent) = recent {
                    let recent = HoardOperation::from_file(&recent)?;
                    if !recent.is_backup {
                        tracing::trace!("most recent log is not a backup, making sure to retain a backup log too");
                        // Find the index of the latest backup
                        let index = files
                            .iter()
                            .enumerate()
                            .rev()
                            .find_map(|(i, path)| {
                                HoardOperation::from_file(path)
                                    .map(|op| op.is_backup.then(|| i))
                                    .transpose()
                            })
                            .transpose()?;

                        if let Some(index) = index {
                            // Found index of latest backup, remove it from deletion list
                            files.remove(index);
                        }
                    }
                } // grcov: ignore

                Ok(files)
            }).collect::<Result<Vec<_>, _>>()
        })
        // Collect a list of all files to delete for each system directory.
        .collect::<Result<Vec<Vec<Vec<PathBuf>>>, _>>()
        .map_err(|err| (0, err))?
        .into_iter()
        // Flatten the list of lists into a single list.
        .flatten()
        .flatten()
        // Delete each file.
        .map(|path| {
            tracing::trace!("deleting {}", path.display());
            fs::remove_file(path)
        })
        // Return the first error or the number of files deleted.
        .fold(Ok((0, ())), |acc, res2| {
            let (count, _) = acc?;
            Ok((count + 1, res2.map_err(|err| (count, err.into()))?))
        })
        .map(|(count, _)| count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_test::{assert_tokens, Token};

    #[test]
    fn test_checksum_derives() {
        let checksum = Checksum::MD5("legit checksum".to_string());
        assert!(format!("{:?}", checksum).contains("MD5"));
        assert_eq!(checksum, checksum.clone());
        assert_tokens(&checksum, &[
            Token::Enum { name: "Checksum" },
            Token::Str("md5"),
            Token::Str("legit checksum"),
        ]);
    }
}
