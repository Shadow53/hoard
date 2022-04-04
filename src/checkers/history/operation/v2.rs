//! Keeping track of all operations.
//!
//! The types in this module are used for logging all operations to disk. This information can be
//! used for debugging purposes, but is more directly used as a [`Checker`] to help prevent
//! synchronized changes from being overwritten.
//!
//! It does this by parsing synchronized logs from this and other systems to determine which system
//! was the last one to touch a file.

use super::Error;
use crate::checkers::history::operation::{OperationFileInfo, OperationImpl};
use crate::checksum::{Checksum, ChecksumType};
use crate::hoard::iter::{OperationIter, ItemOperation};
use crate::hoard::{Direction, Hoard as ConfigHoard};
use crate::hoard_item::HoardItem;
use crate::newtypes::{HoardName, NonEmptyPileName, PileName};
use crate::paths::{HoardPath, RelativePath};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::io;
use time::OffsetDateTime;

/// Errors that may occur while working with operation logs.

/// A single operation log.
///
/// This keeps track of the timestamp of the operation (which may include multiple hoards),
/// all hoards involved in the operation (and the related [`HoardOperation`]), and a record
/// of the latest operation log for each external system at the time of invocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(clippy::module_name_repetitions)]
pub struct OperationV2 {
    /// Timestamp of last operation
    timestamp: OffsetDateTime,
    /// Which direction this operation went
    direction: Direction,
    /// The name of the hoard for this `HoardOperation`.
    hoard: HoardName,
    /// Mapping of pile files to checksums
    files: Hoard,
}

