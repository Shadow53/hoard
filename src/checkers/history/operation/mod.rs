//! Keeping track of all operations.
//!
//! The types in this module are used for logging all operations to disk. This information can be
//! used for debugging purposes, but is more directly used as a [`Checker`]
//! to help prevent synchronized changes from being overwritten.
//!
//! It does this by parsing synchronized logs from this and other systems to determine which system
//! was the last one to touch a file.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use futures::stream::TryStreamExt;
use serde::de::Error as _;
use serde::{Deserialize, Serialize};
use tap::tap::TapFallible;
use thiserror::Error;
use time::OffsetDateTime;
use tokio::{fs, io};
use tokio_stream::wrappers::ReadDirStream;

pub(crate) use util::cleanup_operations;

use crate::checkers::history::operation::util::TIME_FORMAT;
use crate::checkers::history::operation::v1::OperationV1;
use crate::checkers::history::operation::v2::OperationV2;
use crate::checkers::Checker;
use crate::checksum::Checksum;
use crate::hoard::{Direction, Hoard};
use crate::hoard_item::{CachedHoardItem, HoardItem};
use crate::newtypes::{HoardName, PileName};
use crate::paths::{HoardPath, RelativePath};

pub mod util;
pub mod v1;
pub mod v2;

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

/// Indicates what operation is/was/should be performed on the contained [`HoardItem`]
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions)]
#[allow(missing_docs)]
pub enum ItemOperation<T> {
    Create(T),
    Modify(T),
    Delete(T),
    /// Indicates a file that is unchanged.
    Nothing(T),
    /// Indicates a file that does not exist but is listed directly in the config.
    DoesNotExist(T),
}

impl ItemOperation<CachedHoardItem> {
    pub(crate) fn short_name(&self) -> String {
        match self {
            ItemOperation::Create(file) => format!("create {}", file.system_path().display()),
            ItemOperation::Modify(file) => format!("modify {}", file.system_path().display()),
            ItemOperation::Delete(file) => format!("delete {}", file.system_path().display()),
            ItemOperation::Nothing(file) => {
                format!("do nothing with {}", file.system_path().display())
            }
            ItemOperation::DoesNotExist(file) => {
                format!("{} does not exist", file.system_path().display())
            }
        }
    }
}

impl<T> ItemOperation<T> {
    /// Converts into the contained item.
    pub fn into_inner(self) -> T {
        match self {
            ItemOperation::Create(item)
            | ItemOperation::Modify(item)
            | ItemOperation::Delete(item)
            | ItemOperation::Nothing(item)
            | ItemOperation::DoesNotExist(item) => item,
        }
    }
}

/// Enum representing types of operations
///
/// Does not include no operation.
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub enum OperationType {
    /// Created a (system) file.
    Create,
    /// Modified an existing file.
    Modify,
    /// Deleted a (system) file.
    Delete,
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

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(untagged)]
#[allow(clippy::module_name_repetitions)]
enum OperationVersion {
    V1(OperationV1),
    V2(OperationV2),
}

impl<'de> Deserialize<'de> for OperationVersion {
    #[tracing::instrument(skip_all, name = "deserialize_operation")]
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let content =
            match <serde::__private::de::Content as Deserialize>::deserialize(deserializer) {
                Ok(val) => val,
                Err(err) => {
                    return Err(err);
                }
            };

        match Result::map(
            <OperationV2 as Deserialize>::deserialize(
                serde::__private::de::ContentRefDeserializer::<D::Error>::new(&content),
            ),
            OperationVersion::V2,
        ) {
            Ok(ok) => return Ok(ok),
            Err(err) => {
                tracing::warn!("operation does not match V2: {}", err);
            }
        }

        match Result::map(
            <OperationV1 as Deserialize>::deserialize(
                serde::__private::de::ContentRefDeserializer::<D::Error>::new(&content),
            ),
            OperationVersion::V1,
        ) {
            Ok(ok) => return Ok(ok),
            Err(err) => {
                tracing::warn!("operation does not match V1: {}", err);
            }
        }

