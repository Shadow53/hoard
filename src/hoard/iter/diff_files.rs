use crate::checkers::history::operation::{
    Error as OperationError, Hoard as OpHoard, HoardOperation,
};
use crate::diff::{diff_files, Diff};
use crate::hoard::iter::all_files::{AllFilesIter, RootPathItem};
use crate::hoard::Hoard;
use md5::Digest;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};

#[derive(Copy, Clone, PartialEq)]
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

pub(crate) enum HoardDiff {
    BinaryModified {
        path: PathBuf,
        diff_source: DiffSource,
    },
    TextModified {
        path: PathBuf,
        unified_diff: String,
        diff_source: DiffSource,
    },
    PermissionsModified {
        path: PathBuf,
        hoard_perms: std::fs::Permissions,
        system_perms: std::fs::Permissions,
        diff_source: DiffSource,
    },
    Created {
        path: PathBuf,
        diff_source: DiffSource,
    },
    Recreated {
        path: PathBuf,
        diff_source: DiffSource,
    },
    Deleted {
        path: PathBuf,
        diff_source: DiffSource,
    },
}

#[allow(clippy::too_many_lines)]
pub(crate) fn file_diffs(
    hoards_root: &Path,
    hoard_name: &str,
    hoard: &Hoard,
) -> Result<Vec<HoardDiff>, super::Error> {
    let _span = tracing::trace_span!("file_diffs_iterator").entered();
    let paths: HashSet<RootPathItem> = AllFilesIter::new(hoards_root, hoard_name, hoard)?
        .collect::<Result<_, io::Error>>()
        .map_err(super::Error::IO)?;

    paths
        .into_iter()
        .filter_map(|item| {
            diff_files(item.hoard_path.as_ref(), item.system_path.as_ref())
                .transpose()
                .map(|diff| (item.pile_name, item.hoard_path, item.system_path, diff))
        })
        .map(move |(pile_name, hoard_path, system_path, diff)| {
            let prefix = match hoard {
                Hoard::Anonymous(pile) => pile
                    .path
                    .as_ref()
                    .expect("hoard path should be guaranteed here"),
                Hoard::Named(piles) => piles
                    .piles
                    .values()
                    .filter_map(|pile| pile.path.as_ref())
                    .find(|path| system_path.as_ref().starts_with(path))
                    .expect("path should always start with a pile path"),
            };

            let rel_path = system_path
                .as_ref()
                .strip_prefix(prefix)
                .expect("prefix should always match path");

            let has_same_permissions = {
                let hoard_perms = fs::File::open(hoard_path.as_ref())
                    .ok()
                    .as_ref()
                    .map(fs::File::metadata).and_then(Result::ok)
                    .as_ref()
                    .map(fs::Metadata::permissions);
                let system_perms = fs::File::open(system_path.as_ref())
                    .ok()
                    .as_ref()
                    .map(fs::File::metadata).and_then(Result::ok)
                    .as_ref()
                    .map(fs::Metadata::permissions);
                hoard_perms == system_perms
            };
            let has_remote_changes = HoardOperation::file_has_remote_changes(hoard_name, rel_path)?;
            let has_hoard_records = HoardOperation::file_has_records(hoard_name, rel_path)?;
            let local_record = HoardOperation::latest_local(hoard_name, Some(rel_path))?;
            let has_local_records = local_record.is_some();

            let has_local_content_changes = if let Some(HoardOperation { ref hoard, .. }) = local_record {
                tracing::trace!("operation hoard: {:?}, pile: {:?}, rel_path: {:?}", hoard, pile_name, rel_path);
                let checksum = match hoard {
                    OpHoard::Anonymous(op_pile) => {
                        op_pile.get(rel_path).map(ToOwned::to_owned)
                    },
                    OpHoard::Named(op_piles) => {
                        let pile_name = pile_name.expect("pile name should exist");
                        op_piles.get(&pile_name).and_then(|op_pile| op_pile.get(rel_path)).map(ToOwned::to_owned)
                    },
                };

                if let Some(checksum) = checksum {
                    tracing::trace!("{} ({}) previously had checksum {} on this system", system_path.as_ref().display(), rel_path.display(), checksum);
                    match fs::read(system_path.as_ref()) {
                        Err(err) => if let io::ErrorKind::NotFound = err.kind() {
                            false
                        } else {
                            return Err(OperationError::IO(err));
                        },
                        Ok(content) => {
                            let new_sum = format!("{:x}", md5::Md5::digest(&content));
                            tracing::trace!("{} currently has checksum {}", system_path.as_ref().display(), new_sum);
                            new_sum != checksum
                        }
                    }
                } else {
                    tracing::trace!("no checksum found for {}", system_path.as_ref().display());
                    false
                }
            } else {
                tracing::trace!(path=?system_path.as_ref(), "no local operation found for {}", hoard_name);
                system_path.as_ref().exists()
            };

            {
                let local_record = local_record.as_ref();
                tracing::trace!(%has_local_records, %has_hoard_records, %has_remote_changes, %has_same_permissions, %has_local_content_changes, ?local_record);
            }

            let path = system_path.as_ref().to_owned();
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

            let hoard_diff = match diff? {
                Diff::Binary => if created_mixed {
                    HoardDiff::Created {
                        path, diff_source: DiffSource::Mixed,
                    }
                } else {
                    HoardDiff::BinaryModified {
                        path, diff_source
                    }
                },
                Diff::Text(unified_diff) => if created_mixed {
                    HoardDiff::Created { path, diff_source: DiffSource::Mixed }
                } else {
                    HoardDiff::TextModified {
                        path, diff_source, unified_diff
                    }
                },
                Diff::Permissions(hoard_perms, system_perms) => HoardDiff::PermissionsModified {
                    // Cannot track sources of permissions changes, so just mark Mixed
                    path, diff_source: DiffSource::Mixed, hoard_perms, system_perms
                },
                Diff::LeftNotExists => {
                    // File not in hoard directory
                    if has_hoard_records {
                        // Used to exist in hoard directory
                        if has_remote_changes {
                            // Most recent operation is remote, probably deleted
                            HoardDiff::Deleted { path, diff_source: DiffSource::Remote }
                        } else {
                            // Most recent operation is local, probably recreated file
                            HoardDiff::Recreated { path, diff_source: DiffSource::Local }
                        }
                    } else {
                        // Never existed in hoard, newly created
                        HoardDiff::Created { path, diff_source: DiffSource::Local }
                    }
                },
                Diff::RightNotExists => {
                    // File not on system
                    if has_hoard_records {
                        // File exists in the hoard
                        if has_local_records {
                            if has_remote_changes {
                                // Recreated remotely
                                HoardDiff::Recreated { path, diff_source: DiffSource::Remote }
                            } else {
                                // Deleted locally
                                HoardDiff::Deleted { path, diff_source: DiffSource::Local }
                            }
                        } else {
                            // Created remotely
                            HoardDiff::Created { path, diff_source: DiffSource::Remote }
                        }
                    } else {
                        // Unknown
                        HoardDiff::Created { path, diff_source: DiffSource::Unknown }
                    }
                },
            };

            Ok(hoard_diff)
        }).collect::<Result<_, _>>().map_err(super::Error::from)
}
