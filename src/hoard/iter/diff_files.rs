#![allow(unused)]

use std::cmp::Ordering;
use std::fmt;
use std::fs::Permissions;
use std::pin::Pin;
use std::task::{Context, Poll};

use futures::{TryStream, TryStreamExt};
use tokio::io;
use tokio_stream::{Iter, Stream, StreamExt};
use tracing::trace_span;

use crate::checkers::history::operation::{Operation, OperationImpl, OperationType};
use crate::checksum::Checksum;
use crate::diff::Diff;
use crate::hoard::iter::Error;
use crate::hoard::Hoard;
use crate::hoard_item::{CachedHoardItem, HoardItem};
use crate::newtypes::HoardName;
use crate::paths::HoardPath;

use super::all_files::all_files_stream;

/// Indicates where a given change originated from.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DiffSource {
    /// The local machine.
    Local,
    /// Some other machine.
    Remote,
    /// Changes found both locally and on another machine.
    Mixed,
    /// Unknown source
    ///
    /// Likely someone edited a hoard file directly or a log file didn't get synced.
    Unknown,
}

impl fmt::Display for DiffSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DiffSource::Local => write!(f, "locally"),
            DiffSource::Remote => write!(f, "remotely"),
            DiffSource::Mixed => write!(f, "locally and remotely"),
            DiffSource::Unknown => write!(f, "out-of-band"),
        }
    }
}

/// What kind of diff occurred on the contained file, as well as useful associated info.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum HoardFileDiff {
    /// A binary file was modified.
    ///
    /// This applies if one or both of the associated files are non-text.
    BinaryModified {
        /// The associated file item.
        file: CachedHoardItem,
        /// The source of the change.
        diff_source: DiffSource,
    },
    /// A text file was modified.
    TextModified {
        /// The associated file item.
        file: CachedHoardItem,
        /// A unified diff describing the changes.
        ///
        /// Is `None` if `diff_source == DiffSource::Mixed` and the same change was applied in
        /// both locations.
        unified_diff: Option<String>,
        /// The source of the change.
        diff_source: DiffSource,
    },
    /// A file was created.
    Created {
        /// The associated file item.
        file: CachedHoardItem,
        /// A unified diff describing the changes.
        ///
        /// Is `Some(_)` if `diff_source == DiffSource::Mixed`, both files contain text,
        /// and the contents differ in either location.
        unified_diff: Option<String>,
        /// The source of the change.
        diff_source: DiffSource,
    },
    /// A file was deleted.
    Deleted {
        /// The associated file item.
        file: CachedHoardItem,
        /// The source of the change.
        diff_source: DiffSource,
    },
    /// A file is unchanged.
    Unchanged(CachedHoardItem),
    /// A file or path is directly listed in the configuration but does not exist anywhere.
    Nonexistent(CachedHoardItem),
}

#[derive(Debug, Clone)]
struct ProcessedFile {
    file: CachedHoardItem,
    diff: Option<Diff>,
    local_log_is_latest: bool,
    hoard_checksum: Option<Checksum>,
    system_checksum: Option<Checksum>,
    expected_hoard_checksum: Option<Checksum>,
    expected_system_checksum: Option<Checksum>,
    latest_local_log: Option<Operation>,
    latest_remote_log: Option<Operation>,
}

