use md5::Digest;
use std::collections::HashSet;
use std::fmt::Formatter;
use std::io;
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use std::{fmt, fs};

use super::{Direction, Hoard, HoardPath, SystemPath};
use crate::checkers::history::operation::{
    Error as OperationError, Hoard as OpHoard, HoardOperation,
};
use crate::diff::{diff_files, Diff};
use crate::filters::Filter;
use crate::filters::{Error as FilterError, Filters};

use thiserror::Error;

#[derive(Copy, Clone, PartialEq)]
pub(crate) enum DiffSource {
    Local,
    Remote,
    Mixed,
    Unknown,
}

impl fmt::Display for DiffSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to create diff: {0}")]
    Diff(#[from] FilterError),
    #[error("I/O error occurred: {0}")]
    IO(#[from] std::io::Error),
    #[error("failed to check hoard operations: {0}")]
    Operation(#[from] OperationError),
}

pub(crate) struct HoardFilesIter {
    root_paths: Vec<(Option<String>, HoardPath, SystemPath, Option<Filters>)>,
    direction: Direction,
    pile_name: Option<String>,
    dir_entries: Option<Peekable<fs::ReadDir>>,
    src_root: Option<PathBuf>,
    dest_root: Option<PathBuf>,
    filter: Option<Filters>,
}

impl HoardFilesIter {
    pub(crate) fn new(
        hoards_root: &Path,
        direction: Direction,
        hoard_name: &str,
        hoard: &Hoard,
    ) -> Result<Self, Error> {
        let root_paths = match hoard {
            Hoard::Anonymous(pile) => {
                let path = pile.path.clone();
                let filters = pile.config.as_ref().map(Filters::new).transpose()?;
                match path {
                    None => Vec::new(),
                    Some(path) => {
                        let hoard_path = HoardPath(hoards_root.join(hoard_name));
                        let system_path = SystemPath(path);
                        vec![(None, hoard_path, system_path, filters)]
                    }
                }
            }
            Hoard::Named(piles) => piles
                .piles
                .iter()
                .filter_map(|(name, pile)| {
                    let filters = match pile.config.as_ref().map(Filters::new).transpose() {
                        Ok(filters) => filters,
                        Err(err) => return Some(Err(err)),
                    };
                    pile.path.as_ref().map(|path| {
                        let hoard_path = HoardPath(hoards_root.join(hoard_name).join(name));
                        let system_path = SystemPath(path.clone());
                        Ok((Some(name.clone()), hoard_path, system_path, filters))
                    })
                })
                .collect::<Result<_, _>>()?,
        };

        Ok(Self {
            root_paths,
            direction,
            pile_name: None,
            dir_entries: None,
            src_root: None,
            dest_root: None,
            filter: None,
        })
    }

    #[allow(clippy::too_many_lines)]
    pub(crate) fn file_diffs(
        hoards_root: &Path,
        hoard_name: &str,
        hoard: &Hoard,
    ) -> Result<Vec<HoardDiff>, Error> {
        let _span = tracing::trace_span!("file_diffs_iterator").entered();
        let paths: HashSet<(Option<String>, HoardPath, SystemPath)> =
            Self::new(hoards_root, Direction::Backup, hoard_name, hoard)?
                .chain(Self::new(
                    hoards_root,
                    Direction::Restore,
                    hoard_name,
                    hoard,
                )?)
                .collect::<Result<_, _>>()?;

        paths
            .into_iter()
            .filter_map(|(pile_name, h, s)| {
                diff_files(h.as_ref(), s.as_ref()).transpose().map(|diff| (pile_name, h, s, diff))
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
            }).collect::<Result<_, _>>().map_err(Error::from)
    }
}

impl Iterator for HoardFilesIter {
    type Item = io::Result<(Option<String>, HoardPath, SystemPath)>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Attempt to create direntry iterator.
            // If a path to a file is encountered, return that.
            // Otherwise, continue until existing directory is found.
            while self.dir_entries.is_none() || self.dir_entries.as_mut().unwrap().peek().is_none()
            {
                match self.root_paths.pop() {
                    None => return None,
                    Some((pile_name, hoard_path, system_path, filters)) => {
                        let (src, dest) = match self.direction {
                            Direction::Backup => (&system_path.0, &hoard_path.0),
                            Direction::Restore => (&hoard_path.0, &system_path.0),
                        };

                        if src.is_file()
                            && filters
                                .as_ref()
                                .map_or(true, |filter| filter.keep(&PathBuf::new(), src))
                        {
                            return Some(Ok((pile_name, hoard_path, system_path)));
                        } else if src.is_dir() {
                            self.src_root = Some(src.clone());
                            self.dest_root = Some(dest.clone());
                            self.pile_name = pile_name;
                            self.filter = filters;
                            match fs::read_dir(src) {
                                Ok(iter) => self.dir_entries = Some(iter.peekable()),
                                Err(err) => return Some(Err(err)),
                            }
                        }
                    }
                }
            }

            while let Some(entry) = self.dir_entries.as_mut().unwrap().next() {
                let entry = match entry {
                    Ok(entry) => entry,
                    Err(err) => return Some(Err(err)),
                };

                let src = entry.path();
                let dest = self
                    .dest_root
                    .as_ref()
                    .expect("dest_root should not be None")
                    .join(entry.file_name());
                let is_file = src.is_file();
                let is_dir = src.is_dir();

                let keep = match self.direction {
                    Direction::Backup => {
                        let prefix = self.src_root.as_ref().expect("src_root should not be None");
                        self.filter
                            .as_ref()
                            .map_or(true, |filter| filter.keep(prefix, &src))
                    }
                    Direction::Restore => {
                        let prefix = self
                            .dest_root
                            .as_ref()
                            .expect("dest_root should not be None");
                        self.filter
                            .as_ref()
                            .map_or(true, |filter| filter.keep(prefix, &dest))
                    }
                };

                let (hoard_path, system_path) = match self.direction {
                    Direction::Backup => (HoardPath(dest), SystemPath(src)),
                    Direction::Restore => (HoardPath(src), SystemPath(dest)),
                };

                if keep {
                    if is_file {
                        return Some(Ok((self.pile_name.clone(), hoard_path, system_path)));
                    } else if is_dir {
                        self.root_paths.push((
                            self.pile_name.clone(),
                            hoard_path,
                            system_path,
                            self.filter.clone(),
                        ));
                    }
                }
            }
        }
    }
}
