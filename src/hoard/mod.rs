//! This module contains processed versions of builder
//! [`Hoard`](crate::config::builder::hoard::Hoard)s. See documentation for builder `Hoard`s
//! for more details.

pub(crate) mod iter;
pub(crate) mod pile_config;

use crate::checkers::history::last_paths::HoardPaths;
use crate::filters::{Error as FilterError, Filter, Filters};
pub use pile_config::Config as PileConfig;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{fs, io};
use std::ops::Deref;
use serde::{Serialize, Deserialize};
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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
/// Indicates which direction files are being copied in. Used to determine which files are required
/// to exist.
pub enum Direction {
    /// Backing up from system to hoards.
    Backup,
    /// Restoring from hoards to system.
    Restore,
}

#[repr(transparent)]
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct HoardPath(PathBuf);
#[repr(transparent)]
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SystemPath(PathBuf);

impl AsRef<Path> for HoardPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Deref for HoardPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<Path> for SystemPath {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Deref for SystemPath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl From<PathBuf> for HoardPath {
    fn from(p: PathBuf) -> Self {
        Self(p)
    }
}

impl From<PathBuf> for SystemPath {
    fn from(p: PathBuf) -> Self {
        Self(p)
    }
}

/// A single path to hoard, with configuration.
#[derive(Clone, Debug, PartialEq)]
pub struct Pile {
    /// Optional configuration for this path.
    pub config: Option<PileConfig>,
    /// The path to hoard.
    ///
    /// The path is optional because it will almost always be set by processing a configuration
    /// file and it is possible that none of the environment combinations match.
    pub path: Option<PathBuf>,
}

impl Pile {
    /// Helper function for copying files and directories.
    ///
    /// The returned [`PilePaths`] has items inserted as (src, dest).
    ///
    /// # Errors
    ///
    /// Various sorts of I/O errors as the different [`Error`] variants.
    fn copy(
        filters: Option<&Filters>,
        root_prefix: &Path,
        src: &Path,
        dest: &Path,
    ) -> Result<(), Error> {
        let _span = tracing::trace_span!(
            "copy",
            source = ?src,
            destination = ?dest,
            ?root_prefix,
        )
        .entered();

        if !src.exists() {
            tracing::warn!(path=?src, "source path does not exist; skipping");
            return Ok(());
        }

        if !filters
            .as_ref()
            .map_or(true, |filters| filters.keep(root_prefix, src))
        {
            // File should be ignored (not kept), so do nothing.
            tracing::trace!(path=%src.display(), "ignoring path based on filters");
            return Ok(());
        }

        // Fail if src and dest exist but are not both file or directory.
        if src.exists() == dest.exists()
            && src.is_dir() != dest.is_dir()
            && src.is_file() != dest.is_file()
        {
            return Err(Error::TypeMismatch {
                src: src.to_owned(),
                dest: dest.to_owned(),
            });
        }

        if src.is_dir() {
            let _span = tracing::trace_span!("is_directory").entered();

            let dir_contents = fs::read_dir(src).map_err(|err| Error::ReadDir {
                path: src.to_owned(),
                error: err,
            })?;

            for item in dir_contents {
                let item = item.map_err(|err| Error::ReadDir {
                    path: src.to_owned(),
                    error: err,
                })?;

                let dest = dest.join(item.file_name());
                // No tracing event here because we are recursing
                Self::copy(filters, root_prefix, &item.path(), &dest)?;
            }
        } else if src.is_file() {
            let _span = tracing::trace_span!("is_file").entered();

            // Create parent directory only if there is an actual file to copy.
            // Avoids unnecessarily creating empty directories.
            if let Some(parent) = dest.parent() {
                tracing::trace!(
                    destination = src.to_string_lossy().as_ref(),
                    "ensuring parent directories for destination",
                );
                fs::create_dir_all(parent).map_err(|err| Error::CreateDir {
                    path: dest.to_owned(),
                    error: err,
                })?;
            }

            tracing::debug!(
                source = src.to_string_lossy().as_ref(),
                destination = dest.to_string_lossy().as_ref(),
                "copying",
            );

            fs::copy(src.to_owned(), dest).map_err(|err| Error::CopyFile {
                src: src.to_owned(),
                dest: dest.to_owned(),
                error: err,
            })?;
        } else {
            tracing::warn!(
                source = src.to_string_lossy().as_ref(),
                "source is not a file or directory",
            );
        }

        Ok(())
    }