        crate::create_log_error(D::Error::custom(
            "data did not match any operation log version",
        ))
    }
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
    /// Returns an iterator of the file operations represented by this operation object.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UpgradeRequired`] if the operation version is too old to support this
    /// function.
    fn hoard_operations_iter<'a>(
        &'a self,
        _hoard_path: &HoardPath,
        _hoard: &Hoard,
    ) -> Result<Box<dyn Iterator<Item = ItemOperation<HoardItem>> + 'a>, Error> {
        crate::create_log_error(Error::UpgradeRequired)
    }

    /// Returns the operation performed on the given file, if any.
    ///
    /// Returns `None` if the file did not exist in the hoard *or* if the file was unmodified.
    ///
    /// # Errors
    ///
    /// Returns [`Error::UpgradeRequired`] in the case when the operation log format is too old to
    /// contain information about the operation performed on a given file.
    fn file_operation(
        &self,
        _pile_name: &PileName,
        _rel_path: &RelativePath,
    ) -> Result<Option<OperationType>, Error> {
        crate::create_log_error(Error::UpgradeRequired)
    }
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

    fn hoard_operations_iter<'a>(
        &'a self,
        hoard_root: &HoardPath,
        hoard: &Hoard,
    ) -> Result<Box<dyn Iterator<Item = ItemOperation<HoardItem>> + 'a>, Error> {
        match &self {
            OperationVersion::V1(v1) => v1.hoard_operations_iter(hoard_root, hoard),
            OperationVersion::V2(v2) => v2.hoard_operations_iter(hoard_root, hoard),
        }
    }

    fn file_operation(
        &self,
        pile_name: &PileName,
        rel_path: &RelativePath,
    ) -> Result<Option<OperationType>, Error> {
        match &self {
            OperationVersion::V1(v1) => v1.file_operation(pile_name, rel_path),
            OperationVersion::V2(v2) => v2.file_operation(pile_name, rel_path),
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

    fn hoard_operations_iter<'a>(
        &'a self,
        hoard_root: &HoardPath,
        hoard: &Hoard,
    ) -> Result<Box<dyn Iterator<Item = ItemOperation<HoardItem>> + 'a>, Error> {
        self.0.hoard_operations_iter(hoard_root, hoard)
    }

    fn file_operation(
        &self,
        pile_name: &PileName,
        rel_path: &RelativePath,
    ) -> Result<Option<OperationType>, Error> {
        self.0.file_operation(pile_name, rel_path)
    }
}

impl Operation {
    async fn new(
        hoards_root: &HoardPath,
        name: &HoardName,
        hoard: &Hoard,
        direction: Direction,
    ) -> Result<Self, Error> {
        OperationV2::new(hoards_root, name, hoard, direction)
            .await
            .map(OperationVersion::V2)
            .map(Self)
    }

    /// Return an error if this `Operation` is not the most recent schema version.
    ///
    /// # Errors
    ///
    /// [`Error::UpgradeRequired`] if this `Operation` is not the most recent schema.
    pub fn require_latest_version(&self) -> Result<(), Error> {
        if let Self(OperationVersion::V2(_)) = self {
            Ok(())
        } else {
            crate::create_log_error(Error::UpgradeRequired)
        }
    }

    /// Borrows the `Operation` if it is the most recent schema version, otherwise returns an error.
    ///
    /// # Errors
    ///
    /// [`Error::UpgradeRequired`] if this `Operation` is not the most recent schema.
    pub fn as_latest_version(&self) -> Result<&Self, Error> {
        self.require_latest_version().map(|()| self)
    }

    /// Returns the owned `Operation` if it is the most recent schema version, otherwise returns an error.
    ///
    /// # Errors
    ///
    /// [`Error::UpgradeRequired`] if this `Operation` is not the most recent schema.
    pub fn into_latest_version(self) -> Result<Self, Error> {
        self.require_latest_version().map(|()| self)
    }

