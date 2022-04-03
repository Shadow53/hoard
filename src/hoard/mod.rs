//! This module contains processed versions of builder
//! [`Hoard`](crate::config::builder::hoard::Hoard)s. See documentation for builder `Hoard`s
//! for more details.

pub(crate) mod iter;
pub(crate) mod pile_config;

use crate::checkers::history::last_paths::HoardPaths;
use crate::filters::Error as FilterError;
use crate::newtypes::NonEmptyPileName;
use crate::paths::SystemPath;
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
    /// Returns a [`HoardPaths`] based on this `Hoard`.
    #[must_use]
    pub fn get_paths(&self) -> HoardPaths {
        match self {
            Hoard::Anonymous(pile) => pile.path.clone().into(),
            Hoard::Named(piles) => piles
                .piles
                .iter()
                .filter_map(|(key, val)| val.path.clone().map(|path| (key.clone(), path)))
                .collect::<HashMap<_, _>>()
                .into(),
        }
    }
}
