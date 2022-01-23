use std::{fmt, fs};
use std::collections::HashMap;
use std::fmt::Formatter;
use std::io;
use std::iter::Peekable;
use std::path::{Path, PathBuf};

use super::{Direction, Hoard, HoardPath, SystemPath};
use crate::checkers::history::operation::{HoardOperation, Error as OperationError};
use crate::diff::{Diff, diff_files};
use crate::filters::Filter;
use crate::filters::{Error as FilterError, Filters};

use thiserror::Error;

pub(crate) enum DiffSource {
    Local,
    Remote,
    Unknown,
}

impl fmt::Display for DiffSource {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            DiffSource::Local => write!(f, "locally"),
            DiffSource::Remote => write!(f, "remotely"),
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
    root_paths: Vec<(HoardPath, SystemPath, Option<Filters>)>,
    direction: Direction,
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
                        vec![(hoard_path, system_path, filters)]
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
                        Ok((hoard_path, system_path, filters))
                    })
                })
                .collect::<Result<_, _>>()?,
        };

        Ok(Self {
            root_paths,
            direction,
            dir_entries: None,
            src_root: None,
            dest_root: None,
            filter: None,
        })
    }

    pub(crate) fn file_diffs(
        hoards_root: &Path,
        hoard_name: &str,
        hoard: &Hoard
    ) -> Result<Vec<HoardDiff>, Error> {
        let paths: HashMap<HoardPath, SystemPath> = Self::new(hoards_root, Direction::Backup, hoard_name, hoard)?
            .chain(Self::new(hoards_root, Direction::Restore, hoard_name, hoard)?)
            .collect::<Result<_, _>>()?;

        paths
            .into_iter()
            .filter_map(|(h, s)| {
                diff_files(h.as_ref(), s.as_ref()).transpose().map(|diff| (h, s, diff))
            })
            .map(move |(_hoard_path, system_path, diff)| {
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

                let has_remote_changes = HoardOperation::file_has_remote_changes(hoard_name, rel_path)?;
                let has_hoard_records = HoardOperation::file_has_records(hoard_name, rel_path)?;
                let has_local_records = HoardOperation::latest_local(hoard_name, Some(rel_path))?.is_some();

                let path = system_path.as_ref().to_owned();
                let diff_source = if has_remote_changes {
                    DiffSource::Remote
                } else {
                    DiffSource::Local
                };

                let hoard_diff = match diff? {
                    Diff::Binary => HoardDiff::BinaryModified {
                        path, diff_source
                    },
                    Diff::Text(unified_diff) => HoardDiff::TextModified {
                        path, diff_source, unified_diff
                    },
                    Diff::Permissions(hoard_perms, system_perms) => HoardDiff::PermissionsModified {
                        path, diff_source, hoard_perms, system_perms
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
            }).collect()
    }
}

impl Iterator for HoardFilesIter {
    type Item = io::Result<(HoardPath, SystemPath)>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Attempt to create direntry iterator.
            // If a path to a file is encountered, return that.
            // Otherwise, continue until existing directory is found.
            while self.dir_entries.is_none() || self.dir_entries.as_mut().unwrap().peek().is_none()
            {
                match self.root_paths.pop() {
                    None => return None,
                    Some((hoard_path, system_path, filters)) => {
                        let (src, dest) = match self.direction {
                            Direction::Backup => (&system_path.0, &hoard_path.0),
                            Direction::Restore => (&hoard_path.0, &system_path.0),
                        };

                        if src.is_file()
                            && filters
                                .as_ref()
                                .map_or(true, |filter| filter.keep(&PathBuf::new(), src))
                        {
                            return Some(Ok((hoard_path, system_path)));
                        } else if src.is_dir() {
                            self.src_root = Some(src.clone());
                            self.dest_root = Some(dest.clone());
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
                        return Some(Ok((hoard_path, system_path)));
                    } else if is_dir {
                        self.root_paths
                            .push((hoard_path, system_path, self.filter.clone()));
                    }
                }
            }
        }
    }
}
