use crate::checkers::history::operation::{Operation, OperationImpl, OperationType};
use crate::diff::{diff_files, Diff};
use crate::hoard::iter::all_files::AllFilesIter;
use crate::hoard_item::CachedHoardItem;
use crate::hoard::Hoard;
use std::cmp::Ordering;
use std::fmt;
use std::fs::Permissions;
use tracing::trace_span;

use crate::checksum::Checksum;
use crate::newtypes::HoardName;
use crate::paths::HoardPath;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum DiffSource {
    Local,
    Remote,
    Mixed,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum HoardFileDiff {
    BinaryModified {
        file: CachedHoardItem,
        diff_source: DiffSource,
    },
    TextModified {
        file: CachedHoardItem,
        unified_diff: String,
        diff_source: DiffSource,
    },
    PermissionsModified {
        file: CachedHoardItem,
        hoard_perms: Permissions,
        system_perms: Permissions,
        diff_source: DiffSource,
    },
    Created {
        file: CachedHoardItem,
        unified_diff: Option<String>,
        diff_source: DiffSource,
    },
    Deleted {
        file: CachedHoardItem,
        diff_source: DiffSource,
    },
    Unchanged(CachedHoardItem),
}

#[cfg(unix)]
fn compare_perms(left: &Permissions, right: &Permissions) -> Ordering {
    left.mode().cmp(&right.mode())
}

#[cfg(not(unix))]
fn compare_perms(left: &Permissions, right: &Permissions) -> Ordering {
    let left_readonly = left.readonly();
    let right_readonly = right.readonly();
    match (left_readonly, right_readonly) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (_, _) => Ordering::Equal,
    }
}

