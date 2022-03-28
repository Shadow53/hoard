//! Types for working with files that are managed by Hoard.

use crate::paths::{HoardPath, RelativePath, SystemPath};
use std::io::ErrorKind;
use std::path::Path;
use std::{fs, io};
use crate::checksum::{Checksum, ChecksumType, MD5, SHA256};
use crate::newtypes::PileName;

/// A Hoard-managed path with associated methods.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HoardItem {
    pile_name: PileName,
    hoard_prefix: HoardPath,
    system_prefix: SystemPath,
    hoard_path: HoardPath,
    system_path: SystemPath,
    relative_path: RelativePath,
}

impl HoardItem {
    /// Create a new `HoardItem`.
    ///
    /// - `hoard_prefix` is the Hoard-controlled path for this item's pile root.
    ///   For example, a hoard `"a_hoard"` and pile `"a_pile"` will have
    ///   `hoard_prefix = "${DATA_DIR}/hoards/a_hoard/a_pile"`
    /// - `system_prefix` is the system path for this item's pile root.
    /// - `relative_path` is the common path relative to either prefix that this item
    ///   can be found at. For example, if `a_pile` is a directory with file `dir/some_file`,
    ///   `relative_path` is `Some(dir/some_file)`. If `a_pile` is a file, `relative_path == None`.
    #[must_use]
    pub fn new(
        pile_name: PileName,
        hoard_prefix: HoardPath,
        system_prefix: SystemPath,
        relative_path: RelativePath,
    ) -> Self {
        let (hoard_path, system_path) = {
            let hoard_path = hoard_prefix.join(&relative_path);
            let system_path = system_prefix.join(&relative_path);
            (hoard_path, system_path)
        };

        Self {
            pile_name,
            hoard_prefix,
            system_prefix,
            hoard_path,
            system_path,
            relative_path,
        }
    }

    /// Returns the name of the pile this item belongs to, if any.
    #[must_use]
    pub fn pile_name(&self) -> &PileName {
        &self.pile_name
    }

    /// Returns the relative path for this item.
    #[must_use]
    pub fn relative_path(&self) -> &RelativePath {
        &self.relative_path
    }

    /// Returns the hoard-controlled path for this item's pile.
    #[must_use]
    pub fn hoard_prefix(&self) -> &HoardPath {
        &self.hoard_prefix
    }

    /// Returns the system path for this item's pile.
    #[must_use]
    pub fn system_prefix(&self) -> &SystemPath {
        &self.system_prefix
    }

    /// Returns the Hoard-controlled path for this item.
    ///
    /// If [`HoardItem::relative_path()`] is `None`, this is the same as
    /// [`HoardItem::hoard_prefix()`].
    #[must_use]
    pub fn hoard_path(&self) -> &HoardPath {
        &self.hoard_path
    }

    /// Returns the system path for this item.
    ///
    /// If [`HoardItem::relative_path()`] is `None`, this is the same as
    /// [`HoardItem::system_prefix()`].
    #[must_use]
    pub fn system_path(&self) -> &SystemPath {
        &self.system_path
    }

    /// Returns whether this item is a file.
    ///
    /// This is `true` if:
    /// - At least one of `hoard_path` or `system_path` exists
    /// - All existing paths are a file
    #[must_use]
    pub fn is_file(&self) -> bool {
        let sys = self.system_path();
        let hoard = self.hoard_path();
        let sys_exists = sys.exists();
        let hoard_exists = hoard.exists();
        (sys.is_file() || !sys_exists)
            && (hoard.is_file() || !hoard_exists)
            && (sys_exists || hoard_exists)
    }

    /// Returns whether this item is a directory.
    ///
    /// This is `true` if:
    /// - At least one of `hoard_path` or `system_path` exists
    /// - All existing paths are directories
    #[must_use]
    pub fn is_dir(&self) -> bool {
        let sys = self.system_path();
        let hoard = self.hoard_path();
        let sys_exists = sys.exists();
        let hoard_exists = hoard.exists();
        (sys.is_dir() || !sys_exists)
            && (hoard.is_dir() || !hoard_exists)
            && (sys_exists || hoard_exists)
    }

    fn content(path: &Path) -> io::Result<Option<Vec<u8>>> {
        match fs::read(path) {
            Ok(content) => Ok(Some(content)),
            Err(err) => match err.kind() {
                ErrorKind::NotFound => Ok(None),
                _ => Err(err),
            },
        }
    }

    /// Returns the content, as bytes, of the system version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `system_path` is a directory.
    pub fn system_content(&self) -> io::Result<Option<Vec<u8>>> {
        Self::content(self.system_path())
    }

    /// Returns the content, as bytes, of the Hoard version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `hoard_path` is a directory.
    pub fn hoard_content(&self) -> io::Result<Option<Vec<u8>>> {
        Self::content(self.hoard_path())
    }

    /// Returns the requested [`ChecksumType`] for the Hoard version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `hoard_path` is a directory.
    ///
    /// If always calling this function with a constant or programmer-determined value,
    /// consider using [`hoard_md5`] or [`hoard_sha256`] instead.
    pub fn hoard_checksum(&self, typ: ChecksumType) -> io::Result<Option<Checksum>> {
        match typ {
            ChecksumType::MD5 => self.hoard_md5(),
            ChecksumType::SHA256 => self.hoard_sha256(),
        }
    }

    /// Returns the MD5 checksum for the Hoard version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `hoard_path` is a directory.
    pub fn hoard_md5(&self) -> io::Result<Option<Checksum>> {
        self.hoard_content()
            .map(|content| content.as_deref().map(Self::md5))
    }

    /// Returns the SHA256 checksum for the Hoard version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `hoard_path` is a directory.
    pub fn hoard_sha256(&self) -> io::Result<Option<Checksum>> {
        self.hoard_content()
            .map(|content| content.as_deref().map(Self::sha256))
    }

    /// Returns the requested [`ChecksumType`] for the system version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `system_path` is a directory.
    ///
    /// If always calling this function with a constant or programmer-determined value,
    /// consider using [`system_md5`] or [`system_sha256`] instead.
    pub fn system_checksum(&self, typ: ChecksumType) -> io::Result<Option<Checksum>> {
        match typ {
            ChecksumType::MD5 => self.system_md5(),
            ChecksumType::SHA256 => self.system_sha256(),
        }
    }

    /// Returns the MD5 checksum for the system version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `system_path` is a directory.
    pub fn system_md5(&self) -> io::Result<Option<Checksum>> {
        self.system_content()
            .map(|content| content.as_deref().map(Self::md5))
    }

    /// Returns the SHA256 checksum for the system version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `system_path` is a directory.
    pub fn system_sha256(&self) -> io::Result<Option<Checksum>> {
        self.system_content()
            .map(|content| content.as_deref().map(Self::sha256))
    }

    fn md5(content: &[u8]) -> Checksum {
        Checksum::MD5(MD5::from_data(content))
    }

    fn sha256(content: &[u8]) -> Checksum {
        Checksum::SHA256(SHA256::from_data(content))
    }
}
