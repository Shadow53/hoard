use crate::checkers::history::operation::{Operation, OperationImpl};
use crate::diff::{diff_files, Diff};
use crate::hoard::iter::all_files::AllFilesIter;
use crate::hoard::iter::HoardItem;
use crate::hoard::Hoard;
use std::cmp::Ordering;
use std::fs::Permissions;
use std::path::Path;
use std::{fmt, fs};
use tracing::trace_span;

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
        file: HoardItem,
        diff_source: DiffSource,
    },
    TextModified {
        file: HoardItem,
        unified_diff: String,
        diff_source: DiffSource,
    },
    PermissionsModified {
        file: HoardItem,
        hoard_perms: Permissions,
        system_perms: Permissions,
        diff_source: DiffSource,
    },
    Created {
        file: HoardItem,
        unified_diff: Option<String>,
        diff_source: DiffSource,
    },
    Recreated {
        file: HoardItem,
        unified_diff: Option<String>,
        diff_source: DiffSource,
    },
    Deleted {
        file: HoardItem,
        diff_source: DiffSource,
    },
    Unchanged(HoardItem),
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
            (
                Self::Created {
                    file: my_file,
                    diff_source: my_source,
                    unified_diff: my_diff,
                },
                Self::Created {
                    file: other_file,
                    diff_source: other_source,
                    unified_diff: other_diff,
                },
            ) => my_file
                .cmp(other_file)
                .then(my_source.cmp(other_source))
                .then(my_diff.cmp(other_diff)),
            (
                Self::Created { .. },
                Self::Recreated { .. } | Self::Deleted { .. } | Self::Unchanged(_),
            ) => Ordering::Less,
            (Self::Created { .. }, _) => Ordering::Greater,
            (
                Self::Recreated {
                    file: my_file,
                    diff_source: my_source,
                    unified_diff: my_diff,
                },
                Self::Recreated {
                    file: other_file,
                    diff_source: other_source,
                    unified_diff: other_diff,
                },
            ) => my_file
                .cmp(other_file)
                .then(my_source.cmp(other_source))
                .then(my_diff.cmp(other_diff)),
            (Self::Recreated { .. }, Self::Deleted { .. } | Self::Unchanged(_)) => Ordering::Less,
            (Self::Recreated { .. }, _) => Ordering::Greater,
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
    hoard_name: String,
}

#[allow(clippy::struct_excessive_bools)]
struct ProcessedFile {
    has_same_permissions: bool,
    has_remote_changes: bool,
    has_hoard_records: bool,
    has_local_records: bool,
    has_local_content_changes: bool,
}

impl HoardDiffIter {
    pub(crate) fn new(
        hoards_root: &Path,
        hoard_name: String,
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

    fn process_file(hoard_name: &str, file: &HoardItem) -> Result<ProcessedFile, super::Error> {
        let _span = tracing::trace_span!("process_file", ?file).entered();
        let has_same_permissions = {
            let hoard_perms = fs::File::open(file.hoard_path())
                .ok()
                .as_ref()
                .map(fs::File::metadata)
                .and_then(Result::ok)
                .as_ref()
                .map(fs::Metadata::permissions);
            let system_perms = fs::File::open(file.system_path())
                .ok()
                .as_ref()
                .map(fs::File::metadata)
                .and_then(Result::ok)
                .as_ref()
                .map(fs::Metadata::permissions);
            hoard_perms == system_perms
        };

        let has_remote_changes =
            Operation::file_has_remote_changes(hoard_name, file.pile_name(), file.relative_path())
                .map_err(Box::new)?;
        let has_hoard_records =
            Operation::file_has_records(hoard_name, file.pile_name(), file.relative_path())
                .map_err(Box::new)?;
        let local_record =
            Operation::latest_local(hoard_name, Some((file.pile_name(), file.relative_path())))
                .map_err(Box::new)?;
        let has_local_records = local_record.is_some();

        let has_local_content_changes = if let Some(operation) = local_record {
            tracing::trace!(
                "hoard: {:?}, pile: {:?}, rel_path: {:?}",
                hoard_name,
                file.pile_name(),
                file.relative_path()
            );
            let checksum = operation.checksum_for(file.pile_name(), file.relative_path());

            if let Some(checksum) = checksum {
                tracing::trace!(
                    "{} previously had checksum {} on this system",
                    file.system_path().display(),
                    checksum
                );
                file.system_checksum(checksum.typ())?
                    .map_or(false, |new_hash| {
                        tracing::trace!(
                            "{} currently has checksum {}",
                            file.system_path().display(),
                            new_hash
                        );
                        new_hash != checksum
                    })
            } else {
                tracing::trace!("no checksum found for {}", file.system_path().display());
                true
            }
        } else {
            tracing::trace!(path=?file.system_path(), "no local operation found for {}", hoard_name);
            file.system_path().exists()
        };

        Ok(ProcessedFile {
            has_same_permissions,
            has_remote_changes,
            has_hoard_records,
            has_local_records,
            has_local_content_changes,
        })
    }
}

impl Iterator for HoardDiffIter {
    type Item = Result<HoardFileDiff, super::Error>;

