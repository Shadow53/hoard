//! Keeping track of all operations.
//!
//! The types in this module are used for logging all operations to disk. This information can be
//! used for debugging purposes, but is more directly used as a [`Checker`] to help prevent
//! synchronized changes from being overwritten.
//!
//! It does this by parsing synchronized logs from this and other systems to determine which system
//! was the last one to touch a file.

use crate::hoard::{Direction, Hoard as ConfigHoard, Pile as ConfigPile};
use md5::{Digest, Md5};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::convert::TryFrom;
use std::path::{Path, PathBuf};
use std::fs;
use time::OffsetDateTime;
use crate::checkers::history::operation::Checksum;
use super::Error;

/// A single operation log.
///
/// This keeps track of the timestamp of the operation (which may include multiple hoards),
/// all hoards involved in the operation (and the related [`HoardOperation`]), and a record
/// of the latest operation log for each external system at the time of invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(clippy::module_name_repetitions)]
pub(crate) struct OperationV1 {
    /// Timestamp of last operation
    pub(crate) timestamp: OffsetDateTime,
    /// Whether this operation was a backup
    pub(crate) is_backup: bool,
    /// The name of the hoard for this `HoardOperation`.
    pub(crate) hoard_name: String,
    /// Mapping of pile files to checksums
    pub(crate) hoard: Hoard,
}

impl OperationV1 {
    fn new(name: &str, hoard: &ConfigHoard, direction: Direction) -> Result<Self, Error> {
        Ok(Self {
            timestamp: OffsetDateTime::now_utc(),
            is_backup: matches!(direction, Direction::Backup),
            hoard_name: name.into(),
            hoard: Hoard::try_from(hoard)?,
        })
    }
}

impl super::OperationImpl for OperationV1 {
    fn is_backup(&self) -> bool {
        self.is_backup
    }

    fn contains_file(&self, pile_name: Option<&str>, rel_path: &Path) -> bool {
        match (pile_name, &self.hoard) {
            (None, Hoard::Anonymous(pile)) => {
                pile.0.contains_key(rel_path)
            },
            (Some(name), Hoard::Named(piles)) => {
                piles.get(name).map_or(false, |pile| pile.0.contains_key(rel_path))
            },
            _ => false,
        }
    }

    fn timestamp(&self) -> OffsetDateTime {
        self.timestamp
    }

    fn hoard_name(&self) -> &str {
        &self.hoard_name
    }

    fn checksum_for(&self, pile_name: Option<&str>, rel_path: &Path) -> Option<Checksum> {
        match (pile_name, &self.hoard) {
            (None, Hoard::Anonymous(pile)) => {
                pile.0.get(rel_path).map(|md5| Checksum::MD5(md5.to_string()))
            },
            (Some(name), Hoard::Named(piles)) => {
                piles.get(name).and_then(|pile| pile.0.get(rel_path).map(|md5| Checksum::MD5(md5.to_string())))
            },
            _ => None,
        }
    }

    /// Returns, in order: the pile name, the relative path, and the file's checksum.
    fn all_files_with_checksums<'s>(&'s self) -> Box<dyn Iterator<Item=(&str, Option<&str>, &Path, Option<Checksum>)> + 's> {
        match &self.hoard {
            Hoard::Anonymous(pile) => Box::new(pile.0.iter().map(move |(path, md5)| {
                (self.hoard_name.as_str(), None, path.as_path(), Some(Checksum::MD5(md5.clone())))
            })),
            Hoard::Named(piles) => Box::new({
                piles.iter().flat_map(move |(pile_name, pile)| {
                    pile.0.iter().map(move |(rel_path, md5)| {
                        (self.hoard_name.as_str(), Some(pile_name.as_str()), rel_path.as_path(), Some(Checksum::MD5(md5.clone())))
                    })
                })
            })
        }
    }
}

/// Operation log information for a single hoard.
///
/// Really just a wrapper for [`Pile`] because piles may be anonymous or named.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) enum Hoard {
    /// Information for a single, anonymous pile.
    Anonymous(Pile),
    /// Information for some number of named piles.
    Named(HashMap<String, Pile>),
}

impl TryFrom<&ConfigHoard> for Hoard {
    type Error = Error;
    fn try_from(hoard: &ConfigHoard) -> Result<Self, Self::Error> {
        let _span = tracing::trace_span!("hoard_to_operation", ?hoard).entered();
        match hoard {
            ConfigHoard::Anonymous(pile) => Pile::try_from(pile).map(Hoard::Anonymous),
            ConfigHoard::Named(map) => map
                .piles
                .iter()
                .map(|(key, pile)| Pile::try_from(pile).map(|pile| (key.clone(), pile)))
                .collect::<Result<HashMap<_, _>, _>>()
                .map(Hoard::Named),
        }
    }
}

/// A mapping of file path (relative to pile) to file checksum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct Pile(HashMap<PathBuf, String>);

impl Pile {
    pub(crate) fn get(&'_ self, key: &Path) -> Option<&'_ str> {
        self.0.get(key).map(String::as_str)
    }
}

fn hash_path(path: &Path, root: &Path) -> Result<HashMap<PathBuf, String>, Error> {
    let mut map = HashMap::new();
    if path.is_file() {
        tracing::trace!(file=%path.display(), "Hashing file");
        let bytes = fs::read(path)?;
        let digest = Md5::digest(&bytes);
        let rel_path = path
            .strip_prefix(root)
            .expect("paths in hash_path should always be children of the given root")
            .to_path_buf();
        map.insert(rel_path, format!("{:x}", digest));
    } else if path.is_dir() {
        tracing::trace!(dir=%path.display(), "Hashing all files in dir");
        for item in fs::read_dir(path)? {
            let item = item?;
            let path = item.path();
            map.extend(hash_path(&path, root)?);
        }
    } else {
        tracing::warn!(path=%path.display(), "path is neither file nor directory, skipping");
    }

    Ok(map)
}

impl TryFrom<&ConfigPile> for Pile {
    type Error = Error;
    fn try_from(pile: &ConfigPile) -> Result<Self, Self::Error> {
        let _span = tracing::trace_span!("pile_to_operation", ?pile).entered();
        pile.path.as_ref().map_or_else(
            || Ok(Self(HashMap::new())),
            |path| hash_path(path, path).map(Self),
        )
    }
}