impl ProcessedFile {
    #[tracing::instrument(name = "process_file")]
    async fn process(hoard_name: &HoardName, file: CachedHoardItem) -> Result<Self, Error> {
        let _span = tracing::trace_span!("processing_file", hoard=%hoard_name, ?file).entered();
        let diff = file.diff().cloned();

        let latest_local_log =
            Operation::latest_local(hoard_name, Some((file.pile_name(), file.relative_path())))
                .await
                .map_err(Box::new)?
                .map(Operation::into_latest_version)
                .transpose()
                .map_err(Box::new)?;
        let latest_remote_log = Operation::latest_remote_backup(
            hoard_name,
            Some((file.pile_name(), file.relative_path())),
            true,
        )
        .await
        .map_err(Box::new)?
        .map(Operation::into_latest_version)
        .transpose()
        .map_err(Box::new)?;

        let (latest_op, local_log_is_latest) =
            match (latest_local_log.as_ref(), latest_remote_log.as_ref()) {
                (None, None) => (None, false),
                (Some(local), None) => (Some(local), true),
                (None, Some(remote)) => (Some(remote), false),
                (Some(local), Some(remote)) => {
                    if local.timestamp() > remote.timestamp() {
                        (Some(local), true)
                    } else {
                        (Some(remote), false)
                    }
                }
            };

        let expected_hoard_checksum = latest_op
            .as_ref()
            .and_then(|op| op.checksum_for(file.pile_name(), file.relative_path()));
        let hoard_checksum_type = expected_hoard_checksum
            .as_ref()
            .map(Checksum::typ)
            .unwrap_or_default();
        let hoard_checksum = file.hoard_checksum(hoard_checksum_type);

        let (expected_system_checksum, system_checksum_type) = if local_log_is_latest {
            (expected_hoard_checksum.clone(), hoard_checksum_type)
        } else {
            let expected_system_checksum = latest_local_log
                .as_ref()
                .and_then(|op| op.checksum_for(file.pile_name(), file.relative_path()));
            let system_checksum_type = expected_system_checksum
                .as_ref()
                .map(Checksum::typ)
                .unwrap_or_default();
            (expected_system_checksum, system_checksum_type)
        };

        let system_checksum = file.system_checksum(system_checksum_type);

        Ok(Self {
            file,
            diff,
            local_log_is_latest,
            hoard_checksum,
            system_checksum,
            expected_hoard_checksum,
            expected_system_checksum,
            latest_local_log,
            latest_remote_log,
        })
    }

