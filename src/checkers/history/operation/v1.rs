//! Keeping track of all operations.
//!
//! The types in this module are used for logging all operations to disk. This information can be
//! used for debugging purposes, but is more directly used as a [`Checker`] to help prevent
//! synchronized changes from being overwritten.
//!
//! It does this by parsing synchronized logs from this and other systems to determine which system
//! was the last one to touch a file.

use crate::checkers::history::operation::OperationFileInfo;
use crate::checksum::Checksum;
use crate::hoard::Direction;
use crate::newtypes::{HoardName, NonEmptyPileName, PileName};
use crate::paths::RelativePath;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    pub hoard_name: HoardName,
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
        pile_name: &PileName,
        rel_path: &RelativePath,
        _only_modified: bool,
    ) -> bool {
        match (pile_name.as_ref(), &self.hoard) {
            (None, Hoard::Anonymous(pile)) => pile.0.contains_key(rel_path),
            (Some(pile_name), Hoard::Named(piles)) => piles
                .get(pile_name)
                .map_or(false, |pile| pile.0.contains_key(rel_path)),
            _ => false,
        }
    }

    fn timestamp(&self) -> OffsetDateTime {
        self.timestamp
    }

    fn hoard_name(&self) -> &HoardName {
        &self.hoard_name
    }

    fn checksum_for(&self, pile_name: &PileName, rel_path: &RelativePath) -> Option<Checksum> {
        match (pile_name.as_ref(), &self.hoard) {
            (None, Hoard::Anonymous(pile)) => pile.0.get(rel_path).cloned(),
            (Some(pile_name), Hoard::Named(piles)) => piles
                .get(pile_name)
                .and_then(|pile| pile.0.get(rel_path).cloned()),
            _ => None,
        }
    }

    /// Returns, in order: the pile name, the relative path, and the file's checksum.
    fn all_files_with_checksums<'s>(&'s self) -> Box<dyn Iterator<Item = OperationFileInfo> + 's> {
        match &self.hoard {
            Hoard::Anonymous(pile) => {
                Box::new(pile.0.iter().map(move |(rel_path, md5)| OperationFileInfo {
                    pile_name: PileName::anonymous(),
                    relative_path: rel_path.clone(),
                    checksum: Some(md5.clone()),
                }))
            }
            Hoard::Named(piles) => Box::new({
                piles.iter().flat_map(move |(pile_name, pile)| {
                    pile.0.iter().map(move |(rel_path, md5)| OperationFileInfo {
                        pile_name: pile_name.clone().into(),
                        relative_path: rel_path.clone(),
                        checksum: Some(md5.clone()),
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
    Named(HashMap<NonEmptyPileName, Pile>),
}

/// A mapping of file path (relative to pile) to file checksum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Pile(pub(super) HashMap<RelativePath, Checksum>);

impl From<HashMap<RelativePath, Checksum>> for Pile {
    fn from(map: HashMap<RelativePath, Checksum>) -> Self {
        Pile(map)
    }
}
