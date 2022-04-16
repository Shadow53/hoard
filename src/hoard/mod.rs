//! This module contains processed versions of builder
//! [`Hoard`](crate::config::builder::hoard::Hoard)s. See documentation for builder `Hoard`s
//! for more details.

pub(crate) mod iter;
pub(crate) mod pile_config;

use crate::filters::Error as FilterError;
use crate::newtypes::{NonEmptyPileName, PileName};
use crate::paths::{HoardPath, RelativePath, SystemPath};
pub use pile_config::Config as PileConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can happen while backing up or restoring a hoard.
#[derive(Debug, Error)]
pub enum Error {
    /// Error while copying a file.
    #[error("failed to copy {src} to {dest}: {error}")]
    CopyFile {
        /// The path of the source file.
        src: PathBuf,
        /// The path of the destination file.
        dest: PathBuf,
        /// The I/O error that occurred.
        #[source]
        error: io::Error,
    },
    /// Error while creating a directory.
    #[error("failed to create {path}: {error}")]
    CreateDir {
        /// The path of the directory to create.
        path: PathBuf,
        /// The error that occurred while creating.
        #[source]
        error: io::Error,
    },
    /// Error while reading a directory or an item in a directory.
    #[error("cannot read directory {path}: {error}")]
    ReadDir {
        /// The path of the file or directory to read.
        path: PathBuf,
        /// The error that occurred while reading.
        #[source]
        error: io::Error,
    },
    /// Both the source and destination exist but are not both directories or both files.
    #[error("both source (\"{src}\") and destination (\"{dest}\") exist but are not both files or both directories")]
    TypeMismatch {
        /// Source path/
        src: PathBuf,
        /// Destination path.
        dest: PathBuf,
    },
    /// An error occurred while filtering files.
    #[error("error while filtering files: {0}")]
    Filter(#[from] FilterError),
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "lowercase")]
/// Indicates which direction files are being copied in. Used to determine which files are required
/// to exist.
pub enum Direction {
    /// Backing up from system to hoards.
    Backup,
    /// Restoring from hoards to system.
    Restore,
}

/// A single path to hoard, with configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct Pile {
    /// Optional configuration for this path.
    pub config: PileConfig,
    /// The path to hoard.
    ///
    /// The path is optional because it will almost always be set by processing a configuration
    /// file and it is possible that none of the environment combinations match.
    pub path: Option<SystemPath>,
}

/// A collection of multiple related [`Pile`]s.
#[derive(Clone, Debug, PartialEq)]
pub struct MultipleEntries {
    /// The named [`Pile`]s in the hoard.
    pub piles: HashMap<NonEmptyPileName, Pile>,
}

/// A configured hoard. May contain one or more [`Pile`]s.
#[derive(Clone, Debug, PartialEq)]
#[allow(variant_size_differences)]
pub enum Hoard {
    /// A single anonymous [`Pile`].
    Anonymous(Pile),
    /// Multiple named [`Pile`]s.
    Named(MultipleEntries),
}

impl Hoard {
    /// Returns an iterator over all piles with associates paths.
    ///
    /// The [`HoardPath`] and [`SystemPath`] represent the relevant prefix/root path for the given pile.
    #[must_use]
    pub fn get_paths(
        &self,
        hoards_root: HoardPath,
    ) -> Box<dyn Iterator<Item = (PileName, HoardPath, SystemPath)>> {
        match self {
            Hoard::Anonymous(pile) => match pile.path.clone() {
                None => Box::new(std::iter::empty()),
                Some(path) => Box::new(std::iter::once({
                    (PileName::anonymous(), hoards_root, path)
                })),
            },
            Hoard::Named(named) => Box::new(named.piles.clone().into_iter().filter_map(
                move |(name, pile)| {
                    pile.path.map(|path| {
                        let pile_hoard_root = hoards_root.join(&RelativePath::from(&name));
                        (name.into(), pile_hoard_root, path)
                    })
                },
            )),
        }
    }

    /// Returns the pile with the given [`PileName`], if exists.
    pub fn get_pile(&self, name: &PileName) -> Option<&Pile> {
        match (name.as_ref(), self) {
            (None, Self::Anonymous(pile)) => Some(pile),
            (Some(name), Self::Named(map)) => map.piles.get(name),
            _ => None,
        }
    }
}
