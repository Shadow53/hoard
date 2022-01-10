use std::fs;
use std::io;
use std::iter::Peekable;
use std::path::{Path, PathBuf};

use super::{Direction, Hoard, HoardPath, SystemPath};

pub(crate) struct HoardFilesIter {
    root_paths: Vec<(HoardPath, SystemPath)>,
    direction: Direction,
    dir_entries: Option<Peekable<fs::ReadDir>>,
    src_root: Option<PathBuf>,
    dest_root: Option<PathBuf>,
}

impl HoardFilesIter {
    pub(crate) fn new(hoards_root: &Path, direction: Direction, hoard_name: &str, hoard: &Hoard) -> Self {
        let root_paths = match hoard {
            Hoard::Anonymous(pile) => {
                let path = pile.path.clone();
                match path {
                    None => Vec::new(),
                    Some(path) => {
                        let hoard_path = HoardPath(hoards_root.join(hoard_name));
                        let system_path = SystemPath(path);
                        vec![(hoard_path, system_path)]
                    }
                }
            },
            Hoard::Named(piles) => {
                piles.piles.iter()
                    .filter_map(|(name, pile)| {
                        pile.path.as_ref().map(|path| {
                            let hoard_path = HoardPath(hoards_root.join(hoard_name).join(name));
                            let system_path = SystemPath(path.clone());
                            (hoard_path, system_path)
                        })
                    })
                    .collect()
            },
        };

        Self { root_paths, direction, dir_entries: None, src_root: None, dest_root: None }
    }
}

impl Iterator for HoardFilesIter {
    type Item = io::Result<(HoardPath, SystemPath)>;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            // Attempt to create direntry iterator.
            // If a path to a file is encountered, return that.
            // Otherwise, continue until existing directory is found.
            while self.dir_entries.is_none() || self.dir_entries.as_mut().unwrap().peek().is_none() {
                match self.root_paths.pop() {
                    None => return None,
                    Some((hoard_path, system_path)) => {
                        let (src, dest) = match self.direction {
                            Direction::Backup => (&system_path.0, &hoard_path.0),
                            Direction::Restore => (&hoard_path.0, &system_path.0),
                        };

                        if src.is_file() {
                            return Some(Ok((hoard_path, system_path)));
                        } else if src.is_dir() {
                            self.src_root = Some(src.to_path_buf());
                            self.dest_root = Some(dest.to_path_buf());
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
                let dest = self.dest_root.as_ref().expect("dest_root should not be None").join(entry.file_name());
                let is_file = src.is_file();
                let is_dir = src.is_dir();

                let (hoard_path, system_path) = match self.direction {
                    Direction::Backup => (HoardPath(dest), SystemPath(src)),
                    Direction::Restore => (HoardPath(src), SystemPath(dest)),
                };

                if is_file {
                    return Some(Ok((hoard_path, system_path)));
                } else if is_dir {
                    self.root_paths.push((hoard_path, system_path));
                }
            }
        }
    }
}
