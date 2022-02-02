//! Keeping track of all operations.
//!
//! The types in this module are used for logging all operations to disk. This information can be
//! used for debugging purposes, but is more directly used as a [`Checker`] to help prevent
//! synchronized changes from being overwritten.
//!
//! It does this by parsing synchronized logs from this and other systems to determine which system
//! was the last one to touch a file.

use crate::hoard::{Direction, Hoard as ConfigHoard};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::io;
use time::OffsetDateTime;
use crate::hoard::iter::{OperationIter, OperationType};
use crate::hoard_file::Checksum;

use super::Error;

/// Errors that may occur while working with operation logs.

/// A single operation log.
///
/// This keeps track of the timestamp of the operation (which may include multiple hoards),
/// all hoards involved in the operation (and the related [`HoardOperation`]), and a record
/// of the latest operation log for each external system at the time of invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::module_name_repetitions)]
pub(crate) struct OperationV2 {
    /// Timestamp of last operation
    timestamp: OffsetDateTime,
    /// Which direction this operation went
    direction: Direction,
    /// The name of the hoard for this `HoardOperation`.
    hoard: String,
    /// Mapping of pile files to checksums
    files: Hoard,
    // TODO: include unchanged as well
    #[serde(skip, default)]
    hoards_root: PathBuf,
}

impl OperationV2 {
    pub(super) fn new(hoards_root: &Path, name: &str, hoard: &ConfigHoard, direction: Direction) -> Result<Self, Error> {
        Ok(Self {
            timestamp: OffsetDateTime::now_utc(),
            direction,
            hoard: name.into(),
            files: Hoard::new(hoards_root, name, hoard, direction)?,
            hoards_root: hoards_root.to_path_buf(),
        })
    }

    /// Convert a v1 Operation to a v2 Operation.
    ///
    /// Requires all of the operations prior to `old_v1` to have been processed such that the
    /// following is true:
    ///
    /// - `file_checksums`: contains all paths that were ever part of the hoard with their recorded
    ///   checksums. The checksum is `None` if the file did not exist prior to the `old_v1` operation.
    /// - `files`: contains all paths whose checksums are `None` in `file_checksums`. This is used
    ///   as an optimization technique while determining which files were created or deleted.
    pub(crate) fn from_v1(
        file_checksums: &mut HashMap<(String, PathBuf), Option<Checksum>>,
        file_set: &mut HashSet<(String, PathBuf)>,
        old_v1: super::v1::OperationV1,
    ) -> Self {
        let mut these_files = HashSet::new();
        let mut files = HashMap::new();

        for (pile_name, path, md5) in old_v1.all_files_with_checksums() {
            let pile = {
                if files.contains_key(pile_name) {
                    files.insert(pile_name.to_string(), Pile::default());
                }
                files.get_mut(pile_name).unwrap()
            };

            let pile_file = (pile_name.to_string(), path.to_path_buf());
            let checksum = Checksum::MD5(md5.to_string());
            if file_checksums.contains_key(&pile_file) {
                // Modified, Recreated, or Unchanged
                match file_checksums.get(&pile_file).unwrap() {
                    None => {
                        // Recreated
                        pile.created.insert(path.to_path_buf(), checksum.clone());
                    },
                    Some(old_checksum) => {
                        // Modified or Unchanged
                        if old_checksum == &checksum {
                            pile.unmodified.insert(path.to_path_buf(), checksum.clone());
                        } else {
                            pile.modified.insert(path.to_path_buf(), checksum.clone());
                        }
                    }
                }
            } else {
                // Created
                pile.created.insert(path.to_path_buf(), checksum.clone());
            }
            file_checksums.insert(pile_file.clone(), Some(checksum));
            these_files.insert(pile_file);
        }

        let deleted: HashMap<String, HashSet<PathBuf>> = file_set.difference(&these_files)
            .fold(HashMap::new(), |mut acc, (pile_name, rel_path)| {
                if !acc.contains_key(pile_name) {
                    acc.insert(pile_name.to_string(), HashSet::new());
                }
                acc.get_mut(pile_name).unwrap().insert(rel_path.to_path_buf());
                acc
            });

        for (pile_name, deleted) in deleted {
            if !files.contains_key(&pile_name) {
                files.insert(pile_name.clone(), Pile::default());
            }
            files.get_mut(&pile_name).unwrap().deleted = deleted;
        }

        let files = if files.len() == 1 && files.contains_key("") {
            Hoard::Anonymous(files.remove("").unwrap())
        } else {
            Hoard::Named(files)
        };

        Self {
            timestamp: old_v1.timestamp,
            direction: if old_v1.is_backup { Direction::Backup } else { Direction::Restore },
            hoard: old_v1.hoard_name,
            files,
            hoards_root: Default::default()
        }
    }
}

impl super::OperationImpl for OperationV2 {
    fn is_backup(&self) -> bool {
        self.direction == Direction::Backup
    }

