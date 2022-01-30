use std::{fs, io};
use std::iter::Peekable;
use std::path::Path;
use crate::filters::{Filter, Filters};
use crate::hoard::{Direction, Hoard, HoardPath, SystemPath};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct RootPathItem {
    pub(crate) pile_name: Option<String>,
    pub(crate) hoard_path: HoardPath,
    hoard_prefix: HoardPath,
    pub(crate) system_path: SystemPath,
    system_prefix: SystemPath,
    filters: Option<Filters>,
}

impl RootPathItem {
    fn keep(&self) -> bool {
        (self.is_file() || self.is_dir()) && self.filters.as_ref()
            .map_or(true, |filters| filters.keep(self.system_prefix.as_ref(), self.system_path.as_ref()))
    }

    fn is_file(&self) -> bool {
        let sys = self.system_path.as_ref();
        let hoard = self.hoard_path.as_ref();
        let sys_exists = sys.exists();
        let hoard_exists = hoard.exists();
        (sys.is_file() || !sys_exists) && (hoard.is_file() || !hoard_exists) && (sys_exists || hoard_exists)
    }

    fn is_dir(&self) -> bool {
        let sys = self.system_path.as_ref();
        let hoard = self.hoard_path.as_ref();
        let sys_exists = sys.exists();
        let hoard_exists = hoard.exists();
        (sys.is_dir() || !sys_exists) && (hoard.is_dir() || !hoard_exists) && (sys_exists || hoard_exists)
    }
}

pub(crate) struct AllFilesIter {
    root_paths: Vec<RootPathItem>,
    direction: Direction,
    src_entries: Option<Peekable<fs::ReadDir>>,
    dest_entries: Option<Peekable<fs::ReadDir>>,
    current_root: Option<RootPathItem>,
}

impl AllFilesIter {
    pub(crate) fn new(
        hoards_root: &Path,
        direction: Direction,
        hoard_name: &str,
        hoard: &Hoard,
    ) -> Result<Self, super::Error> {
        let root_paths = match hoard {
            Hoard::Anonymous(pile) => {
                let path = pile.path.clone();
                let filters = pile.config.as_ref().map(Filters::new).transpose()?;
                match path {
                    None => Vec::new(),
                    Some(path) => {
                        let hoard_path = HoardPath(hoards_root.join(hoard_name));
                        let system_path = SystemPath(path);
                        vec![RootPathItem { pile_name: None, hoard_prefix: hoard_path.clone(), hoard_path, system_prefix: system_path.clone(), system_path, filters }]
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
                        Ok(RootPathItem{ pile_name: Some(name.clone()), hoard_prefix: hoard_path.clone(), hoard_path, system_prefix: system_path.clone(), system_path, filters})
                    })
                })
                .collect::<Result<_, _>>()?,
        };

        Ok(Self {
            root_paths,
            direction,
            src_entries: None,
            dest_entries: None,
            current_root: None
        })
    }
}

impl AllFilesIter {
    fn has_dir_entries(&mut self) -> bool {
        if let Some(src_entries) = self.src_entries.as_mut() {
            if src_entries.peek().is_some() {
                return true;
            }
        }

        if let Some(dest_entries) = self.dest_entries.as_mut() {
            if dest_entries.peek().is_some() {
                return true;
            }
        }

        false
    }
}

impl Iterator for AllFilesIter {
    type Item = io::Result<RootPathItem>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Attempt to create direntry iterator.
            // If a path to a file is encountered, return that.
            // Otherwise, continue until existing directory is found.
            while !self.has_dir_entries() {
                match self.root_paths.pop() {
                    None => return None,
                    Some(item) => {
                        let (src, dest) = match self.direction {
                            Direction::Backup => (item.system_path.as_ref(), item.hoard_path.as_ref()),
                            Direction::Restore => (item.hoard_path.as_ref(), item.system_path.as_ref()),
                        };

                        if item.keep() {
                            if item.is_file() {
                                return Some(Ok(item));
                            } else if item.is_dir() {
                                match fs::read_dir(src) {
                                    Ok(iter) => self.src_entries = Some(iter.peekable()),
                                    Err(err) => match err.kind() {
                                        io::ErrorKind::NotFound => self.src_entries = None,
                                        _ => return Some(Err(err)),
                                    },
                                }
                                match fs::read_dir(dest) {
                                    Ok(iter) => self.dest_entries = Some(iter.peekable()),
                                    Err(err) => match err.kind() {
                                        io::ErrorKind::NotFound => self.dest_entries = None,
                                        _ => return Some(Err(err)),
                                    },
                                }
                                self.current_root = Some(item);
                            }
                        }
                    }
                }
            }

            let current_root = self.current_root.as_ref().expect("current_root should not be None");

            if let Some(src_entries) = self.src_entries.as_mut() {
                for entry in src_entries {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(err) => return Some(Err(err)),
                    };

                    let rel_path = match self.direction {
                        Direction::Backup => entry.path().strip_prefix(
                            current_root
                                .system_prefix
                                .as_ref()
                        ).expect("system prefix should always match path").to_path_buf(),
                        Direction::Restore => entry.path().strip_prefix(
                            current_root
                                .hoard_prefix
                                .as_ref()
                        ).expect("hoard prefix should always match path").to_path_buf(),
                    };

                    let new_item = RootPathItem {
                        hoard_path: HoardPath(current_root.hoard_prefix.as_ref().join(&rel_path)),
                        system_path: SystemPath(current_root.system_prefix.as_ref().join(rel_path)),
                        .. current_root.clone()
                    };

                    if new_item.keep() {
                        if new_item.is_file() {
                            return Some(Ok(new_item));
                        } else if new_item.is_dir() {
                            self.root_paths.push(new_item);
                        }
                    }
                }
            }

            if let Some(dest_entries) = self.dest_entries.as_mut() {
                for entry in dest_entries {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(err) => return Some(Err(err)),
                    };

                    let rel_path = match self.direction {
                        Direction::Backup => entry.path().strip_prefix(
                            current_root
                                .hoard_prefix
                                .as_ref()
                        ).expect("hoard prefix should always match path").to_path_buf(),
                        Direction::Restore => entry.path().strip_prefix(
                            current_root
                                .system_prefix
                                .as_ref()
                        ).expect("system prefix should always match path").to_path_buf(),
                    };

                    let new_item = RootPathItem {
                        hoard_path: HoardPath(current_root.hoard_prefix.as_ref().join(&rel_path)),
                        system_path: SystemPath(current_root.system_prefix.as_ref().join(rel_path)),
                        ..current_root.clone()
                    };

                    if new_item.keep() {
                        if new_item.is_file() {
                            return Some(Ok(new_item));
                        } else if new_item.is_dir() {
                            self.root_paths.push(new_item);
                        }
                    }
                }
            }
        }
    }
}
