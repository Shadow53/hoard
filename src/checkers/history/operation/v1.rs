//! Keeping track of all operations.
//!
//! The types in this module are used for logging all operations to disk. This information can be
//! used for debugging purposes, but is more directly used as a [`Checker`] to help prevent
//! synchronized changes from being overwritten.
//!
//! It does this by parsing synchronized logs from this and other systems to determine which system
//! was the last one to touch a file.

use crate::checkers::history::operation::{Checksum, OperationFileInfo};
use crate::hoard::Direction;
use crate::paths::RelativePath;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use time::OffsetDateTime;

/// A single operation log.
///
/// This keeps track of the timestamp of the operation (which may include multiple hoards),
/// all hoards involved in the operation (and the related [`HoardOperation`]), and a record
/// of the latest operation log for each external system at the time of invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::module_name_repetitions)]
pub struct OperationV1 {
    /// Timestamp of last operation
    pub timestamp: OffsetDateTime,
    /// Whether this operation was a backup
    pub is_backup: bool,
    /// The name of the hoard for this `HoardOperation`.
    pub hoard_name: String,
    /// Mapping of pile files to checksums
    #[allow(dead_code)]
    pub hoard: Hoard,
}

impl super::OperationImpl for OperationV1 {
    fn direction(&self) -> Direction {
        if self.is_backup {
            Direction::Backup
        } else {
            Direction::Restore
        }
    }

    fn contains_file(
        &self,
        pile_name: Option<&str>,
        rel_path: &RelativePath,
        _only_modified: bool,
    ) -> bool {
        let rel_path = rel_path.to_path_buf();
        match (pile_name, &self.hoard) {
            (None, Hoard::Anonymous(pile)) => pile.0.contains_key(&rel_path),
            (Some(name), Hoard::Named(piles)) => piles
                .get(name)
                .map_or(false, |pile| pile.0.contains_key(&rel_path)),
            _ => false,
        }
    }

    fn timestamp(&self) -> OffsetDateTime {
        self.timestamp
    }

    fn hoard_name(&self) -> &str {
        &self.hoard_name
    }

    fn checksum_for(&self, pile_name: Option<&str>, rel_path: &RelativePath) -> Option<Checksum> {
        let rel_path = rel_path.to_path_buf();
        match (pile_name, &self.hoard) {
            (None, Hoard::Anonymous(pile)) => pile
                .0
                .get(&rel_path)
                .map(|md5| Checksum::MD5(md5.to_string())),
            (Some(name), Hoard::Named(piles)) => piles.get(name).and_then(|pile| {
                pile.0
                    .get(&rel_path)
                    .map(|md5| Checksum::MD5(md5.to_string()))
            }),
            _ => None,
        }
    }

    /// Returns, in order: the pile name, the relative path, and the file's checksum.
    fn all_files_with_checksums<'s>(&'s self) -> Box<dyn Iterator<Item = OperationFileInfo> + 's> {
        match &self.hoard {
            Hoard::Anonymous(pile) => Box::new(pile.0.iter().map(move |(rel_path, md5)| {
                OperationFileInfo {
                    pile_name: None,
                    relative_path: RelativePath::try_from(rel_path.clone())
                        .expect("v1 Operation relative path should always be valid"),
                    checksum: Some(Checksum::MD5(md5.clone())),
                }
            })),
            Hoard::Named(piles) => Box::new({
                piles.iter().flat_map(move |(pile_name, pile)| {
                    pile.0.iter().map(move |(rel_path, md5)| OperationFileInfo {
                        pile_name: Some(pile_name.clone()),
                        relative_path: RelativePath::try_from(rel_path.clone())
                            .expect("v1 Operation relative path should always be valid"),
                        checksum: Some(Checksum::MD5(md5.clone())),
                    })
                })
            }),
        }
    }
}

/// Operation log information for a single hoard.
///
/// Really just a wrapper for [`Pile`] because piles may be anonymous or named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Hoard {
    /// Information for a single, anonymous pile.
    Anonymous(Pile),
    /// Information for some number of named piles.
    Named(HashMap<String, Pile>),
}

/// A mapping of file path (relative to pile) to file checksum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pile(pub(super) HashMap<PathBuf, String>);

impl From<HashMap<PathBuf, String>> for Pile {
    fn from(map: HashMap<PathBuf, String>) -> Self {
        Pile(map)
    }
}