    /// Backs up files to the pile directory.
    ///
    /// `prefix` is the root directory for this pile. This should generally be
    /// `$HOARD_ROOT/$HOARD_NAME/($PILE_NAME)`.
    ///
    /// # Errors
    ///
    /// Various sorts of I/O errors as the different [`enum@Error`] variants.
    pub fn backup(&self, prefix: &Path) -> Result<(), Error> {
        if let Some(path) = &self.path {
            let _span = tracing::debug_span!(
                "backup_pile",
                path = path.to_string_lossy().as_ref(),
                prefix = prefix.to_string_lossy().as_ref()
            )
            .entered();

            let filter = self.config.as_ref().map(Filters::new).transpose()?;

            Self::copy(filter.as_ref(), path, path, prefix)?;
        } else {
            tracing::warn!("pile has no associated path -- perhaps no environment matched?");
        }

        Ok(())
    }

    /// Restores files from the hoard into the filesystem.
    ///
    /// # Errors
    ///
    /// Various sorts of I/O errors as the different [`enum@Error`] variants.
    pub fn restore(&self, prefix: &Path) -> Result<(), Error> {
        // TODO: do stuff with pile config
        if let Some(path) = &self.path {
            let _span = tracing::debug_span!(
                "restore_pile",
                path = path.to_string_lossy().as_ref(),
                prefix = prefix.to_string_lossy().as_ref()
            )
            .entered();

            Self::copy(None, prefix, prefix, path)?;
        } else {
            tracing::warn!("pile has no associated path -- perhaps no environment matched");
        }

        Ok(())
    }
}

/// A collection of multiple related [`Pile`]s.
#[derive(Clone, Debug, PartialEq)]
pub struct MultipleEntries {
    /// The named [`Pile`]s in the hoard.
    pub piles: HashMap<String, Pile>,
}

impl MultipleEntries {
    /// Back up all of the contained [`Pile`]s.
    ///
    /// # Errors
    ///
    /// See [`Pile::backup`].
    pub fn backup(&self, prefix: &Path) -> Result<(), Error> {
        for (name, entry) in &self.piles {
            let _span = tracing::info_span!(
                "backup_multi_pile",
                pile = %name
            )
            .entered();

            let sub_prefix = prefix.join(name);
            entry.backup(&sub_prefix)?;
        }

        Ok(())
    }

    /// Restore all of the contained [`Pile`]s.
    ///
    /// # Errors
    ///
    /// See [`Pile::restore`].
    pub fn restore(&self, prefix: &Path) -> Result<(), Error> {
        for (name, entry) in &self.piles {
            let _span = tracing::info_span!(
                "restore_multi_pile",
                pile = %name
            )
            .entered();

            let sub_prefix = prefix.join(name);
            entry.restore(&sub_prefix)?;
        }

        Ok(())
    }
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
    /// Back up this [`Hoard`].
    ///
    /// # Errors
    ///
    /// See [`Pile::backup`].
    pub fn backup(&self, prefix: &Path) -> Result<(), Error> {
        let _span =
            tracing::trace_span!("backup_hoard", prefix = prefix.to_string_lossy().as_ref())
                .entered();

        match self {
            Hoard::Anonymous(single) => single.backup(prefix),
            Hoard::Named(multiple) => multiple.backup(prefix),
        }
    }

    /// Restore this [`Hoard`].
    ///
    /// # Errors
    ///
    /// See [`Pile::restore`].
    pub fn restore(&self, prefix: &Path) -> Result<(), Error> {
        let _span =
            tracing::trace_span!("restore_hoard", prefix = prefix.to_string_lossy().as_ref(),)
                .entered();

        match self {
            Hoard::Anonymous(single) => single.restore(prefix),
            Hoard::Named(multiple) => multiple.restore(prefix),
        }
    }

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
