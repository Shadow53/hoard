use crate::filters::{Filter, Filters};
use crate::hoard::iter::HoardItem;
use crate::hoard::Hoard;
use std::iter::Peekable;
use std::path::PathBuf;
use std::{fs, io};
use crate::paths::{HoardPath, RelativePath};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct RootPathItem {
    hoard_file: HoardItem,
    filters: Filters,
}

impl RootPathItem {
    fn keep(&self) -> bool {
        (self.is_file() || self.is_dir())
            && self.filters.keep(
                self.hoard_file.system_prefix(),
                self.hoard_file.system_path(),
            )
    }

    fn is_file(&self) -> bool {
        self.hoard_file.is_file()
    }

    fn is_dir(&self) -> bool {
        self.hoard_file.is_dir()
    }
}

#[derive(Debug)]
pub(crate) struct AllFilesIter {
    root_paths: Vec<RootPathItem>,
    system_entries: Option<Peekable<fs::ReadDir>>,
    hoard_entries: Option<Peekable<fs::ReadDir>>,
    current_root: Option<RootPathItem>,
}

impl AllFilesIter {
    pub(crate) fn new(
        hoards_root: &HoardPath,
        hoard_name: &str,
        hoard: &Hoard,
    ) -> Result<Self, super::Error> {
        let hoard_name_root = hoards_root.join(
            &RelativePath::try_from(PathBuf::from(hoard_name))
                .expect("hoard name is a valid RelativePath")
        );
        let root_paths = match hoard {
            Hoard::Anonymous(pile) => {
                let path = pile.path.clone();
                let filters = Filters::new(&pile.config)?;
                match path {
                    None => Vec::new(),
                    Some(system_prefix) => {
                        vec![RootPathItem {
                            hoard_file: HoardItem::new(
                                None,
                                hoard_name_root,
                                system_prefix,
                                RelativePath::none(),
                            ),
                            filters,
                        }]
                    }
                }
            }
            Hoard::Named(piles) => piles
                .piles
                .iter()
                .filter_map(|(name, pile)| {
                    let filters = match Filters::new(&pile.config) {
                        Ok(filters) => filters,
                        Err(err) => return Some(Err(err)),
                    };
                    let name_path = RelativePath::try_from(PathBuf::from(name))
                        .expect("pile name should be a valid relative path");
                    pile.path.as_ref().map(|path| {
                        let hoard_prefix = hoard_name_root.join(&name_path);
                        let system_prefix = path.clone();
                        Ok(RootPathItem {
                            hoard_file: HoardItem::new(
                                Some(name.clone()),
                                hoard_prefix,
                                system_prefix,
                                RelativePath::none(),
                            ),
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

    #[allow(clippy::option_option)]
    fn ensure_dir_entries(&mut self) -> Option<Option<io::Result<HoardItem>>> {
        // Attempt to create direntry iterator.
        // If a path to a file is encountered, return that.
        // Otherwise, continue until existing directory is found.
        while !self.has_dir_entries() {
            match self.root_paths.pop() {
                None => return Some(None),
                Some(item) => {
                    if item.keep() {
                        if item.is_file() {
                            return Some(Some(Ok(item.hoard_file)));
                        } else if item.is_dir() {
                            let hoard_path = item.hoard_file.hoard_path();
                            let system_path = item.hoard_file.system_path();
                            match fs::read_dir(system_path) {
                                Ok(iter) => self.system_entries = Some(iter.peekable()),
                                Err(err) => {
                                    if err.kind() == io::ErrorKind::NotFound {
                                        self.system_entries = None;
                                    } else {
                                        tracing::error!(
                                            "failed to read directory {}: {}",
                                            system_path.display(),
                                            err
                                        );
                                        return Some(Some(Err(err)));
                                    }
                                }
                            }
                            match fs::read_dir(hoard_path) {
                                Ok(iter) => self.hoard_entries = Some(iter.peekable()),
                                Err(err) => {
                                    if err.kind() == io::ErrorKind::NotFound {
                                        self.hoard_entries = None;
                                    } else {
                                        tracing::error!(
                                            "failed to read directory {}: {}",
                                            hoard_path.display(),
                                            err
                                        );
                                        return Some(Some(Err(err)));
                                    }
                                }
                            }
                            self.current_root = Some(item);
                        }
                    }
                }
            }
        }

        None
    }
}

impl Iterator for AllFilesIter {
    type Item = io::Result<HoardItem>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(return_value) = self.ensure_dir_entries() {
                return return_value;
            }

            let current_root = self
                .current_root
                .as_ref()
                .expect("current_root should not be None");

            if let Some(system_entries) = self.system_entries.as_mut() {
                for entry in system_entries {
                    let entry = match entry {
                        Ok(entry) => entry,
                        Err(err) => {
                            tracing::error!(
                                "could not process entry in {}: {}",
                                self.current_root
                                    .as_ref()
                                    .unwrap()
                                    .hoard_file
                                    .system_path()
                                    .display(),
                                err
                            );
                            return Some(Err(err));
                        }
                    };

                    let relative_path = RelativePath::try_from(
                        entry
                            .path()
                            .strip_prefix(&current_root.hoard_file.hoard_prefix())
                            .expect("hoard prefix should always match path")
                            .to_path_buf()
                    ).expect("path created with strip_prefix should always be valid RelativePath");

                    let new_item = RootPathItem {
                        hoard_file: HoardItem::new(
                            current_root.hoard_file.pile_name().map(str::to_string),
                            current_root.hoard_file.hoard_prefix().clone(),
                            current_root.hoard_file.system_prefix().clone(),
                            relative_path,
                        ),
                        filters: current_root.filters.clone(),
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
                        Err(err) => {
                            tracing::error!(
                                "could not process entry in {}: {}",
                                self.current_root
                                    .as_ref()
                                    .unwrap()
                                    .hoard_file
                                    .hoard_path()
                                    .display(),
                                err
                            );
                            return Some(Err(err));
                        }
                    };

                    let relative_path = RelativePath::try_from(
                        entry
                            .path()
                            .strip_prefix(&current_root.hoard_file.hoard_prefix())
                            .expect("hoard prefix should always match path")
                            .to_path_buf()
                    ).expect("path created with strip_prefix should always be valid RelativePath");

                    let new_item = RootPathItem {
                        hoard_file: HoardItem::new(
                            current_root.hoard_file.pile_name().map(str::to_string),
                            current_root.hoard_file.hoard_prefix().clone(),
                            current_root.hoard_file.system_prefix().clone(),
                            relative_path,
                        ),
                        filters: current_root.filters.clone(),
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