impl PartialOrd for HoardFileDiff {
    // Ordering doesn't matter too much, just go with the order of declaration.
    // Manual implementation because Permissions do not implement Hash or (Partial)Ord.
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for HoardFileDiff {
    #[allow(clippy::too_many_lines)]
    fn cmp(&self, other: &Self) -> Ordering {
        #[allow(clippy::match_same_arms)]
        match (self, other) {
            (
                Self::BinaryModified {
                    file: my_file,
                    diff_source: my_source,
                },
                Self::BinaryModified {
                    file: other_file,
                    diff_source: other_source,
                },
            ) => my_file.cmp(other_file).then(my_source.cmp(other_source)),
            (Self::BinaryModified { .. }, _) => Ordering::Less,
            (Self::TextModified { .. }, Self::BinaryModified { .. }) => Ordering::Greater,
            (
                Self::TextModified {
                    file: my_file,
                    diff_source: my_source,
                    unified_diff: my_diff,
                },
                Self::TextModified {
                    file: other_file,
                    diff_source: other_source,
                    unified_diff: other_diff,
                },
            ) => my_file
                .cmp(other_file)
                .then(my_source.cmp(other_source))
                .then(my_diff.cmp(other_diff)),
            (Self::TextModified { .. }, _) => Ordering::Less,
            (
                Self::PermissionsModified {
                    file: my_file,
                    hoard_perms: my_hoard_perms,
                    system_perms: my_sys_perms,
                    diff_source: my_source,
                },
                Self::PermissionsModified {
                    file: other_file,
                    hoard_perms: other_hoard_perms,
                    system_perms: other_sys_perms,
                    diff_source: other_source,
                },
            ) => my_file
                .cmp(other_file)
                .then(compare_perms(my_hoard_perms, other_hoard_perms))
                .then(compare_perms(my_sys_perms, other_sys_perms))
                .then(my_source.cmp(other_source)),
            (
                Self::PermissionsModified { .. },
                Self::BinaryModified { .. } | Self::TextModified { .. },
            ) => Ordering::Greater,
            (Self::PermissionsModified { .. }, _) => Ordering::Less,
            (Self::Created { .. }, Self::Deleted { .. } | Self::Unchanged(_)) => Ordering::Less,
            (
                Self::Created {
                    file: left_file,
                    unified_diff: left_diff,
                    diff_source: left_src,
                },
                Self::Created {
                    file: right_file,
                    unified_diff: right_diff,
                    diff_source: right_src,
                },
            ) => left_file
                .cmp(right_file)
                .then(left_diff.cmp(right_diff))
                .then(left_src.cmp(right_src)),
            (Self::Created { .. }, _) => Ordering::Greater,
            (
                Self::Deleted {
                    file: my_file,
                    diff_source: my_source,
                },
                Self::Deleted {
                    file: other_file,
                    diff_source: other_source,
                },
            ) => my_file.cmp(other_file).then(my_source.cmp(other_source)),
            (Self::Deleted { .. }, Self::Unchanged(_)) => Ordering::Less,
            (Self::Deleted { .. }, _) => Ordering::Greater,
            (Self::Unchanged(my_file), Self::Unchanged(other_file)) => my_file.cmp(other_file),
            (Self::Unchanged(_), _) => Ordering::Greater,
        }
    }
}

pub(crate) struct HoardDiffIter {
    iterator: AllFilesIter,
    hoard_name: HoardName,
}

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
    fn process(hoard_name: &HoardName, file: CachedHoardItem) -> Result<Self, super::Error> {
        let diff = diff_files(file.hoard_path(), file.system_path()).map_err(|err| {
            tracing::error!(
                "failed to diff {} and {}: {}",
                file.system_path().display(),
                file.hoard_path().display(),
                err
            );
            super::Error::IO(err)
        })?;

        let latest_local_log =
            Operation::latest_local(hoard_name, Some((file.pile_name(), file.relative_path())))
                .map_err(Box::new)?
                .map(Operation::into_latest_version)
                .transpose()
                .map_err(Box::new)?;
        let latest_remote_log = Operation::latest_remote_backup(
            hoard_name,
            Some((file.pile_name(), file.relative_path())),
            true,
        )
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

    fn get_hoard_diff(self) -> HoardFileDiff {
        self.unexpected_diff()
            .unwrap_or_else(|| self.expected_diff())
    }

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

    fn unexpected_diff(&self) -> Option<HoardFileDiff> {
        let diff_source = DiffSource::Unknown;
        match (self.unexpected_hoard_op(), self.diff.as_ref()) {
            // Can't keep track of permissions
            (None, _) | (Some(OperationType::Modify), Some(Diff::Permissions(..))) => None,
            (Some(OperationType::Create), Some(Diff::Text(unified_diff))) => {
                Some(HoardFileDiff::Created {
                    file: self.file.clone(),
                    unified_diff: Some(unified_diff.clone()),
                    diff_source,
                })
            }
            (Some(OperationType::Create), _) => Some(HoardFileDiff::Created {
                file: self.file.clone(),
                unified_diff: None,
                diff_source,
            }),
            (Some(OperationType::Delete), _) => Some(HoardFileDiff::Deleted {
                file: self.file.clone(),
                diff_source,
            }),
            (Some(OperationType::Modify), None | Some(Diff::Binary | Diff::SystemNotExists)) => {
                Some(HoardFileDiff::BinaryModified {
                    file: self.file.clone(),
                    diff_source,
                })
            }
            (Some(OperationType::Modify), Some(Diff::Text(unified_diff))) => {
                Some(HoardFileDiff::TextModified {
                    file: self.file.clone(),
                    diff_source,
                    unified_diff: unified_diff.clone(),
                })
            }
            (Some(OperationType::Modify), Some(Diff::HoardNotExists)) => unreachable!(""),
        }
    }

    #[allow(clippy::too_many_lines)]
    fn expected_diff(&self) -> HoardFileDiff {
        let local_op_type = self.local_op_type();
        let remote_op_type = self.remote_op_type();
        let has_logs = self.latest_remote_log.is_some() || self.latest_local_log.is_some();

        let file = self.file.clone();

        #[allow(clippy::match_same_arms)]
        match (has_logs, local_op_type, remote_op_type, self.diff.clone()) {
            (false, None, None, None) => unreachable!("file {} has never existed in Hoard", file.system_path().display()),
            (true, None, None, None) => HoardFileDiff::Unchanged(file),
            (_, None, None, Some(Diff::Permissions(hoard_perms, system_perms))) => {
                HoardFileDiff::PermissionsModified {
                    file,
                    hoard_perms,
                    system_perms,
                    diff_source: DiffSource::Unknown,
                }
            }
            (_, None, None, Some(_)) => {
                unreachable!("diff should not exist if there are no changes")
            }
            (false, _, Some(_), _) => {
                unreachable!("cannot have remote changes without operation logs")
            }
            (false, Some(_), None, None) => HoardFileDiff::Created {
                file, unified_diff: None, diff_source: DiffSource::Unknown
            },
            (
                _,
                Some(OperationType::Delete),
                _,
                Some(Diff::HoardNotExists | Diff::Text(_) | Diff::Permissions(..) | Diff::Binary)
            ) => unreachable!("cannot have deleted local file and not detect it missing"),
            (false, _, _, Some(Diff::SystemNotExists)) => HoardFileDiff::Created {
                file,
                unified_diff: None,
                diff_source: DiffSource::Unknown,
            },
            (
                true,
                None,
                Some(OperationType::Create | OperationType::Modify),
                Some(Diff::HoardNotExists),
            ) => HoardFileDiff::Deleted {
                file,
                diff_source: DiffSource::Unknown,
            },
            (true, Some(OperationType::Create), None, Some(Diff::HoardNotExists)) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: None,
                    diff_source: DiffSource::Local,
                }
            }
            (
                true,
                Some(OperationType::Modify),
                None,
                Some(Diff::HoardNotExists),
            ) => HoardFileDiff::Deleted {
                file,
                diff_source: DiffSource::Unknown,
            },
            (
                true,
                Some(_),
                Some(OperationType::Create | OperationType::Modify),
                Some(Diff::HoardNotExists),
            ) => HoardFileDiff::Deleted {
                file,
                diff_source: DiffSource::Unknown,
            },
            // If system file was created, last state was deleted or non-existent. If remote file was deleted, it is net even with current logged state of system.
            (
                true,
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
                None | Some(OperationType::Modify),
                Some(OperationType::Delete),
                Some(Diff::HoardNotExists),
            ) => HoardFileDiff::Deleted {
                file,
                diff_source: DiffSource::Remote,
            },
            (false, Some(OperationType::Create), None, Some(Diff::HoardNotExists)) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: None,
                    diff_source: DiffSource::Local,
                }
            }
            (
                false,
                Some(OperationType::Modify),
                None,
                Some(Diff::HoardNotExists),
            ) => unreachable!("cannot modify local file if no logs exist"),
            (
                true,
                None,
                Some(OperationType::Create | OperationType::Modify),
                Some(Diff::SystemNotExists),
            ) => HoardFileDiff::Created {
                file,
                unified_diff: None,
                diff_source: DiffSource::Remote,
            },
            (true, None, Some(OperationType::Delete), Some(Diff::SystemNotExists)) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: None,
                    diff_source: DiffSource::Unknown,
                }
            }
            (
                _,
                Some(OperationType::Create | OperationType::Modify),
                _,
                Some(Diff::SystemNotExists),
            ) => unreachable!("cannot have created or modified local file while it doesn't exist"),
            (
                true,
                Some(OperationType::Delete),
                None | Some(OperationType::Modify | OperationType::Create),
                Some(Diff::SystemNotExists),
            ) => HoardFileDiff::Deleted {
                file,
                diff_source: DiffSource::Local,
            },
            (
                true,
                Some(OperationType::Delete),
                Some(OperationType::Delete),
                Some(Diff::SystemNotExists),
            ) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: None,
                    diff_source: DiffSource::Unknown,
                }
            },
            (true, None, Some(_), Some(Diff::Permissions(hoard_perms, system_perms))) => {
                HoardFileDiff::PermissionsModified {
                    file,
                    hoard_perms,
                    system_perms,
                    diff_source: DiffSource::Unknown,
                }
            }
            (true, Some(_), None, Some(Diff::Permissions(hoard_perms, system_perms))) => {
                HoardFileDiff::PermissionsModified {
                    file,
                    hoard_perms,
                    system_perms,
                    diff_source: DiffSource::Unknown,
                }
            }
            (true, Some(_), Some(_), Some(Diff::Permissions(hoard_perms, system_perms))) => {
                HoardFileDiff::PermissionsModified {
                    file,
                    hoard_perms,
                    system_perms,
                    diff_source: DiffSource::Unknown,
                }
            }
            (false, Some(_), None, Some(Diff::Permissions(hoard_perms, system_perms))) => {
                HoardFileDiff::PermissionsModified {
                    file,
                    hoard_perms,
                    system_perms,
                    diff_source: DiffSource::Unknown,
                }
            }
            // Deleted and then recreated? Regardless, appears to this machine as modified
            (
                true,
                None,
                Some(OperationType::Create | OperationType::Modify),
                Some(Diff::Binary),
            ) => HoardFileDiff::BinaryModified {
                file,
                diff_source: DiffSource::Remote,
            },
            (true, None, Some(OperationType::Delete), Some(Diff::Binary)) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: None,
                    diff_source: DiffSource::Unknown,
                }
            }
            (true, Some(OperationType::Create), None, Some(Diff::Binary)) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: None,
                    diff_source: DiffSource::Unknown,
                }
            }
            (true, Some(OperationType::Modify), None, Some(Diff::Binary)) => {
                HoardFileDiff::BinaryModified {
                    file,
                    diff_source: DiffSource::Local,
                }
            }
            (
                true,
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
                Some(OperationType::Modify),
                Some(OperationType::Create),
                Some(Diff::Binary),
            ) => HoardFileDiff::BinaryModified {
                file,
                diff_source: DiffSource::Mixed,
            },
            (true, _, Some(OperationType::Delete), Some(Diff::Binary)) => HoardFileDiff::Created {
                file,
                unified_diff: None,
                diff_source: DiffSource::Unknown,
            },
            (
                true,
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
                Some(OperationType::Modify),
                Some(OperationType::Modify),
                Some(Diff::Binary),
            ) => HoardFileDiff::BinaryModified {
                file,
                diff_source: DiffSource::Mixed,
            },
            (false, _, None, Some(Diff::Binary)) => HoardFileDiff::Created {
                file,
                unified_diff: None,
                diff_source: DiffSource::Unknown,
            },
            (
                true,
                None,
                Some(OperationType::Create | OperationType::Modify),
                Some(Diff::Text(unified_diff)),
            ) => HoardFileDiff::TextModified {
                file,
                unified_diff,
                diff_source: DiffSource::Remote,
            },
            (true, Some(OperationType::Create), None, Some(Diff::Text(_))) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: None,
                    diff_source: DiffSource::Unknown,
                }
            }
            (true, Some(OperationType::Modify), None, Some(Diff::Text(unified_diff))) => {
                HoardFileDiff::TextModified {
                    file,
                    unified_diff,
                    diff_source: DiffSource::Local,
                }
            }
            (true, _, Some(OperationType::Delete), Some(Diff::Text(_))) => HoardFileDiff::Created {
                file,
                unified_diff: None,
                diff_source: DiffSource::Unknown,
            },
            (
                true,
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
                Some(OperationType::Modify),
                Some(OperationType::Create),
                Some(Diff::Text(unified_diff)),
            ) => HoardFileDiff::TextModified {
                file,
                unified_diff,
                diff_source: DiffSource::Mixed,
            },
            (
                true,
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
                Some(OperationType::Modify),
                Some(OperationType::Modify),
                Some(Diff::Text(unified_diff)),
            ) => HoardFileDiff::TextModified {
                file,
                unified_diff,
                diff_source: DiffSource::Mixed,
            },
            (false, _, None, Some(Diff::Text(unified_diff))) => HoardFileDiff::Created {
                file,
                unified_diff: Some(unified_diff),
                diff_source: DiffSource::Unknown,
            },
            (true, None, Some(OperationType::Create | OperationType::Modify), None) => HoardFileDiff::Deleted {
                file, diff_source: DiffSource::Unknown
            },
            (true, None, Some(OperationType::Delete), None) => unreachable!("file never existed locally and was deleted remotely, so should not have been diffed"),
            (true, Some(OperationType::Create), None, None) => HoardFileDiff::Created {
                file,
                unified_diff: None,
                diff_source: DiffSource::Unknown,
            },
            (true, Some(OperationType::Delete), None, None) => HoardFileDiff::Deleted {
                file,
                diff_source: DiffSource::Unknown,
            },
            // TODO: detect if text or binary
            (true, Some(OperationType::Modify), None, None) => HoardFileDiff::BinaryModified {
                file,
                diff_source: DiffSource::Unknown,
            },
            (true, Some(OperationType::Create), Some(OperationType::Create), None) => {
                HoardFileDiff::Created {
                    file,
                    unified_diff: None,
                    diff_source: DiffSource::Mixed,
                }
            },
            (true, Some(OperationType::Create), Some(OperationType::Modify), None) => HoardFileDiff::Created {
                file, unified_diff: None, diff_source: DiffSource::Mixed
            },
            (true, Some(OperationType::Create), Some(OperationType::Delete), None) => HoardFileDiff::Created {
                // If file was deleted remotely and created locally, there can be no diff only if
                // the file was recreated out-of-band in the hoard folder.
                file, unified_diff: None, diff_source: DiffSource::Unknown
            },
            (true, Some(OperationType::Delete), Some(OperationType::Create | OperationType::Modify), None) => HoardFileDiff::Deleted {
                file, diff_source: DiffSource::Unknown
            },
            (true, Some(OperationType::Delete), Some(OperationType::Delete), None) => HoardFileDiff::Deleted {
                file, diff_source: DiffSource::Mixed
            },
            // TODO: Text or binary?
            // Deleted and recreated (or just modified) remotely, but with the same modifications as local
            (true, Some(OperationType::Modify), Some(OperationType::Create | OperationType::Modify), None) => HoardFileDiff::BinaryModified {
                file, diff_source: DiffSource::Mixed
            },
            (true, Some(OperationType::Modify), Some(OperationType::Delete), None) => HoardFileDiff::Created {
                file, unified_diff: None, diff_source: DiffSource::Unknown
            },
        }
    }
}

