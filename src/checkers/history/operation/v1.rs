//! The first operation log format, retained for backwards compatibility.

use crate::checkers::history::operation::OperationFileInfo;
use crate::checksum::{Checksum, MD5};
use crate::hoard::Direction;
use crate::newtypes::{HoardName, NonEmptyPileName, PileName};
use crate::paths::RelativePath;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use time::OffsetDateTime;

/// A single operation log.
///
/// This keeps track of the timestamp of the operation (which may include multiple hoards),
/// all hoards involved in the operation (and the related [`Hoard` operation](Hoard)), and a record
/// of the latest operation log for each external system at the time of invocation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
            (None, Hoard::Anonymous(pile)) => pile.0.get(rel_path).cloned().map(Checksum::MD5),
            (Some(pile_name), Hoard::Named(piles)) => piles
                .get(pile_name)
                .and_then(|pile| pile.0.get(rel_path).cloned().map(Checksum::MD5)),
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
                    checksum: Some(Checksum::MD5((*md5).clone())),
                }))
            }
            Hoard::Named(piles) => Box::new({
                piles.iter().flat_map(move |(pile_name, pile)| {
                    pile.0.iter().map(move |(rel_path, md5)| OperationFileInfo {
                        pile_name: pile_name.clone().into(),
                        relative_path: rel_path.clone(),
                        checksum: Some(Checksum::MD5((*md5).clone())),
                    })
                })
            }),
        }
    }
}

/// Operation log information for a single hoard.
///
/// Really just a wrapper for [`Pile`] because piles may be anonymous or named.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Hoard {
    /// Information for a single, anonymous pile.
    Anonymous(Pile),
    /// Information for some number of named piles.
    Named(HashMap<NonEmptyPileName, Pile>),
}

/// A mapping of file path (relative to pile) to file checksum.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pile(pub(super) HashMap<RelativePath, MD5>);

impl From<HashMap<RelativePath, MD5>> for Pile {
    fn from(map: HashMap<RelativePath, MD5>) -> Self {
        Pile(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{checkers::history::operation::OperationImpl, checksum::MD5, test::relative_path};

    fn checksum() -> MD5 {
        MD5::from_data([0xFD, 0xFF, 0xFE])
    }

    fn anon_op() -> OperationV1 {
        OperationV1 {
            timestamp: OffsetDateTime::now_utc(),
            is_backup: true,
            hoard_name: "hoard_name".parse().unwrap(),
            hoard: Hoard::Anonymous(Pile(maplit::hashmap! {
                relative_path!("test/path") => checksum()
            })),
        }
    }

    fn named_op() -> OperationV1 {
        OperationV1 {
            timestamp: OffsetDateTime::now_utc(),
            is_backup: false,
            hoard_name: "hoard_name".parse().unwrap(),
            hoard: Hoard::Named(maplit::hashmap! {
                "first".parse().unwrap() => Pile(maplit::hashmap! {
                    relative_path!("test/path") => checksum()
                }),
                "second".parse().unwrap() => Pile(maplit::hashmap! {
                    relative_path!("other/path") => checksum()
                })
            }),
        }
    }

    #[test]
    fn test_direction() {
        let anon_op = anon_op();
        assert_eq!(anon_op.direction(), Direction::Backup);
        let named_op = named_op();
        assert_eq!(named_op.direction(), Direction::Restore);
    }

    #[test]
    fn test_contains_file() {
        let anon_op = anon_op();
        assert!(anon_op.contains_file(&PileName::anonymous(), &relative_path!("test/path"), false,));
        assert!(anon_op.contains_file(&PileName::anonymous(), &relative_path!("test/path"), true,));
        assert!(!anon_op.contains_file(
            &PileName::anonymous(),
            &relative_path!("test/missing"),
            false,
        ));
        assert!(!anon_op.contains_file(
            &PileName::anonymous(),
            &relative_path!("test/missing"),
            true,
        ));
        assert!(!anon_op.contains_file(
            &"first".parse().unwrap(),
            &relative_path!("test/path"),
            false,
        ));
        assert!(!anon_op.contains_file(
            &"first".parse().unwrap(),
            &relative_path!("test/path"),
            true,
        ));

        let named_op = named_op();
        assert!(named_op.contains_file(
            &"first".parse().unwrap(),
            &relative_path!("test/path"),
            false,
        ));
        assert!(named_op.contains_file(
            &"first".parse().unwrap(),
            &relative_path!("test/path"),
            true,
        ));
        assert!(!named_op.contains_file(
            &"first".parse().unwrap(),
            &relative_path!("other/path"),
            false,
        ));
        assert!(!named_op.contains_file(
            &"first".parse().unwrap(),
            &relative_path!("other/path"),
            true,
        ));
        assert!(named_op.contains_file(
            &"second".parse().unwrap(),
            &relative_path!("other/path"),
            false,
        ));
        assert!(named_op.contains_file(
            &"second".parse().unwrap(),
            &relative_path!("other/path"),
            true,
        ));
        assert!(!named_op.contains_file(
            &"second".parse().unwrap(),
            &relative_path!("test/path"),
            false,
        ));
        assert!(!named_op.contains_file(
            &"second".parse().unwrap(),
            &relative_path!("test/path"),
            true,
        ));
        assert!(!named_op.contains_file(
            &PileName::anonymous(),
            &relative_path!("test/path"),
            false,
        ));
        assert!(!named_op.contains_file(
            &PileName::anonymous(),
            &relative_path!("test/path"),
            true,
        ));
    }

    #[test]
    fn test_hoard_name() {
        let anon_op = anon_op();
        let named_op = named_op();
        assert_eq!(anon_op.hoard_name(), named_op.hoard_name());
        assert_eq!(
            anon_op.hoard_name(),
            &"hoard_name".parse::<HoardName>().unwrap()
        );
    }

    #[test]
    fn test_checksum_for() {
        let anon_op = anon_op();
        let named_op = named_op();

        assert_eq!(
            Some(Checksum::MD5(checksum())),
            anon_op.checksum_for(&PileName::anonymous(), &relative_path!("test/path"))
        );
        assert_eq!(
            None,
            anon_op.checksum_for(&PileName::anonymous(), &relative_path!("other/path"))
        );
        assert_eq!(
            None,
            anon_op.checksum_for(&"first".parse().unwrap(), &relative_path!("test/path"))
        );

        assert_eq!(
            Some(Checksum::MD5(checksum())),
            named_op.checksum_for(&"first".parse().unwrap(), &relative_path!("test/path"),)
        );
        assert_eq!(
            Some(Checksum::MD5(checksum())),
            named_op.checksum_for(&"second".parse().unwrap(), &relative_path!("other/path"),)
        );
        assert_eq!(
            None,
            named_op.checksum_for(&"first".parse().unwrap(), &relative_path!("other/path"),)
        );
        assert_eq!(
            None,
            named_op.checksum_for(&"second".parse().unwrap(), &relative_path!("test/path"),)
        );
        assert_eq!(
            None,
            named_op.checksum_for(&PileName::anonymous(), &relative_path!("test/path"),)
        );
    }
}
