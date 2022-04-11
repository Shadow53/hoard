use std::collections::BTreeMap;
use std::io;
use crate::checksum::{Checksum, ChecksumType, MD5, SHA256};
use crate::diff::FileContent;
use crate::newtypes::PileName;
use crate::paths::{HoardPath, RelativePath, SystemPath};
use super::hoard_item::HoardItem;

/// Wrapper around [`HoardItem`] that accesses the filesystem at creation time and
/// caches file data.
///
/// # Usage
///
/// This does nothing to ensure that files are not modified during its lifetime. For directly
/// interacting with files on the filesystem, [`HoardItem`] may be better.
///
/// This struct is useful for prolonged processing of a given file.
///
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[allow(clippy::module_name_repetitions)]
pub struct CachedHoardItem {
    inner: HoardItem,
    hoard_content: Option<FileContent>,
    system_content: Option<FileContent>,
    hoard_checksums: Option<BTreeMap<ChecksumType, Checksum>>,
    system_checksums: Option<BTreeMap<ChecksumType, Checksum>>,
    is_file: bool,
    is_dir: bool,
}

impl From<CachedHoardItem> for HoardItem {
    fn from(cached: CachedHoardItem) -> Self {
        cached.inner
    }
}

impl CachedHoardItem {
    /// Create a new `CachedHoardItem`.
    ///
    /// See [`HoardItem::new`] for more about usage.
    ///
    /// # Errors
    ///
    /// Will return I/O errors if they occur while processing file data, with the exception of
    /// `NotFound` errors, which are translated into `None` values, as applicable.
    pub fn new(
        pile_name: PileName,
        hoard_prefix: HoardPath,
        system_prefix: SystemPath,
        relative_path: RelativePath,
    ) -> io::Result<Self> {
        let inner = HoardItem::new(pile_name, hoard_prefix, system_prefix, relative_path);

        let (is_file, is_dir) = {
            let system_exists = inner.system_path().exists();
            let hoard_exists = inner.hoard_path().exists();

            let is_file = (inner.system_path().is_file() || !system_exists) &&
                (inner.hoard_path().is_file() || !hoard_exists) &&
                (system_exists || hoard_exists);

            let is_dir = (inner.system_path().is_dir() || !system_exists) &&
                    (inner.hoard_path().is_dir() || !hoard_exists) &&
                    (system_exists || hoard_exists);

            (is_file, is_dir)
        };

        let (system_content, hoard_content) = if is_file {
            let system_content = inner.system_content()?;
            let hoard_content = inner.hoard_content()?;
            (Some(system_content), Some(hoard_content))
        } else {
            (None, None)
        };

        let system_checksums = system_content.as_ref().and_then(Self::checksums);
        let hoard_checksums = hoard_content.as_ref().and_then(Self::checksums);

        Ok(Self { inner, hoard_content, system_content, hoard_checksums, system_checksums, is_file, is_dir })
    }

    /// Returns the name of the pile this item belongs to, if any.
    #[must_use]
    pub fn pile_name(&self) -> &PileName {
        self.inner.pile_name()
    }

    /// Returns the relative path for this item.
    #[must_use]
    pub fn relative_path(&self) -> &RelativePath {
        self.inner.relative_path()
    }

    /// Returns the hoard-controlled path for this item's pile.
    #[must_use]
    pub fn hoard_prefix(&self) -> &HoardPath {
        self.inner.hoard_prefix()
    }

    /// Returns the system path for this item's pile.
    #[must_use]
    pub fn system_prefix(&self) -> &SystemPath {
        self.inner.system_prefix()
    }

    /// Returns the Hoard-controlled path for this item.
    ///
    /// If [`HoardItem::relative_path()`] is `None`, this is the same as
    /// [`HoardItem::hoard_prefix()`].
    #[must_use]
    pub fn hoard_path(&self) -> &HoardPath {
        self.inner.hoard_path()
    }

    /// Returns the system path for this item.
    ///
    /// If [`HoardItem::relative_path()`] is `None`, this is the same as
    /// [`HoardItem::system_prefix()`].
    #[must_use]
    pub fn system_path(&self) -> &SystemPath {
        self.inner.system_path()
    }