impl OperationV2 {
    pub(super) fn new(
        hoards_root: &HoardPath,
        name: &HoardName,
        hoard: &ConfigHoard,
        direction: Direction,
    ) -> Result<Self, Error> {
        Ok(Self {
            timestamp: OffsetDateTime::now_utc(),
            direction,
            hoard: name.clone(),
            files: Hoard::new(hoards_root, name, hoard, direction)?,
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
    #[allow(clippy::needless_pass_by_value)]
    pub fn from_v1(
        file_checksums: &mut HashMap<(PileName, RelativePath), Option<Checksum>>,
        file_set: &mut HashSet<(PileName, RelativePath)>,
        old_v1: super::v1::OperationV1,
    ) -> Self {
        let mut these_files = HashSet::new();
        let mut files = HashMap::new();
        let is_anonymous = matches!(old_v1.hoard, super::v1::Hoard::Anonymous(_));

        for file_info in old_v1.all_files_with_checksums() {
            let OperationFileInfo {
                pile_name,
                relative_path,
                checksum,
                ..
            } = file_info;
            let pile = files.entry(pile_name.clone()).or_insert_with(Pile::new);

            let pile_file = (pile_name, relative_path.clone());
            let checksum = checksum.expect("v1 Operation only stored files with checksums");
            match file_checksums.get(&pile_file) {
                None | Some(None) => {
                    // Created or recreated
                    pile.created.insert(relative_path, checksum.clone());
                }
                Some(Some(old_checksum)) => {
                    // Modified or Unchanged
                    if old_checksum == &checksum {
                        pile.unmodified.insert(relative_path, checksum.clone());
                    } else {
                        pile.modified.insert(relative_path, checksum.clone());
                    }
                }
            }
            file_checksums.insert(pile_file.clone(), Some(checksum));
            these_files.insert(pile_file);
        }

        let deleted: HashMap<PileName, HashSet<RelativePath>> = file_set
            .difference(&these_files)
            .fold(HashMap::new(), |mut acc, (pile_name, rel_path)| {
                acc.entry(pile_name.clone())
                    .or_insert_with(HashSet::new)
                    .insert(rel_path.clone());
                acc
            });

        *file_set = these_files;

        for (pile_name, deleted) in deleted {
            files.entry(pile_name).or_insert_with(Pile::new).deleted = deleted;
        }

        let files = if is_anonymous {
            Hoard::Anonymous(files.remove(&PileName::anonymous()).unwrap_or_else(|| {
                let mut pile = Pile::new();
                pile.add_deleted(RelativePath::none());
                pile
            }))
        } else {
            Hoard::Named(
                files
                    .into_iter()
                    .map(|(key, val)| {
                        key.try_into()
                            .map(|key| (key, val))
                            .expect("log was verified to not be anonymous")
                    })
                    .collect(),
            )
        };

        Self {
            timestamp: old_v1.timestamp(),
            direction: old_v1.direction(),
            hoard: old_v1.hoard_name().clone(),
            files,
        }
    }
}

impl OperationImpl for OperationV2 {
    fn direction(&self) -> Direction {
        self.direction
    }

    fn contains_file(
        &self,
        pile_name: &PileName,
        rel_path: &RelativePath,
        only_modified: bool,
    ) -> bool {
        self.files
            .get_pile(pile_name)
            .map_or(false, |pile| pile.contains_file(rel_path, only_modified))
    }

    fn timestamp(&self) -> OffsetDateTime {
        self.timestamp
    }

    fn hoard_name(&self) -> &HoardName {
        &self.hoard
    }

    fn checksum_for(&self, pile_name: &PileName, rel_path: &RelativePath) -> Option<Checksum> {
        self.files
            .get_pile(pile_name)
            .and_then(|pile| pile.checksum_for(rel_path))
    }

    fn all_files_with_checksums<'a>(&'a self) -> Box<dyn Iterator<Item = OperationFileInfo> + 'a> {
        match &self.files {
            Hoard::Anonymous(pile) => Box::new(pile.all_files_with_checksums().map(
                move |(path, checksum)| OperationFileInfo {
                    pile_name: PileName::anonymous(),
                    relative_path: path.clone(),
                    checksum,
                },
            )),
            Hoard::Named(piles) => Box::new(piles.iter().flat_map(move |(pile_name, pile)| {
                pile.all_files_with_checksums()
                    .map(move |(path, checksum)| OperationFileInfo {
                        pile_name: pile_name.clone().into(),
                        relative_path: path.clone(),
                        checksum,
                    })
            })),
        }
    }

    fn hoard_operations_iter<'a>(&'a self, hoard_root: &HoardPath, hoard: &crate::hoard::Hoard) -> Result<Box<dyn Iterator<Item = ItemOperation> + 'a>, Error> {
        let iter = hoard.get_paths(hoard_root.clone())
            .filter_map(|(pile_name, hoard_path, system_path)| {
                println!("pile_name: \"{}\", files: {:?}", pile_name, self.files);
                let pile = self.files.get_pile(&pile_name)?;

                let (c_pile_name, c_hoard_path, c_system_path) = (pile_name.clone(), hoard_path.clone(), system_path.clone());
                let created = pile.created.keys().cloned().map(move |rel_path| {
                    ItemOperation::Create(
                        HoardItem::new(
                            // Clone here because the values may be used by the closure being
                            // called multiple times.
                            c_pile_name.clone(),
                            c_hoard_path.clone(),
                            c_system_path.clone(),
                            rel_path
                        )
                    )
                });

                let (m_pile_name, m_hoard_path, m_system_path) = (pile_name.clone(), hoard_path.clone(), system_path.clone());
                let modified = pile.modified.keys().cloned().map(move |rel_path| {
                    ItemOperation::Modify(
                        HoardItem::new(
                            m_pile_name.clone(),
                            m_hoard_path.clone(),
                            m_system_path.clone(),
                            rel_path
                        )
                    )
                });

                let (d_pile_name, d_hoard_path, d_system_path) = (pile_name, hoard_path, system_path);
                let deleted = pile.deleted.iter().cloned().map(move |rel_path| {
                    ItemOperation::Delete(
                        HoardItem::new(
                            d_pile_name.clone(),
                            d_hoard_path.clone(),
                            d_system_path.clone(),
                            rel_path
                        )
                    )
                });

                Some(created.chain(modified).chain(deleted))
            }).flatten();
        Ok(Box::new(iter))
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
#[allow(variant_size_differences)]
enum Hoard {
    Anonymous(Pile),
    Named(HashMap<NonEmptyPileName, Pile>),
}

impl Hoard {
    fn get_or_create_pile<'a>(
        map: &'a mut HashMap<PileName, Pile>,
        pile_name: &PileName,
    ) -> &'a mut Pile {
        if !map.contains_key(pile_name) {
            map.insert(pile_name.clone(), Pile::default());
        }
        map.get_mut(pile_name).unwrap()
    }

    fn require_checksum(
        checksum: Option<Checksum>,
        path: &RelativePath,
    ) -> Result<Checksum, Error> {
        checksum.ok_or_else(|| {
            Error::IO(io::Error::new(
                io::ErrorKind::NotFound,
                format!("could not find {}", path),
            ))
        })
    }

    fn checksum_type(hoard: &ConfigHoard, hoard_file: &HoardItem) -> ChecksumType {
        match (hoard, hoard_file.pile_name().as_ref()) {
            (ConfigHoard::Anonymous(pile), None) => pile.config.checksum_type,
            (ConfigHoard::Named(piles), Some(name)) => piles
                .piles
                .get(name)
                .map(|pile| pile.config.checksum_type)
                .expect("provided pile name should always be in hoard"),
            (hoard, pile_name) => panic!(
                "mismatched hoard type and pile name option: hoard ({:?}), pile_name: {:?}",
                hoard, pile_name
            ),
        }
    }

    fn new(
        hoards_root: &HoardPath,
        hoard_name: &HoardName,
        hoard: &crate::hoard::Hoard,
        direction: Direction,
    ) -> Result<Self, Error> {
        let mut inner: HashMap<PileName, Pile> =
            OperationIter::new(hoards_root, hoard_name.clone(), hoard, direction)?.fold(
                Ok(HashMap::new()),
                |acc, op| -> Result<HashMap<PileName, Pile>, Error> {
                    let mut acc = acc?;
                    let op = op?;

                    match op {
                        ItemOperation::Create(file) => {
                            let checksum = match direction {
                                Direction::Backup => Self::require_checksum(
                                    file.system_checksum(Self::checksum_type(hoard, &file))?,
                                    file.relative_path(),
                                )?,
                                Direction::Restore => Self::require_checksum(
                                    file.hoard_checksum(Self::checksum_type(hoard, &file))?,
                                    file.relative_path(),
                                )?,
                            };
                            Self::get_or_create_pile(&mut acc, file.pile_name())
                                .add_created(file.relative_path().clone(), checksum);
                        }
                        ItemOperation::Modify(file) => {
                            let checksum = match direction {
                                Direction::Backup => Self::require_checksum(
                                    file.system_checksum(Self::checksum_type(hoard, &file))?,
                                    file.relative_path(),
                                )?,
                                Direction::Restore => Self::require_checksum(
                                    file.hoard_checksum(Self::checksum_type(hoard, &file))?,
                                    file.relative_path(),
                                )?,
                            };
                            Self::get_or_create_pile(&mut acc, file.pile_name())
                                .add_modified(file.relative_path().clone(), checksum);
                        }
                        ItemOperation::Delete(file) => {
                            Self::get_or_create_pile(&mut acc, file.pile_name())
                                .add_deleted(file.relative_path().clone());
                        }
                        ItemOperation::Nothing(file) => {
                            let checksum = Self::require_checksum(
                                file.system_checksum(Self::checksum_type(hoard, &file))?,
                                file.relative_path(),
                            )?;
                            Self::get_or_create_pile(&mut acc, file.pile_name())
                                .add_unmodified(file.relative_path().clone(), checksum);
                        }
                    }

                    Ok(acc)
                },
            )?;

        let empty = PileName::anonymous();
        if inner.len() == 1 && inner.contains_key(&empty) {
            Ok(Self::Anonymous(inner.remove(&empty).unwrap()))
        } else {
            let inner = inner
                .into_iter()
                .map(|(key, val)| {
                    key.try_into()
                        .map(|key| (key, val))
                        .map_err(|_| Error::MixedPileNames)
                })
                .collect::<Result<_, _>>()?;
            Ok(Self::Named(inner))
        }
    }

    fn get_pile(&self, name: &PileName) -> Option<&Pile> {
        match (name.as_ref(), self) {
            (None, Hoard::Anonymous(pile)) => Some(pile),
            (Some(name), Hoard::Named(piles)) => piles.get(name),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize, Default)]
