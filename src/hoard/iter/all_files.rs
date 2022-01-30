use std::{fs, io};
use std::iter::Peekable;
use std::path::{Path, PathBuf};
use crate::filters::{Filter, Filters};
use crate::hoard::{Direction, Hoard, HoardPath, SystemPath};

pub(crate) struct AllFilesIter {
    root_paths: Vec<(Option<String>, HoardPath, SystemPath, Option<Filters>)>,
    direction: Direction,
    pile_name: Option<String>,
    dir_entries: Option<Peekable<fs::ReadDir>>,
    src_root: Option<PathBuf>,
    dest_root: Option<PathBuf>,
    filter: Option<Filters>,
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
}

impl Iterator for AllFilesIter {
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
