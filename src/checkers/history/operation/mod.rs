//! Types for recording metadata about a single backup or restore [`Operation`].

use crate::checkers::history::operation::util::TIME_FORMAT;
use crate::checkers::Checker;
use crate::hoard::{Direction, Hoard};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::{fs, io};
use thiserror::Error;
use time::OffsetDateTime;

pub mod util;
pub mod v1;
pub mod v2;

use crate::checkers::history::operation::v1::OperationV1;
use crate::checkers::history::operation::v2::OperationV2;
use crate::hoard_item::Checksum;
use crate::paths::{HoardPath, RelativePath};
pub(crate) use util::cleanup_operations;
use crate::newtypes::{HoardName, PileName};

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
    /// Found a mix of empty/anonymous and actual pile names.
    ///
    /// This shouldn't happen in practice, but returning an error is preferred to panicking.
    #[error("found mixed empty/anonymous and non-empty pile names")]
    MixedPileNames,
}

/// Information logged about a single Hoard file inside of an Operation.
///
/// This is *not* the Operation log file.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[allow(clippy::module_name_repetitions)]
pub struct OperationFileInfo {
    pile_name: PileName,
    relative_path: RelativePath,
    checksum: Option<Checksum>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
#[allow(clippy::module_name_repetitions)]
enum OperationVersion {
    V1(OperationV1),
    V2(OperationV2),
}

/// Functions that must be implemented by all operation log versions.
#[allow(clippy::module_name_repetitions)]
pub trait OperationImpl {
    /// Which [`Direction`] the operation went.
    fn direction(&self) -> Direction;
    /// Whether the operation log contains the given file by pile name and relative path.
    fn contains_file(
        &self,
        pile_name: &PileName,
        rel_path: &RelativePath,
        only_modified: bool,
    ) -> bool;
    /// The timestamp for the logged operation.
    fn timestamp(&self) -> OffsetDateTime;
    /// The associated hoard name for this operation.
    fn hoard_name(&self) -> &HoardName;
    /// The checksum associated with the given file, or `None` if the file does not exist or was
    /// deleted.
    fn checksum_for(&self, pile_name: &PileName, rel_path: &RelativePath) -> Option<Checksum>;
    /// An iterator over all files that exist within this operation log, not including any that
    /// were deleted.
    fn all_files_with_checksums<'a>(&'a self) -> Box<dyn Iterator<Item = OperationFileInfo> + 'a>;
}

impl OperationImpl for OperationVersion {
    fn direction(&self) -> Direction {
        match &self {
            OperationVersion::V1(one) => one.direction(),
            OperationVersion::V2(two) => two.direction(),
        }
    }

    fn contains_file(
        &self,
        pile_name: &PileName,
        rel_path: &RelativePath,
        only_modified: bool,
    ) -> bool {
        match &self {
            OperationVersion::V1(one) => one.contains_file(pile_name, rel_path, only_modified),
            OperationVersion::V2(two) => two.contains_file(pile_name, rel_path, only_modified),
        }
    }

    fn timestamp(&self) -> OffsetDateTime {
        match &self {
            OperationVersion::V1(one) => one.timestamp(),
            OperationVersion::V2(two) => two.timestamp(),
        }
    }

    fn hoard_name(&self) -> &HoardName {
        match &self {
            OperationVersion::V1(one) => one.hoard_name(),
            OperationVersion::V2(two) => two.hoard_name(),
        }
    }

    fn checksum_for(&self, pile_name: &PileName, rel_path: &RelativePath) -> Option<Checksum> {
        match &self {
            OperationVersion::V1(one) => one.checksum_for(pile_name, rel_path),
            OperationVersion::V2(two) => two.checksum_for(pile_name, rel_path),
        }
    }

    fn all_files_with_checksums<'a>(&'a self) -> Box<dyn Iterator<Item = OperationFileInfo> + 'a> {
        match &self {
            OperationVersion::V1(one) => one.all_files_with_checksums(),
            OperationVersion::V2(two) => two.all_files_with_checksums(),
        }
    }
}

/// A wrapper struct for any supported operation log version.
///
/// This struct should be preferred over any specific log version.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(transparent)]
pub struct Operation(OperationVersion);

impl OperationImpl for Operation {
    fn direction(&self) -> Direction {
        self.0.direction()
    }

    fn contains_file(
        &self,
        pile_name: &PileName,
        rel_path: &RelativePath,
        only_modified: bool,
    ) -> bool {
        self.0.contains_file(pile_name, rel_path, only_modified)
    }

    fn timestamp(&self) -> OffsetDateTime {
        self.0.timestamp()
    }

    fn hoard_name(&self) -> &HoardName {
        self.0.hoard_name()
    }

