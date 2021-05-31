pub use super::builder::hoard::Config;
use log::debug;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::{fs, io};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to copy {src} to {dest}: {error}")]
    CopyFile {
        src: PathBuf,
        dest: PathBuf,
        error: io::Error,
    },
    #[error("failed to create {path}: {error}")]
    CreateDir { path: PathBuf, error: io::Error },
    #[error("cannot read directory {path}: {error}")]
    ReadDir { path: PathBuf, error: io::Error },
    #[error("both source (\"{src}\") and destination (\"{dest}\") exist but are not both files or both directories")]
    TypeMismatch { src: PathBuf, dest: PathBuf },
}

#[derive(Clone, Debug, PartialEq)]
pub struct SingleEntry {
    pub config: Option<Config>,
    pub path: Option<PathBuf>,
}

impl SingleEntry {
    fn copy(src: &Path, dest: &Path) -> Result<(), Error> {
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
            debug!("{} is a directory", src.to_string_lossy());

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
                Self::copy(&item.path(), &dest)?;
            }
        } else if src.is_file() {
            debug!("{} is a file", src.to_string_lossy());

            // Create parent directory only if there is an actual file to copy.
            // Avoids unnecessarily creating empty directories.
            if let Some(parent) = dest.parent() {
                debug!("ensuring parent directories");
                fs::create_dir_all(parent).map_err(|err| Error::CreateDir {
                    path: dest.to_owned(),
                    error: err,
                })?;
            }

            debug!(
                "Copying {} to {}",
                src.to_string_lossy(),
                dest.to_string_lossy()
            );

            fs::copy(src.to_owned(), dest).map_err(|err| Error::CopyFile {
                src: src.to_owned(),
                dest: dest.to_owned(),
                error: err,
            })?;
        }

        Ok(())
    }

    /// Backs up files to the pile directory.
    ///
    /// `prefix` is the root directory for this pile. This should generally be
    /// `$HOARD_ROOT/$HOARD_NAME/($PILE_NAME)`.
    pub fn backup(&self, prefix: &Path) -> Result<(), Error> {
        // TODO: do stuff with config
        if let Some(path) = &self.path {
            Self::copy(path, prefix)?;
        }

        Ok(())
    }

    pub fn restore(&self, prefix: &Path) -> Result<(), Error> {
        // TODO: do stuff with config
        if let Some(path) = &self.path {
            Self::copy(prefix, path)?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct MultipleEntries {
    pub items: BTreeMap<String, SingleEntry>,
}

impl MultipleEntries {
    pub fn backup(&self, prefix: &Path) -> Result<(), Error> {
        for entry in self.items.values() {
            entry.backup(prefix)?;
        }

        Ok(())
    }

    pub fn restore(&self, prefix: &Path) -> Result<(), Error> {
        for entry in self.items.values() {
            entry.restore(prefix)?;
        }

        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
#[allow(variant_size_differences)]
pub enum Hoard {
    Single(SingleEntry),
    Multiple(MultipleEntries),
}

impl Hoard {
    pub fn backup(&self, prefix: &Path) -> Result<(), Error> {
        match self {
            Hoard::Single(single) => single.backup(prefix),
            Hoard::Multiple(multiple) => multiple.backup(prefix),
        }
    }

    pub fn restore(&self, prefix: &Path) -> Result<(), Error> {
        match self {
            Hoard::Single(single) => single.restore(prefix),
            Hoard::Multiple(multiple) => multiple.restore(prefix),
        }
    }
}