    #[allow(clippy::too_many_lines)]
    fn next(&mut self) -> Option<Self::Item> {
        if let Some(result) = self.iterator.by_ref().next() {
            let file = super::propagate_error!(result.map_err(super::Error::IO));

            let _span = trace_span!("diff_iterator_next", ?file);

            let diff = match diff_files(file.hoard_path(), file.system_path()) {
                Ok(Some(diff)) => diff,
                Ok(None) => return Some(Ok(HoardFileDiff::Unchanged(file))),
                Err(err) => {
                    tracing::error!(
                        "failed to diff {} and {}: {}",
                        file.system_path().display(),
                        file.hoard_path().display(),
                        err
                    );
                    return Some(Err(super::Error::IO(err)));
                }
            };

            let file_data = super::propagate_error!(Self::process_file(&self.hoard_name, &file));

            let ProcessedFile {
                has_same_permissions,
                has_remote_changes,
                has_hoard_records,
                has_local_records,
                has_local_content_changes,
            } = file_data;

            let diff_source = if has_remote_changes {
                if has_local_content_changes || !has_same_permissions {
                    DiffSource::Mixed
                } else {
                    DiffSource::Remote
                }
            } else if has_local_content_changes || !has_same_permissions {
                DiffSource::Local
            } else {
                DiffSource::Unknown
            };

            let created_mixed =
                has_remote_changes && !has_local_records && has_local_content_changes;

            let hoard_diff = match diff {
                Diff::Binary => {
                    if created_mixed {
                        HoardFileDiff::Created {
                            file,
                            diff_source: DiffSource::Mixed,
                            unified_diff: None,
                        }
                    } else {
                        HoardFileDiff::BinaryModified { file, diff_source }
                    }
                }
                Diff::Text(unified_diff) => {
                    if created_mixed {
                        HoardFileDiff::Created {
                            file,
                            diff_source: DiffSource::Mixed,
                            unified_diff: Some(unified_diff),
                        }
                    } else {
                        HoardFileDiff::TextModified {
                            file,
                            diff_source,
                            unified_diff,
                        }
                    }
                }
                Diff::Permissions(hoard_perms, system_perms) => {
                    HoardFileDiff::PermissionsModified {
                        // Cannot track sources of permissions changes, so just mark Mixed
                        file,
                        diff_source: DiffSource::Mixed,
                        hoard_perms,
                        system_perms,
                    }
                }
                Diff::LeftNotExists => {
                    // File not in hoard directory
                    if has_hoard_records {
                        // Used to exist in hoard directory
                        if has_remote_changes {
                            // Most recent operation is remote, probably deleted
                            HoardFileDiff::Deleted {
                                file,
                                diff_source: DiffSource::Remote,
                            }
                        } else {
                            // Most recent operation is local, probably recreated file
                            HoardFileDiff::Recreated {
                                file,
                                diff_source: DiffSource::Local,
                                unified_diff: None,
                            }
                        }
                    } else {
                        // Never existed in hoard, newly created
                        HoardFileDiff::Created {
                            file,
                            diff_source: DiffSource::Local,
                            unified_diff: None,
                        }
                    }
                }
                Diff::RightNotExists => {
                    // File not on system
                    if has_hoard_records {
                        // File exists in the hoard
                        if has_local_records {
                            if has_remote_changes {
                                // Recreated remotely
                                HoardFileDiff::Recreated {
                                    file,
                                    diff_source: DiffSource::Remote,
                                    unified_diff: None,
                                }
                            } else {
                                // Deleted locally
                                HoardFileDiff::Deleted {
                                    file,
                                    diff_source: DiffSource::Local,
                                }
                            }
                        } else {
                            // Created remotely
                            HoardFileDiff::Created {
                                file,
                                diff_source: DiffSource::Remote,
                                unified_diff: None,
                            }
                        }
                    } else {
                        // Unknown
                        HoardFileDiff::Created {
                            file,
                            diff_source: DiffSource::Unknown,
                            unified_diff: None,
                        }
                    }
                }
            };

            return Some(Ok(hoard_diff));
        }

        None
    }
}
