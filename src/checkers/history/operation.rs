//! Keeping track of all operations.
//!
//! The types in this module are used for logging all operations to disk. This information can be
//! used for debugging purposes, but is more directly used as a [`Checker`] to help prevent
//! synchronized changes from being overwritten.
//!
//! It does this by parsing synchronized logs from this and other systems to determine which system
//! was the last one to touch a file.

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
    /// Mapping of pile files to checksums
    hoard: Hoard,
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

impl HoardOperation {
    fn file_is_log(path: &Path) -> bool {
        let _span = tracing::trace_span!("file_is_log", ?path).entered();
        let result = path.is_file()
            && match path.file_name() {
                None => false,
                Some(name) => match name.to_str() {
                    None => false,
                    Some(name) => LOG_FILE_REGEX.is_match(name),
                },
            };
        tracing::trace!(result, "determined if file is operation log");
        result
    }

    /// Create a new `Operation` from the given hoard.
    ///
    /// # Errors
    ///
    /// Any I/O errors that occur while hashing files.
    pub fn new(is_backup: bool, hoard: &ConfigHoard) -> Result<Self, Error> {
        Ok(Self {
            timestamp: chrono::Utc::now(),
            is_backup,
            hoard: Hoard::try_from(hoard)?,
        })
    }

    /// Checks if files in both operations are the same.
    #[must_use]
    pub fn has_same_files(&self, other: &Self) -> bool {
        self.hoard == other.hoard
    }

    /// Returns the latest operation for the given hoard from a system history root directory.
    fn get_latest_hoard_operation_from_system_dir(
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
                path.map(|path| {
                    tracing::trace!(path=%path.display(), "loading operation log from path");
                    fs::File::open(path)
                        .map(serde_json::from_reader)
                        .map_err(Error::from)?
                        .map_err(Error::from)
                })?
            })
            .filter_map(|operation| match operation {
                Err(err) => Some(Err(err)),
                Ok(operation) => (!backups_only || operation.is_backup).then(|| Ok(operation)),
            })
            .reduce(|left, right| {
                let left = left?;
                let right = right?;
                if left.timestamp.timestamp() > right.timestamp.timestamp() {
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
        Self::get_latest_hoard_operation_from_system_dir(&self_folder, hoard, false)
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
            .map(|dir| Self::get_latest_hoard_operation_from_system_dir(&dir, hoard, true))
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

    /// Returns whether this pending operation is safe to perform.
    ///
    /// # Errors
    ///
    /// Any I/O errors encountered while reading operation logs from disk.
    pub fn is_valid_pending(&self, hoard: &str) -> Result<bool, Error> {
        let _span = tracing::debug_span!("is_pending_operation_valid", %hoard).entered();
        tracing::debug!("checking if the hoard operation is safe to perform");
        let last_local = Self::latest_local(hoard)?;
        let last_remote = Self::latest_remote_backup(hoard)?;

        if !self.is_backup {
            tracing::debug!("not backing up, is safe to continue");
            return Ok(true);
        }

        match (last_local, last_remote) {
            (_, None) => {
                tracing::debug!(%hoard, "no remote operations found for hoard, is safe to continue");
                Ok(true)
            }
            (None, Some(last_remote)) => {
                tracing::debug!(%hoard, "no local operations found, is not safe to continue");
                Ok(self.has_same_files(&last_remote))
            }
            (Some(last_local), Some(last_remote)) => {
                if last_local.timestamp > last_remote.timestamp {
                    // Allow if the last operation on this machine
                    tracing::debug!(
                        local=%last_local.timestamp,
                        remote=%last_remote.timestamp,
                        "latest local operation is more recent than last remote operation"
                    );
                    Ok(true)
                } else {
                    tracing::debug!(
                        local=%last_local.timestamp,
                        remote=%last_remote.timestamp,
                        "latest local operation is less recent than last remote operation"
                    );
                    Ok(self.has_same_files(&last_remote))
                }
            }
        }
    }

    /// Writes this operation log to disk for the given hoard.
    ///
    /// # Errors
    ///
    /// Any I/O errors that may occur while writing the file to disk.
    pub fn commit_to_disk(&self, hoard: &str) -> Result<(), Error> {
        let _span = tracing::trace_span!("commit_operation_to_disk", %hoard).entered();
        let id = super::get_or_generate_uuid()?;
        let path = super::get_history_dir_for_id(id)
            .join(hoard)
            .join(format!("{}.log", self.timestamp.format(TIME_FORMAT_STR)));
        tracing::trace!(path=%path.display(), "ensuring parent directories for operation log file");
        path.parent().map(fs::create_dir_all).transpose()?;
        let file = fs::File::create(path)?;
        serde_json::to_writer(file, self)?;
        Ok(())
    }
}