    #[tracing::instrument(name = "operation_from_file")]
    async fn from_file(path: &Path) -> Result<Self, Error> {
        tracing::trace!(path=%path.display(), "loading operation log from path");
        let content = fs::read(path).await.tap_err(|error| {
            tracing::error!(%error, "failed to open file at {}", path.display());
        })?;
        serde_json::from_slice(&content)
            .map_err(Error::from)
            .tap_err(|error| {
                tracing::error!(%error, "failed to parse JSON from {}", path.display());
            })
    }

    /// Returns the most recent `Operation`
    ///
    /// Is async because it's passed to [`TryFuture::try_fold`], which expects a function returning a future.
    /// Returns a `Result` because that's the easiest signature to use.
    #[allow(clippy::unused_async)]
    async fn reduce_latest(left: Option<Self>, right: Self) -> Result<Option<Self>, Error> {
        match left {
            None => Ok(Some(right)),
            Some(left) => {
                if left.timestamp() > right.timestamp() {
                    // grcov: ignore-start
                    // This branch doesn't seem to be taken by tests, at least locally.
                    // I don't know of a way to force this branch to be taken and it is simple
                    // enough that I feel comfortable marking it ignored.
                    Ok(Some(left))
                    // grcov: ignore-end
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
    #[tracing::instrument(level = "trace")]
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
    #[tracing::instrument(level = "trace")]
    async fn latest_hoard_operation_from_local_dir(
        dir: &HoardPath,
        hoard: &HoardName,
        file: Option<(&PileName, &RelativePath)>,
        backups_only: bool,
        only_modified: bool,
    ) -> Result<Option<Self>, Error> {
        tracing::trace!("getting latest operation log for hoard in dir");
        let root = dir.join(&RelativePath::from(hoard));
        if !root.exists() {
            tracing::trace!(dir=?root, "hoard dir does not exist, no logs found");
            return Ok(None);
        }

        ReadDirStream::new(fs::read_dir(&root).await?)
            .map_err(Error::IO)
            .try_filter_map(|item| async move {
                // Only keep errors and anything where path() returns Some
                let path = item.path();
                Ok(util::file_is_log(&path).then_some(path))
            })
            .and_then(|path| async move { Self::from_file(&path).await })
            .try_filter_map(|operation| async {
                (!backups_only || operation.direction() == Direction::Backup)
                    .then_some(Ok(operation))
                    .transpose()
            })
            .try_filter_map(|operation| async {
                match file {
                    None => Ok(Some(operation)),
                    Some((pile_name, path)) => operation
                        .contains_file(pile_name, path, only_modified)
                        .then_some(Ok(operation))
                        .transpose(),
                }
            })
            .try_fold(None, Self::reduce_latest)
            .await
    }

    /// Returns the latest operation recorded on this machine (by UUID).
    ///
    /// `file`, if provided, must be a path relative to the root of one of the Hoard's Piles.
    ///
    /// # Errors
    ///
    /// - Any errors that occur while reading from the filesystem
    /// - Any parsing errors from `serde_json` when parsing the file
    #[tracing::instrument(level = "debug")]
    pub async fn latest_local(
        hoard: &HoardName,
        file: Option<(&PileName, &RelativePath)>,
    ) -> Result<Option<Self>, Error> {
        tracing::trace!("finding latest Operation file for this machine");
        let uuid = super::get_or_generate_uuid().await?;
        let self_folder = super::get_history_dir_for_id(uuid);
        Self::latest_hoard_operation_from_local_dir(&self_folder, hoard, file, false, false).await
    }

    /// Returns the latest backup operation recorded on any other machine (by UUID).
    ///
    /// `file`, if provided, must be a path relative to the root of one of the Hoard's Piles.
    ///
    /// # Errors
    ///
    /// - Any errors that occur while reading from the filesystem
    /// - Any parsing errors from `serde_json` when parsing the file
    #[tracing::instrument(level = "debug")]
    pub(crate) async fn latest_remote_backup(
        hoard: &HoardName,
        file: Option<(&PileName, &RelativePath)>,
        only_modified: bool,
    ) -> Result<Option<Self>, Error> {
        tracing::trace!("finding latest Operation file from remote machines");
        let uuid = super::get_or_generate_uuid().await?;
        let other_folders = super::get_history_dirs_not_for_id(&uuid).await?;
        tokio_stream::iter(other_folders.into_iter().map(Ok))
            .try_filter_map(|dir| async move {
                Self::latest_hoard_operation_from_local_dir(&dir, hoard, file, true, only_modified)
                    .await
            })
            .try_fold(None, Self::reduce_latest)
            .await?
            .map(Self::into_latest_version)
            .transpose()
    }

    #[tracing::instrument(level = "trace")]
    fn check_has_same_files(&self, remote: &Self) -> Result<Option<Vec<OperationFileInfo>>, Error> {
        let local_files: HashSet<OperationFileInfo> = self
            .as_latest_version()?
            .all_files_with_checksums()
            .collect();
        let remote_files: HashSet<OperationFileInfo> = remote
            .as_latest_version()?
            .all_files_with_checksums()
            .collect();
        if local_files == remote_files {
            Ok(None)
        } else {
            let remote_diffs = remote_files.difference(&local_files).cloned().collect();
            Ok(Some(remote_diffs))
        }
    }
}

#[async_trait::async_trait(? Send)]
impl Checker for Operation {
    type Error = Error;

    async fn new(
        hoards_root: &HoardPath,
        name: &HoardName,
        hoard: &Hoard,
        direction: Direction,
    ) -> Result<Self, Error> {
        Self::new(hoards_root, name, hoard, direction).await
    }

    #[tracing::instrument]
    async fn check(&mut self) -> Result<(), Error> {
        let last_local = Self::latest_local(self.hoard_name(), None).await?;
        let last_remote = Self::latest_remote_backup(self.hoard_name(), None, false).await?;

        let error = match self.0.direction() {
            Direction::Backup => Error::RestoreRequired,
            Direction::Restore => Error::BackupRequired,
        };

        match (last_local, last_remote) {
            (_, None) => Ok(()),
            (None, Some(last_remote)) => {
                self.check_has_same_files(&last_remote)
                    .map(|list_op| match list_op {
                        None => Ok(()),
                        Some(_) => crate::create_log_error(error),
                    })?
            }
            (Some(last_local), Some(last_remote)) => {
                if last_local.timestamp() > last_remote.timestamp() {
                    // Allow if the last operation on this machine
                    Ok(())
                } else {
                    match self.check_has_same_files(&last_remote)? {
                        None => Ok(()),
                        Some(list) => {
                            // Check if any of the files are unchanged compared to when last seen
                            // This can happen if the file is deleted and then recreated remotely
                            let matches_previous = list.into_iter().all(|item| {
                                item.checksum
                                    == last_local.checksum_for(&item.pile_name, &item.relative_path)
                            });
                            if matches_previous {
                                Ok(())
                            } else {
                                crate::create_log_error(error)
                            }
                        }
                    }
                }
            }
        }
    }

    #[tracing::instrument(level = "trace", name = "commit_operation_to_disk")]
    async fn commit_to_disk(self) -> Result<(), Error> {
        let id = super::get_or_generate_uuid().await?;
        let path = super::get_history_dir_for_id(id)
            .join(&RelativePath::from(self.hoard_name()))
            .join(
                &RelativePath::try_from(PathBuf::from(format!(
                    "{}.log",
                    self.timestamp()
                        .format(&TIME_FORMAT)
                        .map_err(Error::FormatDatetime)
                        .tap_err(crate::tap_log_error)?
                )))
                .expect("file name is always a valid RelativePath"),
            );
        tracing::trace!(path=%path.display(), "ensuring parent directories for operation log file");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.tap_err(|error| {
                tracing::error!(
                    %error,
                    "failed to create parent directory {} for operation log",
                    parent.display()
                );
            })?;
        }
        let content = serde_json::to_vec(&self).tap_err(|error| {
            tracing::error!(%error, "failed to serialize operation log as JSON");
        })?;
        fs::write(&path, &content).await.tap_err(|error| {
            tracing::error!(%error, "failed to write operation log file to {}", path.display());
        })?;
        Ok(())
    }
}