    fn contains_file(&self, pile_name: Option<&str>, rel_path: &Path) -> bool {
        self.files.get_pile(pile_name).map_or(false, |pile| pile.contains_file(rel_path))
    }

    fn timestamp(&self) -> OffsetDateTime {
        self.timestamp.clone()
    }

    fn hoard_name(&self) -> &str {
        &self.hoard
    }

    fn checksum_for(&self, pile_name: Option<&str>, rel_path: &Path) -> Option<Checksum> {
        self.files.get_pile(pile_name).and_then(|pile| pile.checksum_for(rel_path))
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[serde(untagged)]
#[allow(variant_size_differences)]
enum Hoard {
    Anonymous(Pile),
    Named(HashMap<String, Pile>),
}

impl Hoard {
    fn get_or_create_pile<'a>(map: &'a mut HashMap<String, Pile>, pile_name: Option<&str>) -> &'a mut Pile {
        let pile_name = pile_name.unwrap_or("");
        if !map.contains_key(pile_name) {
            map.insert(pile_name.to_string(), Pile::default());
        }
        map.get_mut(pile_name).unwrap()
    }

    fn require_checksum(checksum: Option<Checksum>, path: &Path) -> Result<Checksum, Error> {
        checksum.ok_or_else(|| Error::IO(io::Error::new(
            io::ErrorKind::NotFound,
            format!("could not find {}", path.display())
        )))
    }

    fn new(hoards_root: &Path, hoard_name: &str, hoard: &crate::hoard::Hoard, direction: Direction) -> Result<Self, Error> {
        let mut inner: HashMap<String, Pile> = OperationIter::new(hoards_root, hoard_name.to_string(), hoard, direction)?
            .fold(Ok(HashMap::new()), |acc, op| -> Result<HashMap<String, Pile>, Error> {
                let mut acc = acc?;
                let op = op?;

                match op {
                    OperationType::Create(file) => {
                        let checksum = match direction {
                            Direction::Backup => Self::require_checksum(file.system_checksum()?, file.system_path())?,
                            Direction::Restore => Self::require_checksum(file.hoard_checksum()?, file.hoard_path())?,
                        };
                        Self::get_or_create_pile(&mut acc, file.pile_name())
                            .created
                            .insert(file.relative_path().to_path_buf(), checksum);
                    }
                    OperationType::Modify(file) => {
                        let checksum = match direction {
                            Direction::Backup => Self::require_checksum(file.system_checksum()?, file.system_path())?,
                            Direction::Restore => Self::require_checksum(file.hoard_checksum()?, file.hoard_path())?,
                        };
                        Self::get_or_create_pile(&mut acc, file.pile_name())
                            .modified
                            .insert(file.relative_path().to_path_buf(), checksum);
                    }
                    OperationType::Delete(file) => {
                        Self::get_or_create_pile(&mut acc, file.pile_name()).deleted.insert(file.relative_path().to_path_buf());
                    }
                    OperationType::Nothing(file) => {
                        let checksum = Self::require_checksum(file.system_checksum()?, file.system_path())?;
                        Self::get_or_create_pile(&mut acc, file.pile_name())
                            .modified
                            .insert(file.relative_path().to_path_buf(), checksum);
                    }
                }

                Ok(acc)
            })?;

        if inner.len() == 1 && inner.contains_key("") {
            Ok(Self::Anonymous(inner.remove("").unwrap()))
        } else {
            Ok(Self::Named(inner))
        }
    }

    fn get_pile(&self, name: Option<&str>) -> Option<&Pile> {
        match (name, self) {
            (None, Hoard::Anonymous(pile)) => Some(pile),
            (Some(name), Hoard::Named(piles)) => piles.get(name),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Default)]
struct Pile {
    created: HashMap<PathBuf, Checksum>,
    modified: HashMap<PathBuf, Checksum>,
    deleted: HashSet<PathBuf>,
    unmodified: HashMap<PathBuf, Checksum>,
}

impl Pile {
    fn contains_file(&self, rel_path: &Path) -> bool {
        self.created.contains_key(rel_path) ||
            self.modified.contains_key(rel_path) ||
            self.deleted.contains(rel_path) ||
            self.unmodified.contains_key(rel_path)
    }

    fn checksum_for(&self, rel_path: &Path) -> Option<Checksum> {
        self.created.get(rel_path)
            .or_else(|| self.modified.get(rel_path))
            .or_else(|| self.unmodified.get(rel_path))
            .map(Clone::clone)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_test::{assert_tokens, Token};

    #[test]
    fn test_checksum_derives() {
        let checksum = Checksum::MD5("legit checksum".to_string());
        assert!(format!("{:?}", checksum).contains("MD5"));
        assert_eq!(checksum, checksum.clone());
        assert_tokens(
            &checksum,
            &[
                Token::Enum { name: "Checksum" },
                Token::Str("md5"),
                Token::Str("legit checksum"),
            ],
        );
    }
}

