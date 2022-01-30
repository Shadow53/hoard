use crate::checkers::history::operation::{
    Error as OperationError, Hoard as OpHoard, HoardOperation,
};
use crate::diff::{diff_files, Diff};
use crate::hoard::iter::all_files::AllFilesIter;
use crate::hoard::Hoard;
use md5::Digest;
use std::path::Path;
use std::{fmt, fs, io};
use tracing::trace_span;
use crate::hoard::iter::HoardFile;

#[derive(Debug, Copy, Clone, PartialEq)]
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

#[derive(Debug, Clone)]
pub(crate) enum HoardFileDiff {
    BinaryModified {
        file: HoardFile,
        diff_source: DiffSource,
    },
    TextModified {
        file: HoardFile,
        unified_diff: String,
        diff_source: DiffSource,
    },
    PermissionsModified {
        file: HoardFile,
        hoard_perms: std::fs::Permissions,
        system_perms: std::fs::Permissions,
        diff_source: DiffSource,
    },
    Created {
        file: HoardFile,
        diff_source: DiffSource,
    },
    Recreated {
        file: HoardFile,
        diff_source: DiffSource,
    },
    Deleted {
        file: HoardFile,
        diff_source: DiffSource,
    },
}

pub(crate) struct HoardDiffIter{
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

        Ok(Self { iterator, hoard_name })
    }

    fn process_file(hoard_name: &str, file: &HoardFile) -> Result<ProcessedFile, super::Error> {
        let _span = tracing::trace_span!("process_file", ?file).entered();
        let has_same_permissions = {
            let hoard_perms = fs::File::open(file.hoard_path())
                .ok()
                .as_ref()
                .map(fs::File::metadata).and_then(Result::ok)
                .as_ref()
                .map(fs::Metadata::permissions);
            let system_perms = fs::File::open(file.system_path())
                .ok()
                .as_ref()
                .map(fs::File::metadata).and_then(Result::ok)
                .as_ref()
                .map(fs::Metadata::permissions);
            hoard_perms == system_perms
        };
        let has_remote_changes = HoardOperation::file_has_remote_changes(hoard_name, file.relative_path())?;
        let has_hoard_records = HoardOperation::file_has_records(hoard_name, file.relative_path())?;
        let local_record = HoardOperation::latest_local(hoard_name, Some(file.relative_path()))?;
        let has_local_records = local_record.is_some();

        let has_local_content_changes = if let Some(HoardOperation { ref hoard, .. }) = local_record {
            tracing::trace!("operation hoard: {:?}, pile: {:?}, rel_path: {:?}", hoard, file.pile_name(), file.relative_path());
            let checksum = match hoard {
                OpHoard::Anonymous(op_pile) => {
                    op_pile.get(file.relative_path()).map(ToOwned::to_owned)
                },
                OpHoard::Named(op_piles) => {
                    let pile_name = file.pile_name().expect("pile name should exist");
                    op_piles.get(pile_name).and_then(|op_pile| op_pile.get(file.relative_path())).map(ToOwned::to_owned)
                },
            };

            if let Some(checksum) = checksum {
                tracing::trace!("{} ({}) previously had checksum {} on this system", file.system_path().display(), file.relative_path().display(), checksum);
                match fs::read(file.system_path()) {
                    Err(err) => if let io::ErrorKind::NotFound = err.kind() {
                        false
                    } else {
                        return Err(super::Error::Operation(OperationError::IO(err)));
                    },
                    Ok(content) => {
                        let new_sum = format!("{:x}", md5::Md5::digest(&content));
                        tracing::trace!("{} currently has checksum {}", file.system_path().display(), new_sum);
                        new_sum != checksum
                    }
                }
            } else {
                tracing::trace!("no checksum found for {}", file.system_path().display());
                false
            }
        } else {
            tracing::trace!(path=?file.system_path(), "no local operation found for {}", hoard_name);
            file.system_path().exists()
        };

        Ok(ProcessedFile { has_same_permissions, has_remote_changes, has_hoard_records, has_local_records, has_local_content_changes })
    }
}

impl Iterator for HoardDiffIter {
    type Item = Result<HoardFileDiff, super::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        for result in self.iterator.by_ref() {
            let file = match result {
                Ok(file) => file,
                Err(err) => return Some(Err(super::Error::IO(err))),
            };

            let _span = trace_span!("diff_iterator_next", ?file);

            let diff = match diff_files(file.hoard_path(), file.system_path()) {
                Ok(None) => {
                    tracing::trace!("no diff found for file");
                    continue
                },
                Ok(Some(diff)) => diff,
                Err(err) => return Some(Err(super::Error::IO(err))),
            };

            let file_data = match Self::process_file(&self.hoard_name, &file) {
                Ok(data) => data,
                Err(err) => return Some(Err(err)),
            };

            let ProcessedFile {
                has_same_permissions, has_remote_changes, has_hoard_records, has_local_records, has_local_content_changes
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

            let created_mixed = has_remote_changes && !has_local_records && has_local_content_changes;

            let hoard_diff = match diff {
                Diff::Binary => if created_mixed {
                    HoardFileDiff::Created {
                        file, diff_source: DiffSource::Mixed,
                    }
                } else {
                    HoardFileDiff::BinaryModified {
                        file, diff_source
                    }
                },
                Diff::Text(unified_diff) => if created_mixed {
                    HoardFileDiff::Created { file, diff_source: DiffSource::Mixed }
                } else {
                    HoardFileDiff::TextModified {
                        file, diff_source, unified_diff
                    }
                },
                Diff::Permissions(hoard_perms, system_perms) => HoardFileDiff::PermissionsModified {
                    // Cannot track sources of permissions changes, so just mark Mixed
                    file, diff_source: DiffSource::Mixed, hoard_perms, system_perms
                },
                Diff::LeftNotExists => {
                    // File not in hoard directory
                    if has_hoard_records {
                        // Used to exist in hoard directory
                        if has_remote_changes {
                            // Most recent operation is remote, probably deleted
                            HoardFileDiff::Deleted { file, diff_source: DiffSource::Remote }
                        } else {
                            // Most recent operation is local, probably recreated file
                            HoardFileDiff::Recreated { file, diff_source: DiffSource::Local }
                        }
                    } else {
                        // Never existed in hoard, newly created
                        HoardFileDiff::Created { file, diff_source: DiffSource::Local }
                    }
                },
                Diff::RightNotExists => {
                    // File not on system
                    if has_hoard_records {
                        // File exists in the hoard
                        if has_local_records {
                            if has_remote_changes {
                                // Recreated remotely
                                HoardFileDiff::Recreated { file, diff_source: DiffSource::Remote }
                            } else {
                                // Deleted locally
                                HoardFileDiff::Deleted { file, diff_source: DiffSource::Local }
                            }
                        } else {
                            // Created remotely
                            HoardFileDiff::Created { file, diff_source: DiffSource::Remote }
                        }
                    } else {
                        // Unknown
                        HoardFileDiff::Created { file, diff_source: DiffSource::Unknown }
                    }
                },
            };

            return Some(Ok(hoard_diff))
        }

        None
    }
}