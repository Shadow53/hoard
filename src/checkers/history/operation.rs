//! Keeping track of all operations.
//!
//! The types in this module are used for logging all operations to disk. This information can be
//! used for debugging purposes, but is more directly used as a [`Checker`] to help prevent
//! synchronized changes from being overwritten.
//!
//! It does this by parsing synchronized logs from this and other systems to determine which system
//! was the last one to touch a file.

use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::DirEntry;
use std::path::{Path, PathBuf};
use std::{fs, io};
use thiserror::Error;
use uuid::Uuid;

static LOG_FILE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new("^([0-9]{2}_){2}([0-9]{2}-[0-9]{2}_){2}([0-9]{2}).log$")
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
pub struct Operation {
    /// Timestamp of last operation
    timestamp: chrono::DateTime<chrono::Utc>,
    /// Maps name of hoard to mapping of files to their checksums
    hoards: HashMap<String, Hoard>,
    /// Maps UUID of device to name of previous log file
    last_logs: HashMap<Uuid, String>,
}

/// Operation log information for a single hoard.
///
/// Really just a wrapper for [`PileOperation`] because piles may be anonymous or named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Hoard {
    /// Information for a single, anonymous pile.
    Anonymous(Pile),
    /// Information for some number of named piles.
    Named(HashMap<String, Pile>),
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

impl Operation {
    fn file_is_log(path: &Path) -> bool {
        path.is_file()
            && match path.file_name() {
                None => false,
                Some(name) => match name.to_str() {
                    None => false,
                    Some(name) => LOG_FILE_REGEX.is_match(name),
                },
            }
    }

    /// Create a new `Operation` from the given hoards mappings and map of last logs.
    #[must_use]
    pub fn new(hoards: HashMap<String, Hoard>, last_logs: HashMap<Uuid, String>) -> Self {
        Self {
            timestamp: chrono::Utc::now(),
            hoards,
            last_logs,
        }
    }

    /// Returns the latest operation recorded on this machine (by UUID).
    ///
    /// # Errors
    ///
    /// - Any errors that occur while reading from the filesystem
    /// - Any parsing errors from `serde_json` when parsing the file
    pub fn latest() -> Result<Option<Self>, Error> {
        tracing::debug!("finding latest Operation file for this machine");
        let uuid = super::get_or_generate_uuid()?;
        let self_folder = super::get_history_dir_for_id(uuid);
        let latest_file = self_folder
            .read_dir()?
            .filter_map(|item| {
                // Only keep errors and anything where path() returns Some
                item.map(|item| {
                    let path = item.path();
                    Self::file_is_log(&path).then(|| path)
                })
                .transpose()
            })
            .reduce(|acc, item| {
                let acc = acc?;
                let item = item?;
                if acc.file_name() > item.file_name() {
                    Ok(acc)
                } else {
                    Ok(item)
                }
            })
            .transpose()?;

        latest_file
            .map(|entry| fs::File::open(entry).map(serde_json::from_reader))
            .transpose()?
            .transpose()
            .map_err(Error::Serde)
    }

    /// Returns a sorted list of all unseen operations since this one happened.
    ///
    /// # Errors
    ///
    /// - Any I/O errors that occur while reading log files.
    /// - Any `serde` errors that occur while parsing log files.
    pub fn all_unseen_operations_since(&self) -> Result<Vec<Operation>, Error> {
        //
        let _span = tracing::debug_span!("loading unseen operation logs", since=%self.timestamp.format("%F %T")).entered();
        let root = super::get_history_root_dir();
        let mut result_vec = Vec::new();
        for result in root.read_dir()? {
            let entry = result?;
            if let Ok(id) = entry.file_name().to_string_lossy().parse::<Uuid>() {
                // Found a remote system log dir
                tracing::debug!("checking logs for system: {}", id);
                let _span = tracing::trace_span!("checking_logs", %id).entered();

                let last_log = self.last_logs.get(&id);
                let last_log_os = last_log.as_ref().map(std::ffi::OsString::from);

                tracing::trace!("getting all unseen entries based on file name");
                let mut entries: Vec<DirEntry> = entry
                    .path()
                    .read_dir()?
                    .filter_map(Result::ok)
                    .filter(|item| Self::file_is_log(&item.path()))
                    .filter(|item| match &last_log_os {
                        None => true,
                        Some(file) => &item.file_name() > file,
                    })
                    .collect();

                // Order of items read with read_dir is not guaranteed to be sorted.
                tracing::trace!("sorting entries by file name");
                entries.sort_by_key(fs::DirEntry::file_name);

                result_vec.append(&mut entries);
            }
        }

        tracing::trace!("parsing all unseen logs");
        result_vec
            .into_iter()
            .map(|entry| {
                let file = fs::File::open(entry.path())?;
                Ok(serde_json::from_reader(file)?)
            })
            .collect::<Result<Vec<_>, _>>()
    }
}
