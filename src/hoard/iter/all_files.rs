use crate::filters::{Filter, Filters};
use crate::hoard::{Hoard, HoardPath, SystemPath};
use std::iter::Peekable;
use std::path::Path;
use std::{fs, io};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct HoardFile {
    pub(crate) pile_name: Option<String>,
    pub(crate) hoard_path: HoardPath,
    pub(crate) system_path: SystemPath,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct RootPathItem {
    hoard_file: HoardFile,
    hoard_prefix: HoardPath,
    system_prefix: SystemPath,
    filters: Option<Filters>,
}

impl HoardFile {
    fn is_file(&self) -> bool {
        let sys = self.system_path.as_ref();
        let hoard = self.hoard_path.as_ref();
        let sys_exists = sys.exists();
        let hoard_exists = hoard.exists();
        (sys.is_file() || !sys_exists)
            && (hoard.is_file() || !hoard_exists)
            && (sys_exists || hoard_exists)
    }

    fn is_dir(&self) -> bool {
        let sys = self.system_path.as_ref();
        let hoard = self.hoard_path.as_ref();
        let sys_exists = sys.exists();
        let hoard_exists = hoard.exists();
        (sys.is_dir() || !sys_exists)
            && (hoard.is_dir() || !hoard_exists)
            && (sys_exists || hoard_exists)
    }
}

impl RootPathItem {
    fn keep(&self) -> bool {
        (self.is_file() || self.is_dir())
            && self.filters.as_ref().map_or(true, |filters| {
                filters.keep(self.system_prefix.as_ref(), self.hoard_file.system_path.as_ref())
            })
    }

    fn is_file(&self) -> bool {
        self.hoard_file.is_file()
    }

    fn is_dir(&self) -> bool {
        self.hoard_file.is_dir()
    }
}

pub(crate) struct AllFilesIter {
    root_paths: Vec<RootPathItem>,
    system_entries: Option<Peekable<fs::ReadDir>>,
    hoard_entries: Option<Peekable<fs::ReadDir>>,
    current_root: Option<RootPathItem>,
}

impl AllFilesIter {
    pub(crate) fn new(
        hoards_root: &Path,
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
                        vec![RootPathItem {
                            hoard_prefix: hoard_path.clone(),
                            system_prefix: system_path.clone(),
                            hoard_file: HoardFile {
                                pile_name: None,
                                hoard_path,
                                system_path,
                            },
                            filters,
                        }]
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
                        Ok(RootPathItem {
                            hoard_prefix: hoard_path.clone(),
                            system_prefix: system_path.clone(),
                            hoard_file: HoardFile {
                                pile_name: Some(name.clone()),
                                hoard_path,
                                system_path,
                            },
                            filters,
                        })
                    })
                })
                .collect::<Result<_, _>>()?,
        };

        Ok(Self {
            root_paths,
            system_entries: None,
            hoard_entries: None,
            current_root: None,
        })
    }
}

impl AllFilesIter {
    fn has_dir_entries(&mut self) -> bool {
        if let Some(system_entries) = self.system_entries.as_mut() {
            if system_entries.peek().is_some() {
                return true;
            }
        }

        if let Some(hoard_entries) = self.hoard_entries.as_mut() {
            if hoard_entries.peek().is_some() {
                return true;
            }
        }

        false
    }
}

impl Iterator for AllFilesIter {
    type Item = io::Result<HoardFile>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Attempt to create direntry iterator.
            // If a path to a file is encountered, return that.
            // Otherwise, continue until existing directory is found.
            while !self.has_dir_entries() {
                match self.root_paths.pop() {
                    None => return None,
                    Some(item) => {
                        if item.keep() {
                            if item.is_file() {
                                return Some(Ok(item.hoard_file));
                            } else if item.is_dir() {
                                match fs::read_dir(item.hoard_file.system_path.as_ref()) {
                                    Ok(iter) => self.system_entries = Some(iter.peekable()),
                                    Err(err) => match err.kind() {
                                        io::ErrorKind::NotFound => self.system_entries = None,
                                        _ => return Some(Err(err)),
                                    },
                                }
                                match fs::read_dir(item.hoard_file.hoard_path.as_ref()) {
                                    Ok(iter) => self.hoard_entries = Some(iter.peekable()),
                                    Err(err) => match err.kind() {
                                        io::ErrorKind::NotFound => self.hoard_entries = None,
                                        _ => return Some(Err(err)),
                                    },
                                }
                                self.current_root = Some(item);
                            }
                        }
                    }
                }
            }

            let current_root = self
                .current_root
                .as_ref()
                .expect("current_root should not be None");

            if let Some(system_entries) = self.system_entries.as_mut() {
                for entry in system_entries {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(err) => return Some(Err(err)),
                    };

                    let rel_path = entry
                        .path()
                        .strip_prefix(current_root.system_prefix.as_ref())
                        .expect("system prefix should always match path")
                        .to_path_buf();

                    let new_item = RootPathItem {
                        hoard_file: HoardFile {
                            hoard_path: HoardPath(current_root.hoard_prefix.as_ref().join(&rel_path)),
                            system_path: SystemPath(current_root.system_prefix.as_ref().join(rel_path)),
                            pile_name: current_root.hoard_file.pile_name.clone(),
                        },
                        ..current_root.clone()
                    };

                    if new_item.keep() {
                        if new_item.is_file() {
                            return Some(Ok(new_item.hoard_file));
                        } else if new_item.is_dir() {
                            self.root_paths.push(new_item);
                        }
                    }
                }
            }

            if let Some(hoard_entries) = self.hoard_entries.as_mut() {
                for entry in hoard_entries {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(err) => return Some(Err(err)),
                    };

                    let rel_path = entry
                        .path()
                        .strip_prefix(current_root.hoard_prefix.as_ref())
                        .expect("hoard prefix should always match path")
                        .to_path_buf();

                    let new_item = RootPathItem {
                        hoard_file: HoardFile {
                            hoard_path: HoardPath(current_root.hoard_prefix.as_ref().join(&rel_path)),
                            system_path: SystemPath(current_root.system_prefix.as_ref().join(rel_path)),
                            pile_name: current_root.hoard_file.pile_name.clone(),
                        },
                        ..current_root.clone()
                    };

                    if new_item.keep() {
                        if new_item.is_file() {
                            return Some(Ok(new_item.hoard_file));
                        } else if new_item.is_dir() {
                            self.root_paths.push(new_item);
                        }
                    }
                }
            }
        }
    }
}