    fn checksum_for(&self, pile_name: &PileName, rel_path: &RelativePath) -> Option<Checksum> {
        self.0.checksum_for(pile_name, rel_path)
    }

    fn all_files_with_checksums<'a>(&'a self) -> Box<dyn Iterator<Item = OperationFileInfo> + 'a> {
        self.0.all_files_with_checksums()
    }
}

impl Operation {
    fn new(
        hoards_root: &HoardPath,
        name: &HoardName,
        hoard: &Hoard,
        direction: Direction,
    ) -> Result<Self, Error> {
        v2::OperationV2::new(hoards_root, name, hoard, direction)
            .map(OperationVersion::V2)
            .map(Self)
    }

    fn require_latest_version(&self) -> Result<(), Error> {
        if let Self(OperationVersion::V2(_)) = self {
            Ok(())
        } else {
            Err(Error::UpgradeRequired)
        }
    }

    fn as_latest_version(&self) -> Result<&Self, Error> {
        self.require_latest_version().map(|_| self)
    }

    fn into_latest_version(self) -> Result<Self, Error> {
        self.require_latest_version().map(|_| self)
    }

    fn from_file(path: &Path) -> Result<Self, Error> {
        tracing::trace!(path=%path.display(), "loading operation log from path");
        fs::File::open(path)
            .map(serde_json::from_reader)
            .map_err(|err| {
                tracing::error!("failed to open file at {}: {}", path.display(), err);
                Error::from(err)
            })?
            .map_err(|err| {
                tracing::error!("failed to parse JSON from {}: {}", path.display(), err);
                Error::from(err)
            })
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

    fn reduce_option_latest(
        left: Result<Option<Self>, Error>,
        right: Result<Option<Self>, Error>,
    ) -> Result<Option<Self>, Error> {
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

    /// Given a summary of previous operations, convert this [`Operation`] to the latest version.
    ///
    /// # Parameters
    ///
    /// - `file_checksums`: A mapping of file (as (`pile_name`, `relative_path`) tuple) to the
    ///   file's checksum prior to this operation. If the file was deleted at some point, checksum
    ///   should be `None` rather than deleting the file from the map.
    /// - `file_set`: A set of files that exist in the hoard prior to this operation. If a file was
    ///   deleted at some point, it should be removed from this set.
    pub(crate) fn convert_to_latest_version(
        self,
        file_checksums: &mut HashMap<(PileName, RelativePath), Option<Checksum>>,
        file_set: &mut HashSet<(PileName, RelativePath)>,
    ) -> Self {
        // Conversion always modifies file_checksums and file_set with the contents of the Operation.
        let latest = match self.0 {
            OperationVersion::V1(one) => OperationV2::from_v1(file_checksums, file_set, one),
            OperationVersion::V2(two) => {
                let mut new_file_set = HashSet::new();
                for file_info in two.all_files_with_checksums() {
                    let OperationFileInfo {
                        pile_name,
                        relative_path,
                        checksum,
                        ..
                    } = file_info;
                    let pile_file = (pile_name, relative_path);
                    new_file_set.insert(pile_file.clone());
                    file_checksums.insert(pile_file, checksum);
                }
                *file_set = new_file_set;
                two
            }
        };

        Self(OperationVersion::V2(latest))
    }

    /// Returns the latest operation for the given hoard from a system history root directory.
    ///
    /// `path` must be relative to one of the hoard's piles.
    fn latest_hoard_operation_from_local_dir(
        dir: &HoardPath,
        hoard: &HoardName,
        path: Option<(&PileName, &RelativePath)>,
        backups_only: bool,
        only_modified: bool,
    ) -> Result<Option<Self>, Error> {
        let _span = tracing::trace_span!("get_latest_hoard_operation", ?dir, %hoard).entered();
        tracing::trace!("getting latest operation log for hoard in dir");
        let root = dir.join(&RelativePath::from(hoard));
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
            .map(|path| -> Result<Self, Error> { path.map(|path| Self::from_file(&path))? })
            .filter_map(|operation| match operation {
                Err(err) => Some(Err(err)),
                Ok(operation) => (!backups_only || operation.direction() == Direction::Backup)
                    .then(|| Ok(operation)),
            })
            .filter_map(|operation| match path {
                None => Some(operation),
                Some((pile_name, path)) => match operation {
                    Err(err) => Some(Err(err)),
                    Ok(operation) => operation
                        .contains_file(pile_name, path, only_modified)
                        .then(|| Ok(operation)),
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
    pub fn latest_local(
        hoard: &HoardName,
        file: Option<(&PileName, &RelativePath)>,
    ) -> Result<Option<Self>, Error> {
        let _span = tracing::debug_span!("latest_local", %hoard).entered();
        tracing::trace!("finding latest Operation file for this machine");
        let uuid = super::get_or_generate_uuid()?;
        let self_folder = super::get_history_dir_for_id(uuid);
        Self::latest_hoard_operation_from_local_dir(&self_folder, hoard, file, false, false)
    }

    /// Returns the latest backup operation recorded on any other machine (by UUID).
    ///
    /// `file`, if provided, must be a path relative to the root of one of the Hoard's Piles.
    ///
    /// # Errors
    ///
    /// - Any errors that occur while reading from the filesystem
    /// - Any parsing errors from `serde_json` when parsing the file
    pub(crate) fn latest_remote_backup(
        hoard: &HoardName,
        file: Option<(&PileName, &RelativePath)>,
        only_modified: bool,
    ) -> Result<Option<Self>, Error> {
        let _span = tracing::debug_span!("latest_remote_backup").entered();
        tracing::trace!("finding latest Operation file from remote machines");
        let uuid = super::get_or_generate_uuid()?;
        let other_folders = super::get_history_dirs_not_for_id(&uuid)?;
        other_folders
            .into_iter()
            .map(|dir| {
                Self::latest_hoard_operation_from_local_dir(&dir, hoard, file, true, only_modified)
            })
            .reduce(Self::reduce_option_latest)
            .transpose()
            .map(Option::flatten)?
            .map(Self::into_latest_version)
            .transpose()
    }

    /// Returns whether the given `file` has unapplied remote changes.
    ///
    /// `file` must be a path relative to the root of one of the Hoard's Piles, or
    /// `RelativePath::none()` if the Pile represents a file.
    ///
    /// # Errors
    ///
    /// - Any errors returned by [`latest_local`] or [`latest_remote_backup`].
    pub(crate) fn file_has_remote_changes(
        hoard: &HoardName,
        pile_name: &PileName,
        file: &RelativePath,
    ) -> Result<bool, Error> {
        let remote = Self::latest_remote_backup(hoard, Some((pile_name, file)), true)?;
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
    pub(crate) fn file_has_records(
        hoard: &HoardName,
        pile_name: &PileName,
        file: &RelativePath,
    ) -> Result<bool, Error> {
        let remote = Self::latest_remote_backup(hoard, Some((pile_name, file)), false)?;
        let local = Self::latest_local(hoard, Some((pile_name, file)))?;

        Ok(remote.is_some() || local.is_some())
    }

    pub(crate) fn check_has_same_files(&self, other: &Self) -> Result<(), Error> {
        let self_files: HashSet<OperationFileInfo> = self
            .as_latest_version()?
            .all_files_with_checksums()
            .collect();
        let other_files: HashSet<OperationFileInfo> = other
            .as_latest_version()?
            .all_files_with_checksums()
            .collect();
        if self_files == other_files {
            Ok(())
        } else {
            match self.0.direction() {
                Direction::Backup => Err(Error::RestoreRequired),
                Direction::Restore => Err(Error::BackupRequired),
            }
        }
    }
}

impl Checker for Operation {
    type Error = Error;

    fn new(
        hoards_root: &HoardPath,
        name: &HoardName,
        hoard: &Hoard,
        direction: Direction,
    ) -> Result<Self, Self::Error> {
        Self::new(hoards_root, name, hoard, direction)
    }

    fn check(&mut self) -> Result<(), Self::Error> {
        let _span =
            tracing::debug_span!("is_pending_operation_valid", hoard=%self.hoard_name()).entered();
        tracing::debug!("checking if the hoard operation is safe to perform");
        let last_local = Self::latest_local(self.hoard_name(), None)?;
        let last_remote = Self::latest_remote_backup(self.hoard_name(), None, false)?;

        if self.direction() != Direction::Backup {
            tracing::debug!("not backing up, is safe to continue");
            return Ok(());
        }

        match (last_local, last_remote) {
            (_, None) => {
                tracing::debug!("no remote operations found for hoard, is safe to continue");
                Ok(())
            }
            (None, Some(last_remote)) => {
                tracing::debug!("no local operations found, might not be safe to continue");
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
            .join(&RelativePath::from(self.hoard_name()))
            .join(&RelativePath::try_from(PathBuf::from(format!(
                "{}.log",
                self.timestamp()
                    .format(&TIME_FORMAT)
                    .map_err(Error::FormatDatetime)?
            ))).expect("file name is always a valid RelativePath"));
        tracing::trace!(path=%path.display(), "ensuring parent directories for operation log file");
        path.parent().map(fs::create_dir_all).transpose()?;
        let file = fs::File::create(path)?;
        serde_json::to_writer(file, &self)?;
        Ok(())
    }
}