    #[allow(clippy::too_many_lines)]
    #[tracing::instrument]
    fn get_hoard_diff(self) -> HoardFileDiff {
        let _span = tracing::trace_span!("get_diff", processed_file=?self).entered();
        let local_op_type = self.local_op_type();
        let remote_op_type = self.remote_op_type();
        let unexpected_op_type = self.unexpected_hoard_op();
        let has_logs = self.latest_remote_log.is_some() || self.latest_local_log.is_some();

        let file = self.file.clone();
        let _span = tracing::trace_span!("expected_diff", %has_logs, ?local_op_type, ?remote_op_type, diff=?self.diff).entered();

        #[allow(clippy::match_same_arms)]
        let expected_diff = match (
            has_logs,
            unexpected_op_type,
            local_op_type,
            remote_op_type,
            self.diff.clone(),
        ) {
            (_, Some(OperationType::Create), _, _, Some(Diff::Text(unified_diff))) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: Some(unified_diff),
                    diff_source: DiffSource::Unknown,
                }
            }
            (_, Some(OperationType::Create), _, _, _) => HoardFileDiff::Created {
                file,
                unified_diff: None,
                diff_source: DiffSource::Unknown,
            },
            (_, Some(OperationType::Delete), _, _, _) => HoardFileDiff::Deleted {
                file,
                diff_source: DiffSource::Unknown,
            },
            (_, Some(OperationType::Modify), _, _, None | Some(Diff::SystemNotExists)) => {
                if self.file.is_text() {
                    HoardFileDiff::TextModified {
                        file,
                        unified_diff: None,
                        diff_source: DiffSource::Unknown,
                    }
                } else {
                    HoardFileDiff::BinaryModified {
                        file,
                        diff_source: DiffSource::Unknown,
                    }
                }
            }
            (_, Some(OperationType::Modify), _, _, Some(Diff::Binary)) => {
                HoardFileDiff::BinaryModified {
                    file,
                    diff_source: DiffSource::Unknown,
                }
            }
            (_, Some(OperationType::Modify), _, _, Some(Diff::Text(unified_diff))) => {
                HoardFileDiff::TextModified {
                    file,
                    diff_source: DiffSource::Unknown,
                    unified_diff: Some(unified_diff),
                }
            }
            (_, Some(OperationType::Modify), _, _, Some(Diff::HoardNotExists)) => {
                unreachable!("cannot have modified hoard file if it does not exist")
            }
            (false, Some(OperationType::Modify), _, _, _) => {
                unreachable!("cannot modify a hoard file without operation logs")
            }
            // File/dir never existed in hoard but is listed as a named pile
            (false, None, None, None, None) => HoardFileDiff::Nonexistent(file),
            (true, None, None, None, None) => {
                if file.is_file() {
                    HoardFileDiff::Unchanged(file)
                } else {
                    HoardFileDiff::Nonexistent(file)
                }
            }
            (_, None, None, None, Some(_)) => {
                unreachable!("cannot have a diff if there are no changes");
            }
            (false, None, _, Some(_), _) => {
                unreachable!("cannot have remote changes without operation logs")
            }
            (false, None, Some(_), None, None) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
            (
                _,
                _,
                Some(OperationType::Delete),
                _,
                Some(Diff::HoardNotExists | Diff::Text(_) | Diff::Binary),
            ) => unreachable!("cannot have deleted local file and not detect it missing"),
            (false, None, _, _, Some(Diff::SystemNotExists)) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
            (
                true,
                None,
                None,
                Some(OperationType::Create | OperationType::Modify),
                Some(Diff::HoardNotExists),
            ) => {
                unreachable!("should have detected unexpected hoard file deletion");
            }
            (true, None, Some(OperationType::Create), None, Some(Diff::HoardNotExists)) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: None,
                    diff_source: DiffSource::Local,
                }
            }
            (true, None, Some(OperationType::Modify), None, Some(Diff::HoardNotExists)) => {
                unreachable!("should have detected unexpected hoard file deletion");
            }
            (
                true,
                None,
                Some(_),
                Some(OperationType::Create | OperationType::Modify),
                Some(Diff::HoardNotExists),
            ) => {
                unreachable!("should have detected unexpected hoard file deletion");
            }
            // If system file was created, last state was deleted or non-existent. If remote file was deleted, it is net even with current logged state of system.
            (
                true,
                None,
                Some(OperationType::Create),
                Some(OperationType::Delete),
                Some(Diff::HoardNotExists),
            ) => HoardFileDiff::Created {
                file,
                unified_diff: None,
                diff_source: DiffSource::Local,
            },
            (
                true,
                None,
                None | Some(OperationType::Modify),
                Some(OperationType::Delete),
                Some(Diff::HoardNotExists),
            ) => HoardFileDiff::Deleted {
                file,
                diff_source: DiffSource::Remote,
            },
            (false, None, Some(OperationType::Create), None, Some(Diff::HoardNotExists)) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: None,
                    diff_source: DiffSource::Local,
                }
            }
            (false, _, Some(OperationType::Modify), None, Some(Diff::HoardNotExists)) => {
                unreachable!("cannot modify local file if no logs exist")
            }
            (
                true,
                None,
                None,
                Some(OperationType::Create | OperationType::Modify),
                Some(Diff::SystemNotExists),
            ) => HoardFileDiff::Created {
                file,
                unified_diff: None,
                diff_source: DiffSource::Remote,
            },
            (true, None, None, Some(OperationType::Delete), Some(Diff::SystemNotExists)) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
            (
                _,
                _,
                Some(OperationType::Create | OperationType::Modify),
                _,
                Some(Diff::SystemNotExists),
            ) => unreachable!("cannot have created or modified local file while it doesn't exist"),
            (
                true,
                None,
                Some(OperationType::Delete),
                None | Some(OperationType::Modify | OperationType::Create),
                Some(Diff::SystemNotExists),
            ) => HoardFileDiff::Deleted {
                file,
                diff_source: DiffSource::Local,
            },
            (
                true,
                None,
                Some(OperationType::Delete),
                Some(OperationType::Delete),
                Some(Diff::SystemNotExists),
            ) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
            // Deleted and then recreated? Regardless, appears to this machine as modified
            (
                true,
                None,
                None,
                Some(OperationType::Create | OperationType::Modify),
                Some(Diff::Binary),
            ) => HoardFileDiff::BinaryModified {
                file,
                diff_source: DiffSource::Remote,
            },
            (true, None, None, Some(OperationType::Delete), Some(Diff::Binary)) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
            (true, None, Some(OperationType::Create), None, Some(Diff::Binary)) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
            (true, None, Some(OperationType::Modify), None, Some(Diff::Binary)) => {
                HoardFileDiff::BinaryModified {
                    file,
                    diff_source: DiffSource::Local,
                }
            }
            (
                true,
                None,
                Some(OperationType::Create),
                Some(OperationType::Create),
                Some(Diff::Binary),
            ) => HoardFileDiff::Created {
                file,
                unified_diff: None,
                diff_source: DiffSource::Mixed,
            },
            (
                true,
                None,
                Some(OperationType::Modify),
                Some(OperationType::Create),
                Some(Diff::Binary),
            ) => HoardFileDiff::BinaryModified {
                file,
                diff_source: DiffSource::Mixed,
            },
            (true, None, _, Some(OperationType::Delete), Some(Diff::Binary)) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
            (
                true,
                None,
                Some(OperationType::Create),
                Some(OperationType::Modify),
                Some(Diff::Binary),
            ) => HoardFileDiff::Created {
                file,
                unified_diff: None,
                diff_source: DiffSource::Mixed,
            },
            (
                true,
                None,
                Some(OperationType::Modify),
                Some(OperationType::Modify),
                Some(Diff::Binary),
            ) => HoardFileDiff::BinaryModified {
                file,
                diff_source: DiffSource::Mixed,
            },
            (false, None, _, None, Some(Diff::Binary)) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
            (
                true,
                None,
                None,
                Some(OperationType::Create | OperationType::Modify),
                Some(Diff::Text(unified_diff)),
            ) => HoardFileDiff::TextModified {
                file,
                unified_diff: Some(unified_diff),
                diff_source: DiffSource::Remote,
            },
            (true, None, Some(OperationType::Create), None, Some(Diff::Text(_))) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
            (true, None, Some(OperationType::Modify), None, Some(Diff::Text(unified_diff))) => {
                HoardFileDiff::TextModified {
                    file,
                    unified_diff: Some(unified_diff),
                    diff_source: DiffSource::Local,
                }
            }
            (true, None, _, Some(OperationType::Delete), Some(Diff::Text(_))) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
            (
                true,
                None,
                Some(OperationType::Create),
                Some(OperationType::Create),
                Some(Diff::Text(unified_diff)),
            ) => HoardFileDiff::Created {
                file,
                unified_diff: Some(unified_diff),
                diff_source: DiffSource::Mixed,
            },
            (
                true,
                None,
                Some(OperationType::Modify),
                Some(OperationType::Create),
                Some(Diff::Text(unified_diff)),
            ) => HoardFileDiff::TextModified {
                file,
                unified_diff: Some(unified_diff),
                diff_source: DiffSource::Mixed,
            },
            (
                true,
                None,
                Some(OperationType::Create),
                Some(OperationType::Modify),
                Some(Diff::Text(unified_diff)),
            ) => HoardFileDiff::Created {
                file,
                unified_diff: Some(unified_diff),
                diff_source: DiffSource::Mixed,
            },
            (
                true,
                None,
                Some(OperationType::Modify),
                Some(OperationType::Modify),
                Some(Diff::Text(unified_diff)),
            ) => HoardFileDiff::TextModified {
                file,
                unified_diff: Some(unified_diff),
                diff_source: DiffSource::Mixed,
            },
            (false, None, _, None, Some(Diff::Text(unified_diff))) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
            (true, None, None, Some(OperationType::Create | OperationType::Modify), None) => {
                HoardFileDiff::Deleted {
                    file,
                    diff_source: DiffSource::Unknown,
                }
            }
            (true, None, None, Some(OperationType::Delete), None) => HoardFileDiff::Deleted {
                file,
                diff_source: DiffSource::Remote,
            },
            (true, None, Some(OperationType::Create), None, None) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
            (true, None, Some(OperationType::Delete), None, None) => {
                unreachable!("should have detected unexpected hoard file deletion");
            }
            (true, None, Some(OperationType::Modify), None, None) => {
                unreachable!("should have detected unexpected hoard file modification");
            }
            (true, None, Some(OperationType::Create), Some(OperationType::Create), None) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: None,
                    diff_source: DiffSource::Mixed,
                }
            }
            (true, None, Some(OperationType::Create), Some(OperationType::Modify), None) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: None,
                    diff_source: DiffSource::Mixed,
                }
            }
            (true, None, Some(OperationType::Create), Some(OperationType::Delete), None) => {
                // If file was deleted remotely and created locally, there can be no diff only if
                // the file was recreated out-of-band in the hoard folder.
                unreachable!("should have detected unexpected hoard file creation");
            }
            (
                true,
                None,
                Some(OperationType::Delete),
                Some(OperationType::Create | OperationType::Modify),
                None,
            ) => {
                unreachable!("should have detected unexpected hoard file deletion");
            }
            (true, None, Some(OperationType::Delete), Some(OperationType::Delete), None) => {
                HoardFileDiff::Deleted {
                    file,
                    diff_source: DiffSource::Mixed,
                }
            }
            // Deleted and recreated (or just modified) remotely, but with the same modifications as local
            (
                true,
                None,
                Some(OperationType::Modify),
                Some(OperationType::Create | OperationType::Modify),
                None,
            ) => {
                if file.is_text() {
                    HoardFileDiff::TextModified {
                        file,
                        unified_diff: None,
                        diff_source: DiffSource::Mixed,
                    }
                } else {
                    HoardFileDiff::BinaryModified {
                        file,
                        diff_source: DiffSource::Mixed,
                    }
                }
            }
            (true, None, Some(OperationType::Modify), Some(OperationType::Delete), None) => {
                unreachable!("should have detected unexpected hoard file creation");
            }
        };
        tracing::trace!(?expected_diff);
        expected_diff
    }

    #[tracing::instrument]
    fn remote_op_type(&self) -> Option<OperationType> {
        (!self.local_log_is_latest).then(|| {
            self.latest_remote_log.as_ref().and_then(|op| {
                op.file_operation(self.file.pile_name(), self.file.relative_path())
                    .expect("getting file operation should not fail because operation should have been converted to latest version")
            })
        }).flatten()
    }

    fn local_op_type(&self) -> Option<OperationType> {
        match (
            self.expected_system_checksum.as_ref(),
            self.system_checksum.as_ref(),
        ) {
            (None, None) => None,
            (None, Some(_)) => Some(OperationType::Create),
            (Some(_), None) => Some(OperationType::Delete),
            (Some(expected), Some(current)) => (current != expected).then(|| OperationType::Modify),
        }
    }

    fn unexpected_hoard_op(&self) -> Option<OperationType> {
        match (
            self.hoard_checksum.as_ref(),
            self.expected_hoard_checksum.as_ref(),
        ) {
            (None, None) => None,
            (None, Some(_)) => Some(OperationType::Delete),
            (Some(_), None) => Some(OperationType::Create),
            (Some(left), Some(right)) => {
                if left == right {
                    None
                } else {
                    Some(OperationType::Modify)
                }
            }
        }
    }
}

