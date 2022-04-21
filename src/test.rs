//! Items to be used only for testing Hoard-related code.
#![cfg(test)]

use std::{
    fs, io,
    path::{Path, PathBuf},
};

use tempfile::TempDir;
use thiserror::Error;
use uuid::Uuid;

mod macros {
    macro_rules! path_string {
        ($path: expr) => {{
            #[cfg(unix)]
            {
                String::from($path)
            }
            #[cfg(windows)]
            {
                format!("C:{}", $path).replace("/", "\\")
            }
        }};
    }
    macro_rules! system_path {
        ($path: expr) => {{
            let path = crate::test::path_string!($path);
            crate::paths::SystemPath::try_from(PathBuf::from(path)).unwrap()
        }};
    }
    pub(crate) use path_string;
    pub(crate) use system_path;
}

pub(crate) use macros::{path_string, system_path};

#[derive(Debug, Error)]
#[allow(variant_size_differences)]
pub enum Error {
    #[error("I/O error: {0}")]
    IO(#[from] io::Error),
    #[error("failed to parse UUID: {0}")]
    Uuid(#[from] uuid::Error),
}

#[derive(Debug)]
pub struct Tester {
    config_dir: TempDir,
    data_dir: TempDir,
}

impl Tester {
    /// Create a new `Tester`.
    ///
    /// This creates temporary config and data directories and uses the `HOARD_*_DIR` environment
    /// variables to override the directories.
    ///
    /// The temporary directories will be cleaned up when this `Tester` is dropped.
    ///
    /// # Errors
    ///
    /// Any I/O errors while creating the temporary directories.
    pub fn new() -> io::Result<Self> {
        let config_dir = TempDir::new()?;
        let data_dir = TempDir::new()?;

        std::env::set_var("HOARD_DATA_DIR", data_dir.path());
        std::env::set_var("HOARD_CONFIG_DIR", config_dir.path());

        Ok(Self {
            config_dir,
            data_dir,
        })
    }

    /// Returns the overridden config directory.
    #[must_use]
    pub fn config_dir(&self) -> &Path {
        self.config_dir.path()
    }

    /// Returns the overridden data directory.
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        self.data_dir.path()
    }

    /// Returns the path to the UUID file
    #[must_use]
    pub fn uuid_path(&self) -> PathBuf {
        self.config_dir().join("uuid")
    }

    /// Return the current system UUID.
    ///
    /// # Errors
    ///
    /// - I/O errors while reading the UUID file.
    /// - Errors while parsing the file contents as a UUID string.
    pub fn uuid(&self) -> Result<Uuid, Error> {
        let data = fs::read_to_string(self.uuid_path())?;
        data.parse::<Uuid>().map_err(Error::from)
    }

    /// Writes the given UUID as a string to the UUID file.
    ///
    /// # Errors
    ///
    /// Any I/O errors that may occur while writing.
    pub fn set_uuid(&self, id: &Uuid) -> io::Result<()> {
        fs::write(self.uuid_path(), id.to_string())
    }
}