    /// Returns whether this item is a file.
    ///
    /// This is `true` if:
    /// - At least one of `hoard_path` or `system_path` exists
    /// - All existing paths are a file
    #[must_use]
    pub fn is_file(&self) -> bool {
        self.is_file
    }

    /// Returns whether this item is a directory.
    ///
    /// This is `true` if:
    /// - At least one of `hoard_path` or `system_path` exists
    /// - All existing paths are directories
    #[must_use]
    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    /// Returns whether this file contains text.
    ///
    /// This is `true` if at least one file (system/hoard) exists and all files that exist contain
    /// text.
    #[must_use]
    pub fn is_text(&self) -> bool {
        self.is_file() && matches!(
            (self.system_content.as_ref(), self.hoard_content.as_ref()),
            (Some(FileContent::Text(_) | FileContent::Missing), Some(FileContent::Text(_) | FileContent::Missing))
        )
    }

    /// Returns whether this file contains text.
    ///
    /// This is `true` if at least one file (system/hoard) exists and is not text.
    #[must_use]
    pub fn is_binary(&self) -> bool {
        self.is_file() && matches!(
            (self.system_content.as_ref(), self.hoard_content.as_ref()),
            (Some(FileContent::Binary(_)), Some(_)) | (Some(_), Some(FileContent::Binary(_)))
        )
    }

    /// Returns the content, as bytes, of the system version of the file.
    ///
    /// Returns [`Some(FileContent::Missing)`] if [`Self::hoard_path`] is a file and [`Self::system_path`]
    /// does not exist.
    ///
    /// Returns `None` if [`Self::system_path`] is not a file.
    #[must_use]
    pub fn system_content(&self) -> Option<&FileContent> {
        self.system_content.as_ref()
    }

    /// Returns the content, as bytes, of the Hoard version of the file.
    ///
    /// Returns [`Some(FileContent::Missing)`] if [`Self::system_path`] is a file and [`Self::hoard_path`]
    /// does not exist.
    ///
    /// Returns `None` if [`Self::hoard_path`] is not a file.
    #[must_use]
    pub fn hoard_content(&self) -> Option<&FileContent> {
        self.hoard_content.as_ref()
    }

    fn checksums(content: &FileContent) -> Option<BTreeMap<ChecksumType, Checksum>> {
        match content {
            FileContent::Missing => None,
            FileContent::Text(s) => {
                let mut map = BTreeMap::new();
                map.insert(ChecksumType::MD5, Checksum::MD5(MD5::from_data(s.as_bytes())));
                map.insert(ChecksumType::SHA256, Checksum::SHA256(SHA256::from_data(s.as_bytes())));
                Some(map)
            },
            FileContent::Binary(data) => {
                let mut map = BTreeMap::new();
                map.insert(ChecksumType::MD5, Checksum::MD5(MD5::from_data(data.as_slice())));
                map.insert(ChecksumType::SHA256, Checksum::SHA256(SHA256::from_data(data.as_slice())));
                Some(map)
            },
        }
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
    #[must_use]
    pub fn hoard_checksum(&self, typ: ChecksumType) -> Option<Checksum> {
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
    #[must_use]
    pub fn hoard_md5(&self) -> Option<Checksum> {
        self.hoard_checksums.as_ref()
            .and_then(|map| map.get(&ChecksumType::MD5).cloned())
    }

    /// Returns the SHA256 checksum for the Hoard version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `hoard_path` is a directory.
    #[must_use]
    pub fn hoard_sha256(&self) -> Option<Checksum> {
        self.hoard_checksums.as_ref()
            .and_then(|map| map.get(&ChecksumType::SHA256).cloned())
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
    #[must_use]
    pub fn system_checksum(&self, typ: ChecksumType) -> Option<Checksum> {
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
    #[must_use]
    pub fn system_md5(&self) -> Option<Checksum> {
        self.system_checksums.as_ref()
            .and_then(|map| map.get(&ChecksumType::MD5).cloned())
    }

    /// Returns the SHA256 checksum for the system version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `system_path` is a directory.
    #[must_use]
    pub fn system_sha256(&self) -> Option<Checksum> {
        self.system_checksums.as_ref()
            .and_then(|map| map.get(&ChecksumType::SHA256).cloned())
    }
}