/// A [`TryStream`] returning a [`HoardFileDiff`] for every Hoard-managed file in the given hoard.
///
/// # Errors
///
/// Any errors that may occur while creating the stream.
#[tracing::instrument]
pub async fn diff_stream(
    hoards_root: &HoardPath,
    hoard_name: HoardName,
    hoard: &Hoard,
) -> Result<impl TryStream<Ok = HoardFileDiff, Error = Error>, Error> {
    tracing::trace!("creating new diff stream");
    let stream = all_files_stream(hoards_root, &hoard_name, hoard)
        .await?
        .map_ok(move |file| (file, hoard_name.clone()))
        .and_then(|(file, hoard_name)| async move {
            let file = CachedHoardItem::try_from_hoard_item(file)
                .await
                .map_err(Error::IO)?;
            let _span = trace_span!("diff_iterator_next", ?file);
            let processed: ProcessedFile = ProcessedFile::process(&hoard_name, file).await?;
            Ok(processed.get_hoard_diff())
        });

    Ok(stream)
}

/// Like [`diff_stream`], but filters for modified files only.
///
/// # Errors
///
/// See [`diff_stream`]
#[tracing::instrument]
pub async fn changed_diff_only_stream(
    hoards_root: &HoardPath,
    hoard_name: HoardName,
    hoard: &Hoard,
) -> Result<impl Stream<Item = Result<HoardFileDiff, Error>>, Error> {
    let stream = diff_stream(hoards_root, hoard_name, hoard).await?;
    let stream = stream.try_filter_map(|item| async move {
        match item {
            HoardFileDiff::Unchanged(_) | HoardFileDiff::Nonexistent(_) => Ok(None),
            item => Ok(Some(item)),
        }
    });
    Ok(stream)
}
