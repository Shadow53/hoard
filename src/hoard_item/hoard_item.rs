use std::path::Path;
use tap::TapFallible;

use tokio::io;

use crate::checksum::{Checksum, ChecksumType, MD5, SHA256};
use crate::diff::FileContent;
use crate::newtypes::PileName;
use crate::paths::{HoardPath, RelativePath, SystemPath};

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
    #[tracing::instrument(name = "new_hoard_item")]
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

    async fn content(path: &Path) -> io::Result<FileContent> {
        FileContent::read_path(path).await.tap_err(
            crate::tap_log_error_msg(&format!("failed to read content from {}", path.display()))
        )
    }

    async fn raw_content(path: &Path) -> io::Result<Option<Vec<u8>>> {
        Ok(match Self::content(path).await? {
            FileContent::Missing => None,
            FileContent::Binary(data) => Some(data),
            FileContent::Text(s) => Some(s.into_bytes()),
        })
    }

    /// Returns the content, as bytes, of the system version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `system_path` is a directory.
    #[tracing::instrument(name = "hoard_item_system_content")]
    pub async fn system_content(&self) -> io::Result<FileContent> {
        Self::content(self.system_path()).await
    }

    /// Returns the content, as bytes, of the Hoard version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `hoard_path` is a directory.
    #[tracing::instrument(name = "hoard_item_hoard_content")]
    pub async fn hoard_content(&self) -> io::Result<FileContent> {
        Self::content(self.hoard_path()).await
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
    #[tracing::instrument(name = "hoard_item_hoard_checksum")]
    pub async fn hoard_checksum(&self, typ: ChecksumType) -> io::Result<Option<Checksum>> {
        match typ {
            ChecksumType::MD5 => self.hoard_md5().await,
            ChecksumType::SHA256 => self.hoard_sha256().await,
        }
    }

    /// Returns the MD5 checksum for the Hoard version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `hoard_path` is a directory.
    #[tracing::instrument(name = "hoard_item_hoard_md5")]
    pub async fn hoard_md5(&self) -> io::Result<Option<Checksum>> {
        Self::raw_content(self.hoard_path())
            .await
            .map(|content| content.as_deref().map(Self::md5))
    }

    /// Returns the SHA256 checksum for the Hoard version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `hoard_path` is a directory.
    #[tracing::instrument(name = "hoard_item_hoard_sha256")]
    pub async fn hoard_sha256(&self) -> io::Result<Option<Checksum>> {
        Self::raw_content(self.hoard_path())
            .await
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
    #[tracing::instrument(name = "hoard_item_system_checksum")]
    pub async fn system_checksum(&self, typ: ChecksumType) -> io::Result<Option<Checksum>> {
        match typ {
            ChecksumType::MD5 => self.system_md5().await,
            ChecksumType::SHA256 => self.system_sha256().await,
        }
    }

    /// Returns the MD5 checksum for the system version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `system_path` is a directory.
    #[tracing::instrument(name = "hoard_item_system_md5")]
    pub async fn system_md5(&self) -> io::Result<Option<Checksum>> {
        Self::raw_content(self.system_path())
            .await
            .map(|content| content.as_deref().map(Self::md5))
    }

    /// Returns the SHA256 checksum for the system version of the file.
    ///
    /// # Errors
    ///
    /// Returns `Ok(None)` if the file does not exist, and errors for all other
    /// error cases for [`std::fs::read`], including if `system_path` is a directory.
    #[tracing::instrument(name = "hoard_item_system_sha256")]
    pub async fn system_sha256(&self) -> io::Result<Option<Checksum>> {
        Self::raw_content(self.system_path())
            .await
            .map(|content| content.as_deref().map(Self::sha256))
    }

    fn md5(content: &[u8]) -> Checksum {
        Checksum::MD5(MD5::from_data(content))
    }

    fn sha256(content: &[u8]) -> Checksum {
        Checksum::SHA256(SHA256::from_data(content))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::test::Tester;

    use super::*;

    const CONTENT_A: &str = "content A";
    const MD5_CONTENT_A: &str = "4f4e99c2da696a47de3b455758bff316";
    const SHA256_CONTENT_A: &str =
        "49114a9a2b7d46ec27be62ae3eade12f78d46cf5a99c52cd4f80381d723eed6e";
    const FILE_NAME: &str = "hoard_item.txt";

    fn hoard_item(tester: &Tester) -> HoardItem {
        HoardItem::new(
            PileName::anonymous(),
            HoardPath::try_from(tester.data_dir().to_path_buf()).unwrap(),
            SystemPath::try_from(tester.config_dir().to_path_buf()).unwrap(),
            RelativePath::try_from(PathBuf::from(FILE_NAME)).unwrap(),
        )
    }

    #[tokio::test]
    async fn test_system_methods() {
        let tester = Tester::new().unwrap();
        let item = hoard_item(&tester);

        assert_eq!(item.system_content().await.unwrap(), FileContent::Missing);
        assert_eq!(item.system_md5().await.unwrap(), None);
        assert_eq!(item.system_checksum(ChecksumType::MD5).await.unwrap(), None);
        assert_eq!(item.system_sha256().await.unwrap(), None);
        assert_eq!(
            item.system_checksum(ChecksumType::SHA256).await.unwrap(),
            None
        );

        fs::write(item.system_path(), CONTENT_A).unwrap();

        assert_eq!(
            item.system_content().await.unwrap(),
            FileContent::Text(CONTENT_A.to_string())
        );
        assert_eq!(
            item.system_md5().await.unwrap(),
            Some(Checksum::MD5(MD5_CONTENT_A.parse::<MD5>().unwrap()))
        );
        assert_eq!(
            item.system_checksum(ChecksumType::MD5).await.unwrap(),
            Some(Checksum::MD5(MD5_CONTENT_A.parse::<MD5>().unwrap()))
        );
        assert_eq!(
            item.system_sha256().await.unwrap(),
            Some(Checksum::SHA256(
                SHA256_CONTENT_A.parse::<SHA256>().unwrap()
            ))
        );
        assert_eq!(
            item.system_checksum(ChecksumType::SHA256).await.unwrap(),
            Some(Checksum::SHA256(
                SHA256_CONTENT_A.parse::<SHA256>().unwrap()
            ))
        );
    }

    #[tokio::test]
    async fn test_hoard_methods() {
        let tester = Tester::new().unwrap();
        let item = hoard_item(&tester);

        assert_eq!(item.hoard_content().await.unwrap(), FileContent::Missing);
        assert_eq!(item.hoard_md5().await.unwrap(), None);
        assert_eq!(item.hoard_checksum(ChecksumType::MD5).await.unwrap(), None);
        assert_eq!(item.hoard_sha256().await.unwrap(), None);
        assert_eq!(
            item.hoard_checksum(ChecksumType::SHA256).await.unwrap(),
            None
        );

        fs::write(item.hoard_path(), CONTENT_A).unwrap();

        assert_eq!(
            item.hoard_content().await.unwrap(),
            FileContent::Text(CONTENT_A.to_string())
        );
        assert_eq!(
            item.hoard_md5().await.unwrap(),
            Some(Checksum::MD5(MD5_CONTENT_A.parse::<MD5>().unwrap()))
        );
        assert_eq!(
            item.hoard_checksum(ChecksumType::MD5).await.unwrap(),
            Some(Checksum::MD5(MD5_CONTENT_A.parse::<MD5>().unwrap()))
        );
        assert_eq!(
            item.hoard_sha256().await.unwrap(),
            Some(Checksum::SHA256(
                SHA256_CONTENT_A.parse::<SHA256>().unwrap()
            ))
        );
        assert_eq!(
            item.hoard_checksum(ChecksumType::SHA256).await.unwrap(),
            Some(Checksum::SHA256(
                SHA256_CONTENT_A.parse::<SHA256>().unwrap()
            ))
        );
    }

    mod is_file_is_dir {
        use super::*;

        macro_rules! test_is_file_is_dir {
            (name: $name:ident, system: $system_is_file:expr, hoard: $hoard_is_file:expr, expect_file: $expected_file:literal, expect_dir: $expected_dir:literal) => {
                #[test]
                #[allow(clippy::bool_assert_comparison)]
                fn $name() {
                    let tester = Tester::new().unwrap();
                    let item = hoard_item(&tester);

                    match $system_is_file {
                        Some(true) => fs::write(item.system_path(), CONTENT_A).unwrap(),
                        Some(false) => fs::create_dir_all(item.system_path()).unwrap(),
                        None => {}
                    }

                    match $hoard_is_file {
                        Some(true) => fs::write(item.hoard_path(), CONTENT_A).unwrap(),
                        Some(false) => fs::create_dir_all(item.hoard_path()).unwrap(),
                        None => {}
                    }

                    assert_eq!($expected_file, item.is_file());
                    assert_eq!($expected_dir, item.is_dir());
                }
            };
        }

        test_is_file_is_dir! {
            name: test_neither_exists,
            system: None,
            hoard: None,
            expect_file: false,
            expect_dir: false
        }

        test_is_file_is_dir! {
            name: test_system_is_file,
            system: Some(true),
            hoard: None,
            expect_file: true,
            expect_dir: false
        }

        test_is_file_is_dir! {
            name: test_system_is_dir,
            system: Some(false),
            hoard: None,
            expect_file: false,
            expect_dir: true
        }

        test_is_file_is_dir! {
            name: test_hoard_is_file,
            system: None,
            hoard: Some(true),
            expect_file: true,
            expect_dir: false
        }

        test_is_file_is_dir! {
            name: test_hoard_is_dir,
            system: None,
            hoard: Some(false),
            expect_file: false,
            expect_dir: true
        }

        test_is_file_is_dir! {
            name: test_both_are_file,
            system: Some(true),
            hoard: Some(true),
            expect_file: true,
            expect_dir: false
        }

        test_is_file_is_dir! {
            name: test_both_are_dir,
            system: Some(false),
            hoard: Some(false),
            expect_file: false,
            expect_dir: true
        }

        test_is_file_is_dir! {
            name: test_system_is_file_hoard_is_dir,
            system: Some(true),
            hoard: Some(false),
            expect_file: false,
            expect_dir: false
        }

        test_is_file_is_dir! {
            name: test_system_is_dir_hoard_is_file,
            system: Some(false),
            hoard: Some(true),
            expect_file: false,
            expect_dir: false
        }
    }
}
