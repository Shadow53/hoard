//! Types for recording metadata about a single backup or restore [`Operation`].

use std::{fs, io};
use std::path::Path;
use serde::{Serialize, Deserialize};
use thiserror::Error;
use time::OffsetDateTime;
use crate::checkers::Checker;
use crate::checkers::history::operation::util::TIME_FORMAT;
use crate::hoard::{Direction, Hoard};

mod v1;
mod v2;
mod util;

pub(crate) use util::cleanup_operations;
use crate::hoard_file::Checksum;

/// Errors that may occur while working with an [`Operation`].
#[derive(Debug, Error)]
pub enum Error {
    /// Failed to format a datetime.
    #[error("failed to format the current datetime: {0}")]
    FormatDatetime(#[from] time::error::Format),
    /// Any I/O error.
    #[error("an I/O error occurred: {0}")]
    IO(#[from] io::Error),
    /// An error occurred while (de)serializing with `serde`.
    #[error("a (de)serialization error occurred: {0}")]
    Serde(#[from] serde_json::Error),
    /// There are unapplied changes from another system. A restore or forced backup is required.
    #[error("found unapplied remote changes - restore this hoard to apply changes or force a backup with --force")]
    RestoreRequired,
    /// There are non-backed-up changes on this system. A backup or forced restore is required.
    #[error("found unsaved local changes - backup this hoard to save changes or force a restore with --force")]
    BackupRequired,
    /// The operation log files must be converted to the latest version.
    #[error("operation log format has changed -- please run `hoard upgrade`")]
    UpgradeRequired,
    /// An error occurred in the file iterator.
    #[error("error while iterating files: {0}")]
    Iterator(#[from] crate::hoard::iter::Error),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
enum OperationVersion {
    V1(v1::OperationV1),
    V2(v2::OperationV2),
}

pub(crate) trait OperationImpl {
    fn is_backup(&self) -> bool;
    fn contains_file(&self, pile_name: Option<&str>, rel_path: &Path) -> bool;
    fn timestamp(&self) -> OffsetDateTime;
    fn hoard_name(&self) -> &str;
    fn checksum_for(&self, pile_name: Option<&str>, rel_path: &Path) -> Option<Checksum>;
}

impl OperationImpl for OperationVersion {
    fn is_backup(&self) -> bool {
        match &self {
            OperationVersion::V1(one) => one.is_backup(),
            OperationVersion::V2(two) => two.is_backup(),
        }
    }

    fn contains_file(&self, pile_name: Option<&str>, rel_path: &Path) -> bool {
        match &self {
            OperationVersion::V1(one) => one.contains_file(pile_name, rel_path),
            OperationVersion::V2(two) => two.contains_file(pile_name, rel_path),
        }
    }

    fn timestamp(&self) -> OffsetDateTime {
        match &self {
            OperationVersion::V1(one) => one.timestamp(),
            OperationVersion::V2(two) => two.timestamp(),
        }
    }

    fn hoard_name(&self) -> &str {
        match &self {
            OperationVersion::V1(one) => one.hoard_name(),
            OperationVersion::V2(two) => two.hoard_name(),
        }
    }

    fn checksum_for(&self, pile_name: Option<&str>, rel_path: &Path) -> Option<Checksum> {
        match &self {
            OperationVersion::V1(one) => one.checksum_for(pile_name, rel_path),
            OperationVersion::V2(two) => two.checksum_for(pile_name, rel_path),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(transparent)]
pub(crate) struct Operation(OperationVersion);

impl OperationImpl for Operation {
    fn is_backup(&self) -> bool {
        self.0.is_backup()
    }

    fn contains_file(&self, pile_name: Option<&str>, rel_path: &Path) -> bool {
        self.0.contains_file(pile_name, rel_path)
    }

    fn timestamp(&self) -> OffsetDateTime {
        self.0.timestamp()
    }

    fn hoard_name(&self) -> &str {
        self.0.hoard_name()
    }

    fn checksum_for(&self, pile_name: Option<&str>, rel_path: &Path) -> Option<Checksum> {
        self.0.checksum_for(pile_name, rel_path)
    }
}

impl Operation {
    fn new(hoards_root: &Path, name: &str, hoard: &Hoard, direction: Direction) -> Result<Self, Error> {
        v2::OperationV2::new(hoards_root, name, hoard, direction)
            .map(OperationVersion::V2)
            .map(Self)
    }

    fn as_latest_version(&self) -> Result<&v2::OperationV2, Error> {
        if let Self(OperationVersion::V2(op)) = self {
            Ok(op)
        } else {
            Err(Error::UpgradeRequired)
        }
    }

    fn from_file(path: &Path) -> Result<Self, Error> {
        tracing::trace!(path=%path.display(), "loading operation log from path");
        fs::File::open(path)
            .map(serde_json::from_reader)
            .map_err(Error::from)?
            .map_err(Error::from)
    }

    fn reduce_latest(left: Result<Self, Error>, right: Result<Self, Error>) -> Result<Self, Error> {
        let left = left?;
        let right = right?;
        if left.timestamp() > right.timestamp() {
            // grcov: ignore-start
            // This branch doesn't seem to be taken by tests, at least locally.
            // I don't know of a way to force this branch to be taken and it is simple
            // enough that I feel comfortable marking it ignored.
            Ok(left)
            // grcov: ignore-end
        } else {
            Ok(right)
        }
    }

    fn reduce_option_latest(left: Result<Option<Self>, Error>, right: Result<Option<Self>, Error>) -> Result<Option<Self>, Error> {
        match (left?, right?) {
            (Some(left), None) => Ok(Some(left)),
            (None, Some(right)) => Ok(Some(right)),
            (None, None) => Ok(None),
            (Some(left), Some(right)) => {
                if left.timestamp() > right.timestamp() {
                    Ok(Some(left))
                } else {
                    Ok(Some(right))
                }
            }
        }
    }

    /// Returns the latest operation for the given hoard from a system history root directory.
    ///
    /// `path` must be relative to one of the hoard's piles.
    fn latest_hoard_operation_from_system_dir(
        dir: &Path,
        hoard: &str,
        path: Option<(Option<&str>, &Path)>,
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
                    util::file_is_log(&path).then(|| path)
                })
                    .transpose()
            })
            .map(|path| -> Result<Self, Error> {
                path.map(|path| Self::from_file(&path))?
            })
            .filter_map(|operation| match operation {
                Err(err) => Some(Err(err)),
                Ok(operation) => (!backups_only || operation.is_backup()).then(|| Ok(operation)),
            })
            .filter_map(|operation| match path {
                None => Some(operation),
                Some((pile_name, path)) => match operation {
                    Err(err) => Some(Err(err)),
                    Ok(operation) => operation.contains_file(pile_name, path).then(|| Ok(operation)),
                },
            })
            .reduce(Self::reduce_latest)
            .transpose()
    }

    /// Returns the latest operation recorded on this machine (by UUID).
    ///
    /// `file`, if provided, must be a path relative to the root of one of the Hoard's Piles.
    ///
    /// # Errors
    ///
    /// - Any errors that occur while reading from the filesystem
    /// - Any parsing errors from `serde_json` when parsing the file
    pub(crate) fn latest_local(hoard: &str, file: Option<(Option<&str>, &Path)>) -> Result<Option<Self>, Error> {
        let _span = tracing::debug_span!("latest_local", %hoard).entered();
        tracing::debug!("finding latest Operation file for this machine");
        let uuid = super::get_or_generate_uuid()?;
        let self_folder = super::get_history_dir_for_id(uuid);
        Self::latest_hoard_operation_from_system_dir(&self_folder, hoard, file, false)
    }

    /// Returns the latest backup operation recorded on any other machine (by UUID).
    ///
    /// `file`, if provided, must be a path relative to the root of one of the Hoard's Piles.
    ///
    /// # Errors
    ///
    /// - Any errors that occur while reading from the filesystem
    /// - Any parsing errors from `serde_json` when parsing the file
    pub(crate) fn latest_remote_backup(hoard: &str, file: Option<(Option<&str>, &Path)>) -> Result<Option<Self>, Error> {
        let _span = tracing::debug_span!("latest_remote_backup").entered();
        tracing::debug!("finding latest Operation file from remote machines");
        let uuid = super::get_or_generate_uuid()?;
        let other_folders = super::get_history_dirs_not_for_id(&uuid)?;
        other_folders
            .into_iter()
            .map(|dir| Self::latest_hoard_operation_from_system_dir(&dir, hoard, file, true))
            .reduce(Self::reduce_option_latest)
            .transpose()
            .map(Option::flatten)
    }

    /// Returns whether the given `file` has unapplied remote changes.
    ///
    /// `file` must be a path relative to the root of one of the Hoard's Piles.
    ///
    /// # Errors
    ///
    /// - Any errors returned by [`latest_local`] or [`latest_remote_backup`].
    pub(crate) fn file_has_remote_changes(hoard: &str, pile_name: Option<&str>, file: &Path) -> Result<bool, Error> {
        let remote = Self::latest_remote_backup(hoard, Some((pile_name, file)))?;
        let local = Self::latest_local(hoard, Some((pile_name, file)))?;

        let result = match (remote, local) {
            (None, _) => false,
            (Some(_), None) => true,
            (Some(remote), Some(local)) => remote.timestamp() > local.timestamp(),
        };

        Ok(result)
    }

    /// Returns whether the given `file` has any records from `hoard`.
    ///
    /// `file` must be a path relative to the root of one of the Hoard's Piles.
    ///
    /// # Errors
    ///
    /// - Any errors returned by [`latest_local`] or [`latest_remote_backup`].
    pub(crate) fn file_has_records(hoard: &str, pile_name: Option<&str>, file: &Path) -> Result<bool, Error> {
        let remote = Self::latest_remote_backup(hoard, Some((pile_name, file)))?;
        let local = Self::latest_local(hoard, Some((pile_name, file)))?;

        Ok(remote.is_some() || local.is_some())
    }

    pub(crate) fn check_has_same_files(&self, other: &Self) -> Result<(), Error> {
        if self.as_latest_version()? == other.as_latest_version()? {
            Ok(())
        } else if self.is_backup() {
            Err(Error::RestoreRequired)
        } else {
            Err(Error::BackupRequired)
        }
    }
}

impl Checker for Operation {
    type Error = Error;

    fn new(hoards_root: &Path, name: &str, hoard: &Hoard, direction: Direction) -> Result<Self, Self::Error> {
        Self::new(hoards_root, name, hoard, direction)
    }

    fn check(&mut self) -> Result<(), Self::Error> {
        let _span =
            tracing::debug_span!("is_pending_operation_valid", hoard=%self.hoard_name()).entered();
        tracing::debug!("checking if the hoard operation is safe to perform");
        let last_local = Self::latest_local(self.hoard_name(), None)?;
        let last_remote = Self::latest_remote_backup(self.hoard_name(), None)?;

        if !self.is_backup() {
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
                if last_local.timestamp() > last_remote.timestamp() {
                    // Allow if the last operation on this machine
                    tracing::debug!(
                        local=%last_local.timestamp(),
                        remote=%last_remote.timestamp(),
                        "latest local operation is more recent than last remote operation"
                    );
                    Ok(())
                } else {
                    tracing::debug!(
                        local=%last_local.timestamp(),
                        remote=%last_remote.timestamp(),
                        "latest local operation is less recent than last remote operation"
                    );
                    self.check_has_same_files(&last_remote)
                }
            }
        }
    }

    fn commit_to_disk(self) -> Result<(), Self::Error> {
        let _span =
            tracing::trace_span!("commit_operation_to_disk", hoard=%self.hoard_name()).entered();
        let id = super::get_or_generate_uuid()?;
        let path = super::get_history_dir_for_id(id)
            .join(self.hoard_name())
            .join(format!(
                "{}.log",
                self.timestamp()
                    .format(&TIME_FORMAT)
                    .map_err(Error::FormatDatetime)?
            ));
        tracing::trace!(path=%path.display(), "ensuring parent directories for operation log file");
        path.parent().map(fs::create_dir_all).transpose()?;
        let file = fs::File::create(path)?;
        serde_json::to_writer(file, &self)?;
        Ok(())
    }
}