struct Pile {
    created: HashMap<RelativePath, Checksum>,
    modified: HashMap<RelativePath, Checksum>,
    deleted: HashSet<RelativePath>,
    unmodified: HashMap<RelativePath, Checksum>,
}

impl Pile {
    fn new() -> Self {
        Self::default()
    }

    fn add_created(&mut self, path: RelativePath, checksum: Checksum) {
        self.created.insert(path, checksum);
    }

    fn add_modified(&mut self, path: RelativePath, checksum: Checksum) {
        self.modified.insert(path, checksum);
    }

    fn add_deleted(&mut self, path: RelativePath) {
        self.deleted.insert(path);
    }

    fn add_unmodified(&mut self, path: RelativePath, checksum: Checksum) {
        self.unmodified.insert(path, checksum);
    }

    fn contains_file(&self, rel_path: &RelativePath, only_modified: bool) -> bool {
        self.created.contains_key(rel_path)
            || self.modified.contains_key(rel_path)
            || self.deleted.contains(rel_path)
            || (!only_modified && self.unmodified.contains_key(rel_path))
    }

    fn checksum_for(&self, rel_path: &RelativePath) -> Option<Checksum> {
        self.created
            .get(rel_path)
            .or_else(|| self.modified.get(rel_path))
            .or_else(|| self.unmodified.get(rel_path))
            .map(Clone::clone)
    }