impl HoardDiffIter {
    pub(crate) fn new(
        hoards_root: &HoardPath,
        hoard_name: HoardName,
        hoard: &Hoard,
    ) -> Result<Self, super::Error> {
        let _span = tracing::trace_span!("file_diffs_iterator").entered();
        tracing::trace!("creating new diff iterator");
        let iterator = AllFilesIter::new(hoards_root, &hoard_name, hoard)?;
        tracing::trace!("created diff iterator: {:?}", iterator);

        Ok(Self {
            iterator,
            hoard_name,
        })
    }

    pub(crate) fn only_changed(self) -> impl Iterator<Item = <Self as Iterator>::Item> {
        self.filter(|diff| !matches!(diff, Ok(HoardFileDiff::Unchanged(_))))
    }
}

impl Iterator for HoardDiffIter {
    type Item = Result<HoardFileDiff, super::Error>;

    #[allow(clippy::too_many_lines)]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(result) = self.iterator.by_ref().next() {
            let file: CachedHoardItem = super::propagate_error!(result.map_err(super::Error::IO));
            let _span = trace_span!("diff_iterator_next", ?file);
            let processed: ProcessedFile =
                super::propagate_error!(ProcessedFile::process(&self.hoard_name, file));
            return Some(Ok(processed.get_hoard_diff()));
        }

        None
    }
}