    fn all_files_with_checksums(&self) -> impl Iterator<Item = (&RelativePath, Option<Checksum>)> {
        let created = self
            .created
            .iter()
            .map(|(path, checksum)| (path, Some(checksum.clone())));
        let modified = self
            .modified
            .iter()
            .map(|(path, checksum)| (path, Some(checksum.clone())));
        let unmodified = self
            .unmodified
            .iter()
            .map(|(path, checksum)| (path, Some(checksum.clone())));
        let deleted = self.deleted.iter().map(|path| (path, None));

        created.chain(modified).chain(unmodified).chain(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksum::MD5;
    use serde_test::{assert_tokens, Token};
    use std::path::PathBuf;

    #[test]
    fn test_checksum_derives() {
        let checksum = Checksum::MD5(MD5::from_data("testing"));
        assert!(format!("{:?}", checksum).contains("MD5"));
        assert_eq!(checksum, checksum.clone());
        assert_tokens(
            &checksum,
            &[
                Token::Enum { name: "Checksum" },
                Token::Str("md5"),
                Token::Str("ae2b1fca515949e5d54fb22b8ed95575"),
            ],
        );
    }

    mod v2_from_v1 {
        use super::super::super::v1;
        use super::*;
        use maplit;
        use time::Duration;

        fn assert_conversion(ops_v1: Vec<v1::OperationV1>, ops_v2: Vec<OperationV2>) {
            let mut mapping = HashMap::new();
            let mut file_set = HashSet::new();

            for (op_v1, op_v2) in ops_v1.into_iter().zip(ops_v2) {
                let new_op = OperationV2::from_v1(&mut mapping, &mut file_set, op_v1);
                assert_eq!(op_v2, new_op);
            }
        }

        #[test]
        fn test_from_anonymous_file() {
            let first_timestamp = time::OffsetDateTime::now_utc();
            let second_timestamp = first_timestamp - Duration::hours(2);
            let third_timestamp = second_timestamp - Duration::hours(2);
            let hoard_name: HoardName = "anon_file".parse().unwrap();
            let ops_v1 = vec![
                v1::OperationV1 {
                    timestamp: first_timestamp,
                    is_backup: true,
                    hoard_name: hoard_name.clone(),
                    hoard: v1::Hoard::Anonymous(v1::Pile(
                        maplit::hashmap! { RelativePath::none() => Checksum::MD5("d3369a026ace494f56ead54d502a00dd".parse().unwrap()) },
                    )),
                },
                v1::OperationV1 {
                    timestamp: second_timestamp,
                    is_backup: false,
                    hoard_name: hoard_name.clone(),
                    hoard: v1::Hoard::Anonymous(v1::Pile(
                        maplit::hashmap! { RelativePath::none() => Checksum::MD5("d3369a026ace494f56ead54d502a00dd".parse().unwrap()) },
                    )),
                },
                v1::OperationV1 {
                    timestamp: third_timestamp,
                    is_backup: true,
                    hoard_name: hoard_name.clone(),
                    hoard: v1::Hoard::Anonymous(v1::Pile(HashMap::new())),
                },
            ];
            let ops_v2 = vec![
                OperationV2 {
                    timestamp: first_timestamp,
                    direction: Direction::Backup,
                    hoard: hoard_name.clone(),
                    files: Hoard::Anonymous({
                        let mut pile = Pile::new();
                        pile.add_created(
                            RelativePath::none(),
                            Checksum::MD5("d3369a026ace494f56ead54d502a00dd".parse().unwrap()),
                        );
                        pile
                    }),
                },
                OperationV2 {
                    timestamp: second_timestamp,
                    direction: Direction::Restore,
                    hoard: hoard_name.clone(),
                    files: Hoard::Anonymous({
                        let mut pile = Pile::new();
                        pile.add_unmodified(
                            RelativePath::none(),
                            Checksum::MD5("d3369a026ace494f56ead54d502a00dd".parse().unwrap()),
                        );
                        pile
                    }),
                },
                OperationV2 {
                    timestamp: third_timestamp,
                    direction: Direction::Backup,
                    hoard: hoard_name,
                    files: Hoard::Anonymous({
                        let mut pile = Pile::new();
                        pile.add_deleted(RelativePath::none());
                        pile
                    }),
                },
            ];

            assert_conversion(ops_v1, ops_v2);
        }

        #[test]
        fn test_from_anonymous_dir() {
            let first_timestamp = time::OffsetDateTime::now_utc();
            let second_timestamp = first_timestamp - Duration::hours(2);
            let third_timestamp = second_timestamp - Duration::hours(2);
            let hoard_name: HoardName = "anon_dir".parse().unwrap();
            let ops_v1 = vec![
                v1::OperationV1 {
                    timestamp: first_timestamp,
                    is_backup: true,
                    hoard_name: hoard_name.clone(),
                    hoard: v1::Hoard::Anonymous(v1::Pile(maplit::hashmap! {
                        RelativePath::try_from(PathBuf::from("file_1")).unwrap() => Checksum::MD5("ba9d332813a722b273a95fa13dd88d94".parse().unwrap()),
                        RelativePath::try_from(PathBuf::from("file_2")).unwrap() => Checksum::MD5("92ed3b5f07b44bc4f70d0b24d5e1867c".parse().unwrap()),
                    })),
                },
                v1::OperationV1 {
                    timestamp: second_timestamp,
                    is_backup: true,
                    hoard_name: hoard_name.clone(),
                    hoard: v1::Hoard::Anonymous(v1::Pile(maplit::hashmap! {
                        RelativePath::try_from(PathBuf::from("file_1")).unwrap() => Checksum::MD5("1cfab2a192005a9a8bdc69106b4627e2".parse().unwrap()),
                        RelativePath::try_from(PathBuf::from("file_2")).unwrap() => Checksum::MD5("92ed3b5f07b44bc4f70d0b24d5e1867c".parse().unwrap()),
                        RelativePath::try_from(PathBuf::from("file_3")).unwrap() => Checksum::MD5("797b373a9c4ec0d6de0a31a90b5bee8e".parse().unwrap()),
                    })),
                },
                v1::OperationV1 {
                    timestamp: third_timestamp,
                    is_backup: true,
                    hoard_name: hoard_name.clone(),
                    hoard: v1::Hoard::Anonymous(v1::Pile(maplit::hashmap! {
                        RelativePath::try_from(PathBuf::from("file_1")).unwrap() => Checksum::MD5("1cfab2a192005a9a8bdc69106b4627e2".parse().unwrap()),
                        RelativePath::try_from(PathBuf::from("file_3")).unwrap() => Checksum::MD5("1deb21ef3bb87be4ad71d73fff6bb8ec".parse().unwrap()),
                    })),
                },
            ];
            let ops_v2 = vec![
                OperationV2 {
                    timestamp: first_timestamp,
                    direction: Direction::Backup,
                    hoard: hoard_name.clone(),
                    files: Hoard::Anonymous({
                        let mut pile = Pile::new();
                        pile.add_created(
                            RelativePath::try_from(PathBuf::from("file_1")).unwrap(),
                            Checksum::MD5("ba9d332813a722b273a95fa13dd88d94".parse().unwrap()),
                        );
                        pile.add_created(
                            RelativePath::try_from(PathBuf::from("file_2")).unwrap(),
                            Checksum::MD5("92ed3b5f07b44bc4f70d0b24d5e1867c".parse().unwrap()),
                        );
                        pile
                    }),
                },
                OperationV2 {
                    timestamp: second_timestamp,
                    direction: Direction::Backup,
                    hoard: hoard_name.clone(),
                    files: Hoard::Anonymous({
                        let mut pile = Pile::new();
                        pile.add_created(
                            RelativePath::try_from(PathBuf::from("file_3")).unwrap(),
                            Checksum::MD5("797b373a9c4ec0d6de0a31a90b5bee8e".parse().unwrap()),
                        );
                        pile.add_modified(
                            RelativePath::try_from(PathBuf::from("file_1")).unwrap(),
                            Checksum::MD5("1cfab2a192005a9a8bdc69106b4627e2".parse().unwrap()),
                        );
                        pile.add_unmodified(
                            RelativePath::try_from(PathBuf::from("file_2")).unwrap(),
                            Checksum::MD5("92ed3b5f07b44bc4f70d0b24d5e1867c".parse().unwrap()),
                        );
                        pile
                    }),
                },
                OperationV2 {
                    timestamp: third_timestamp,
                    direction: Direction::Backup,
                    hoard: hoard_name,
                    files: Hoard::Anonymous({
                        let mut pile = Pile::new();
                        pile.add_modified(
                            RelativePath::try_from(PathBuf::from("file_3")).unwrap(),
                            Checksum::MD5("1deb21ef3bb87be4ad71d73fff6bb8ec".parse().unwrap()),
                        );
                        pile.add_deleted(RelativePath::try_from(PathBuf::from("file_2")).unwrap());
                        pile.add_unmodified(
                            RelativePath::try_from(PathBuf::from("file_1")).unwrap(),
                            Checksum::MD5("1cfab2a192005a9a8bdc69106b4627e2".parse().unwrap()),
                        );
                        pile
                    }),
                },
            ];

            assert_conversion(ops_v1, ops_v2);
        }

        #[test]
        #[allow(clippy::too_many_lines)]
        fn test_from_named() {
            let first_timestamp = time::OffsetDateTime::now_utc();
            let second_timestamp = first_timestamp - Duration::hours(2);
            let third_timestamp = second_timestamp - Duration::hours(2);
            let hoard_name: HoardName = "named".parse().unwrap();
            let ops_v1 = vec![
                v1::OperationV1 {
                    timestamp: first_timestamp,
                    is_backup: true,
                    hoard_name: hoard_name.clone(),
                    hoard: v1::Hoard::Named(maplit::hashmap! {
                        "single_file".parse().unwrap() => v1::Pile(maplit::hashmap! { RelativePath::none() => Checksum::MD5("d3369a026ace494f56ead54d502a00dd".parse().unwrap()) }),
                        "dir".parse().unwrap() => v1::Pile(maplit::hashmap! {
                            RelativePath::try_from(PathBuf::from("file_1")).unwrap() => Checksum::MD5("ba9d332813a722b273a95fa13dd88d94".parse().unwrap()),
                            RelativePath::try_from(PathBuf::from("file_2")).unwrap() => Checksum::MD5("92ed3b5f07b44bc4f70d0b24d5e1867c".parse().unwrap()),
                        })
                    }),
                },
                v1::OperationV1 {
                    timestamp: second_timestamp,
                    is_backup: true,
                    hoard_name: hoard_name.clone(),
                    hoard: v1::Hoard::Named(maplit::hashmap! {
                        "single_file".parse().unwrap() => v1::Pile(maplit::hashmap! { RelativePath::none() => Checksum::MD5("d3369a026ace494f56ead54d502a00dd".parse().unwrap()) }),
                        "dir".parse().unwrap() => v1::Pile(maplit::hashmap! {
                            RelativePath::try_from(PathBuf::from("file_1")).unwrap() => Checksum::MD5("1cfab2a192005a9a8bdc69106b4627e2".parse().unwrap()),
                            RelativePath::try_from(PathBuf::from("file_2")).unwrap() => Checksum::MD5("92ed3b5f07b44bc4f70d0b24d5e1867c".parse().unwrap()),
                            RelativePath::try_from(PathBuf::from("file_3")).unwrap() => Checksum::MD5("797b373a9c4ec0d6de0a31a90b5bee8e".parse().unwrap()),
                        })
                    }),
                },
                v1::OperationV1 {
                    timestamp: third_timestamp,
                    is_backup: true,
                    hoard_name: hoard_name.clone(),
                    hoard: v1::Hoard::Named(maplit::hashmap! {
                        "single_file".parse().unwrap() => v1::Pile(HashMap::new()),
                        "dir".parse().unwrap() => v1::Pile(maplit::hashmap! {
                            RelativePath::try_from(PathBuf::from("file_1")).unwrap() => Checksum::MD5("1cfab2a192005a9a8bdc69106b4627e2".parse().unwrap()),
                            RelativePath::try_from(PathBuf::from("file_3")).unwrap() => Checksum::MD5("1deb21ef3bb87be4ad71d73fff6bb8ec".parse().unwrap()),
                        })
                    }),
                },
            ];
            let ops_v2 = vec![
                OperationV2 {
                    timestamp: first_timestamp,
                    direction: Direction::Backup,
                    hoard: hoard_name.clone(),
                    files: Hoard::Named(maplit::hashmap! {
                        "single_file".parse().unwrap() => {
                            let mut pile = Pile::new();
                            pile.add_created(RelativePath::none(), Checksum::MD5("d3369a026ace494f56ead54d502a00dd".parse().unwrap()));
                            pile
                        },
                        "dir".parse().unwrap() => {
                            let mut pile = Pile::new();
                            pile.add_created(
                                RelativePath::try_from(PathBuf::from("file_1")).unwrap(),
                                Checksum::MD5("ba9d332813a722b273a95fa13dd88d94".parse().unwrap())
                            );
                            pile.add_created(
                                RelativePath::try_from(PathBuf::from("file_2")).unwrap(),
                                Checksum::MD5("92ed3b5f07b44bc4f70d0b24d5e1867c".parse().unwrap())
                            );
                            pile
                        }
                    }),
                },
                OperationV2 {
                    timestamp: second_timestamp,
                    direction: Direction::Backup,
                    hoard: hoard_name.clone(),
                    files: Hoard::Named(maplit::hashmap! {
                        "single_file".parse().unwrap() => {
                            let mut pile = Pile::new();
                            pile.add_unmodified(RelativePath::none(), Checksum::MD5("d3369a026ace494f56ead54d502a00dd".parse().unwrap()));
                            pile
                        },
                        "dir".parse().unwrap() => {
                            let mut pile = Pile::new();
                            pile.add_modified(
                                RelativePath::try_from(PathBuf::from("file_1")).unwrap(),
                                Checksum::MD5("1cfab2a192005a9a8bdc69106b4627e2".parse().unwrap())
                            );
                            pile.add_unmodified(
                                RelativePath::try_from(PathBuf::from("file_2")).unwrap(),
                                Checksum::MD5("92ed3b5f07b44bc4f70d0b24d5e1867c".parse().unwrap())
                            );
                            pile.add_created(
                                RelativePath::try_from(PathBuf::from("file_3")).unwrap(),
                                Checksum::MD5("797b373a9c4ec0d6de0a31a90b5bee8e".parse().unwrap())
                            );
                            pile
                        }
                    }),
                },
                OperationV2 {
                    timestamp: third_timestamp,
                    direction: Direction::Backup,
                    hoard: hoard_name.parse().unwrap(),
                    files: Hoard::Named(maplit::hashmap! {
                        "single_file".parse().unwrap() => {
                            let mut pile = Pile::new();
                            pile.add_deleted(RelativePath::none());
                            pile
                        },
                        "dir".parse().unwrap() => {
                            let mut pile = Pile::new();
                            pile.add_unmodified(
                                RelativePath::try_from(PathBuf::from("file_1")).unwrap(),
                                Checksum::MD5("1cfab2a192005a9a8bdc69106b4627e2".parse().unwrap())
                            );
                            pile.add_deleted(
                                RelativePath::try_from(PathBuf::from("file_2")).unwrap(),
                            );
                            pile.add_modified(
                                RelativePath::try_from(PathBuf::from("file_3")).unwrap(),
                                Checksum::MD5("1deb21ef3bb87be4ad71d73fff6bb8ec".parse().unwrap())
                            );
                            pile
                        }
                    }),
                },
            ];

            assert_conversion(ops_v1, ops_v2);
        }
    }
}
