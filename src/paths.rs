//! Contains wrapper types for [`PathBuf`] to help ensure correct logic.
//!
//! - [`HoardPath`]
//! - [`SystemPath`]
//! - [`RelativePath`]

use std::fmt;
use std::fmt::Formatter;
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

use crate::newtypes::{HoardName, NonEmptyPileName, PileName};

/// Returns the default root for hoard files.
#[must_use]
#[tracing::instrument(level = "trace")]
pub fn hoards_dir() -> HoardPath {
    HoardPath::try_from(crate::dirs::data_dir().join("hoards"))
        .expect("HoardPath that is the hoards directory should always be valid")
}

/// Normalize a path, removing things like `.` and `..`.
///
/// CAUTION: This does not resolve symlinks (unlike
/// [`std::fs::canonicalize`]). This may cause incorrect or surprising
/// behavior at times. This should be used carefully. Unfortunately,
/// [`std::fs::canonicalize`] can be hard to use correctly, since it can often
/// fail, or on Windows returns annoying device paths. This is a problem Cargo
/// needs to improve on.
///
/// Copied from the `cargo-util` source code, with doc comment, on 2022-03-08.
#[must_use]
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut components = path.components().peekable();
    let mut ret = if let Some(c @ Component::Prefix(..)) = components.peek().copied() {
        components.next();
        PathBuf::from(c.as_os_str())
    } else {
        PathBuf::new()
    };

    for component in components {
        match component {
            Component::Prefix(..) => unreachable!(),
            Component::RootDir => {
                ret.push(component.as_os_str());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(ret.components().last(), Some(Component::Normal(_))) {
                    ret.pop();
                } else {
                    ret.push(Component::ParentDir);
                }
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    ret
}

fn is_valid_absolute(path: &Path) -> bool {
    path.is_absolute()
        && path
            .components()
            .all(|comp| !matches!(comp, Component::ParentDir))
}

/// Errors that may be returned when using [`TryFrom`] on a path wrapper.
///
/// The [`PathBuf`] contained in the error is a normalized version of the one provided to [`TryFrom`].
/// That is:
///
/// - All non-essential `.` and `..` are removed
/// - No symbolic links are followed
/// - No file or directory existence is checked
/// - No other filesystem accesses are performed
#[derive(Debug, Error, PartialEq)]
pub enum Error {
    /// The path provided to [`HoardPath::try_from()`] was invalid.
    ///
    /// The path is invalid if it is not the hoards root (see [`hoard_dir`]) or a child of it.
    #[error("invalid HoardPath: {0:?}")]
    InvalidHoardPath(PathBuf),
    /// The path provided to [`SystemPath::try_from()`] was invalid.
    ///
    /// The path is invalid if it is is a *valid* [`HoardPath`].
    #[error("invalid SystemPath: {0:?}")]
    InvalidSystemPath(PathBuf),
    /// The path provided to [`RelativePath::try_from()`] was invalid.
    ///
    /// This may be because of one of two reasons:
    /// 1. The path is not relative (i.e. is absolute)
    /// 2. The path escapes its parent, e.g.
    ///   - `../`
    ///   - `child/../other_child/../..`
    ///   - Any other path that, when normalized, would have a prefix of `../`
    #[error("invalid relative path: {0:?}")]
    InvalidRelativePath(PathBuf),
}

/// A wrapper for [`PathBuf`] indicating a path within the Hoard.
///
/// That is, any file -- or directory containing files -- that is something backed up to the Hoard
/// or a metadata file stored alongside the Hoard (e.g. history logs for last paths/operations
/// checks). In short, anything stored within the Hoard data directory.
///
/// This does *not* include items in the config directory, including the configuration file.
/// This makes it so even the config file can be managed by Hoard.
#[repr(transparent)]
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct HoardPath(PathBuf);

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

impl TryFrom<PathBuf> for HoardPath {
    type Error = Error;

    #[tracing::instrument(level = "trace", name = "hoard_path_try_from_path")]
    fn try_from(input: PathBuf) -> Result<Self, Self::Error> {
        let value = normalize_path(&input);
        if !is_valid_absolute(&value) {
            return Err(Error::InvalidHoardPath(input));
        }

        let hoard_root = crate::dirs::data_dir();
        if value == hoard_root || value.strip_prefix(&hoard_root).is_ok() {
            Ok(Self(value))
        } else {
            Err(Error::InvalidHoardPath(value))
        }
    }
}

impl FromStr for HoardPath {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(PathBuf::from(s))
    }
}

impl HoardPath {
    /// Joins this [`HoardPath`] with the given [`RelativePath`].
    ///
    /// This function does not validate whether any paths exist or whether they are files or
    /// directories. That is, it is possible to take a [`HoardPath`] representing a file and
    /// use it as the prefix directory for a [`RelativePath`]. It is up to the caller to make
    /// sure the resulting path is valid for use.
    #[must_use]
    pub fn join(&self, rhs: &RelativePath) -> Self {
        Self::try_from(
            rhs.0
                .as_ref()
                .map_or_else(|| self.0.clone(), |rel_path| self.0.join(&rel_path)),
        )
        .expect("a HoardPath rooted in an existing HoardPath is always valid")
    }
}

/// A wrapper for [`PathBuf`] indicating a path that is not a [`HoardPath`].
///
/// That is, any file that is not stored in the Hoard data directory. See [`HoardPath`] for more.
#[repr(transparent)]
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SystemPath(PathBuf);

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

impl TryFrom<PathBuf> for SystemPath {
    type Error = Error;

    #[tracing::instrument(level = "trace", name = "system_path_try_from_path")]
    fn try_from(input: PathBuf) -> Result<Self, Self::Error> {
        let value = normalize_path(&input);
        if !is_valid_absolute(&value) {
            return Err(Error::InvalidSystemPath(input));
        }

        let hoard_root = crate::dirs::data_dir();
        if value == hoard_root || value.strip_prefix(&hoard_root).is_ok() {
            Err(Error::InvalidSystemPath(value))
        } else {
            Ok(Self(value))
        }
    }
}

impl SystemPath {
    /// Joins this [`SystemPath`] with the given [`RelativePath`].
    ///
    /// This function does not validate whether any paths exist or whether they are files or
    /// directories. That is, it is possible to take a [`SystemPath`] representing a file and
    /// use it as the prefix directory for a [`RelativePath`]. It is up to the caller to make
    /// sure the resulting path is valid for use.
    #[must_use]
    pub fn join(&self, rhs: &RelativePath) -> Self {
        Self::try_from(
            rhs.0
                .as_ref()
                .map_or_else(|| self.0.clone(), |rel_path| self.0.join(&rel_path)),
        )
        .expect("a SystemPath rooted in an existing SystemPath is always valid")
    }
}

/// A wrapper for [`PathBuf`] that represents a relative path that is a child of the
/// "current" directory.
///
/// To construct an instance of `RelativePath`, use [`RelativePath::try_from()`].
/// This will perform the necessary checks to ensure that the path:
///
/// - Is a relative path (i.e. not absolute)
/// - Does not escape the "parent" (i.e., the normalized version of the path does not
///   start with `../`)
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct RelativePath(Option<PathBuf>);

impl Serialize for RelativePath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0
            .clone()
            .unwrap_or_else(PathBuf::new)
            .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for RelativePath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path = PathBuf::deserialize(deserializer)?;
        Self::try_from(path).map_err(D::Error::custom)
    }
}

impl TryFrom<PathBuf> for RelativePath {
    type Error = Error;

    #[tracing::instrument(level = "trace", name = "relative_path_try_from_path")]
    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        let normalized = normalize_path(&value);

        if normalized.to_str() == Some("") {
            Ok(Self(None))
        } else if normalized.is_relative()
            && normalized.components().next() != Some(Component::ParentDir)
        {
            Ok(Self(Some(value)))
        } else {
            Err(Error::InvalidRelativePath(value))
        }
    }
}

impl TryFrom<Option<PathBuf>> for RelativePath {
    type Error = Error;

    #[tracing::instrument(level = "trace", name = "relative_path_try_from_path_option")]
    fn try_from(value: Option<PathBuf>) -> Result<Self, Self::Error> {
        match value {
            None => Ok(Self(None)),
            Some(path) => Self::try_from(path),
        }
    }
}

impl AsRef<Option<PathBuf>> for RelativePath {
    fn as_ref(&self) -> &Option<PathBuf> {
        &self.0
    }
}

impl fmt::Display for RelativePath {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.0 {
            None => write!(f, ""),
            Some(path) => write!(f, "{}", path.display()),
        }
    }
}

impl From<&HoardName> for RelativePath {
    fn from(name: &HoardName) -> Self {
        let path = &**name;
        RelativePath(Some(PathBuf::from(path)))
    }
}

impl From<&PileName> for RelativePath {
    fn from(name: &PileName) -> Self {
        RelativePath(name.as_deref().map(PathBuf::from))
    }
}

impl From<&NonEmptyPileName> for RelativePath {
    fn from(name: &NonEmptyPileName) -> Self {
        RelativePath(Some(PathBuf::from(name.as_ref())))
    }
}

impl RelativePath {
    #[must_use]
    /// Clones a [`PathBuf`] from this [`RelativePath`].
    pub fn to_path_buf(&self) -> PathBuf {
        match &self.0 {
            None => PathBuf::new(),
            Some(path) => path.clone(),
        }
    }

    /// Returns the contained path, if one exists.
    ///
    /// To get a path regardless of the contents of `self`, use [`RelativePath::to_path_buf`].
    #[must_use]
    pub fn as_path(&self) -> Option<&Path> {
        self.0.as_deref()
    }

    #[must_use]
    /// Returns an instance of [`RelativePath`] that represents no change.
    ///
    /// That is, `some_hoard_path.join(relative_path) == some_hoard_path`.
    pub fn none() -> Self {
        Self(None)
    }

    /// Returns the parent [`RelativePath`] to this one.
    ///
    /// If this `RelativePath` has one or fewer components to it, an empty `RelativePath` is returned.
    #[must_use]
    pub fn parent(&self) -> RelativePath {
        RelativePath(
            self.0
                .as_deref()
                .and_then(Path::parent)
                .map(Path::to_path_buf),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod hoard_path {
        use super::*;

        #[test]
        fn test_try_from_invalid() {
            let path = PathBuf::from("/invalid/hoard/path");
            let error = HoardPath::try_from(path.clone()).expect_err("invalid hoard path");
            assert_eq!(error, Error::InvalidHoardPath(path));
        }

        #[test]
        fn test_from_str_invalid() {
            let path_str = "/invalid/hoard/path";
            let path = PathBuf::from(&path_str);
            let error = HoardPath::from_str(path_str).expect_err("invalid hoard path string");
            assert_eq!(error, Error::InvalidHoardPath(path));
        }

        #[test]
        fn test_from_str_valid() {
            let valid_path =
                hoards_dir().join(&RelativePath::try_from(PathBuf::from("valid")).unwrap());
            let valid_str = valid_path.as_ref().to_str().unwrap();
            let path = HoardPath::from_str(valid_str).unwrap();
            assert_eq!(path, valid_path);
        }
    }

    mod system_path {
        use super::*;

        #[test]
        fn test_try_from_hoard_path() {
            let hoard_path = hoards_dir().as_ref().join("test");
            let error = SystemPath::try_from(hoard_path.clone())
                .expect_err("a hoard path cannot be a system path");
            assert_eq!(error, Error::InvalidSystemPath(hoard_path));
        }
    }

    mod relative_path {
        use super::*;

        #[test]
        fn test_absolute_paths_not_allowed() {
            #[cfg(unix)]
            let bin_path = PathBuf::from("/bin/sh");
            #[cfg(windows)]
            let bin_path = PathBuf::from("C:\\Windows\\System32\\cmd.exe");

            let home_path = crate::dirs::home_dir().join("file.txt");

            if let Err(Error::InvalidRelativePath(path)) = RelativePath::try_from(bin_path.clone())
            {
                assert_eq!(path, bin_path);
            } else {
                panic!(
                    "absolute path {} should not have been allowed",
                    bin_path.display()
                );
            }

            if let Err(Error::InvalidRelativePath(path)) = RelativePath::try_from(home_path.clone())
            {
                assert_eq!(path, home_path);
            } else {
                panic!(
                    "absolute path {} should not have been allowed",
                    home_path.display()
                );
            }
        }

        #[test]
        fn test_paths_resolving_to_curdir_stored_as_none() {
            let test_paths = [
                PathBuf::new(),
                PathBuf::from("./"),
                PathBuf::from("./child/.."),
                PathBuf::from("child/grandchild/../other_grand/../.."),
            ];

            for path in test_paths {
                let rel_path = RelativePath::try_from(path).expect("relative path should be valid");
                assert_eq!(
                    rel_path,
                    RelativePath(None),
                    "expected RelativePath(None), got {:?}",
                    rel_path
                );
            }
        }

        #[test]
        fn test_relative_paths_allowed() {
            let test_paths = [
                PathBuf::from("./child"),
                PathBuf::from("./child/grandchild/and/some/more"),
                PathBuf::from("no/leading/dot"),
                PathBuf::from("contains/parent/../dots"),
            ];

            for path in test_paths {
                RelativePath::try_from(path).expect("path should be a valid relative path");
            }
        }

        #[test]
        fn test_relative_paths_accessing_grandparent_not_allowed() {
            let test_paths = [
                PathBuf::from("../"),
                PathBuf::from("../child"),
                PathBuf::from("child/../.."),
                PathBuf::from("child/../other_child/../.."),
            ];

            for path in test_paths {
                RelativePath::try_from(path)
                    .expect_err("paths that access the grandparent are not valid");
            }
        }

        #[test]
        fn test_try_from_option() {
            assert_eq!(RelativePath::try_from(None).unwrap(), RelativePath(None));
            let valid_path = PathBuf::from("valid/relative");
            assert_eq!(
                RelativePath::try_from(Some(valid_path.clone())).unwrap(),
                RelativePath(Some(valid_path))
            );
            #[cfg(unix)]
            let invalid_path = PathBuf::from("/invalid/path");
            #[cfg(windows)]
            let invalid_path = PathBuf::from("C:\\\\invalid\\path");

            assert!(
                invalid_path.is_absolute(),
                "invalid path must be absolute for this test."
            );
            let error = RelativePath::try_from(Some(invalid_path.clone()))
                .expect_err("absolute path should error");
            assert_eq!(error, Error::InvalidRelativePath(invalid_path));
        }

        #[test]
        fn test_as_ref() {
            assert_eq!(RelativePath::none().as_ref(), &None);
            assert_eq!(
                RelativePath::try_from(PathBuf::from("valid"))
                    .unwrap()
                    .as_ref(),
                &Some(PathBuf::from("valid"))
            );
        }

        #[test]
        fn test_to_string() {
            assert_eq!(RelativePath::none().to_string(), "");
            assert_eq!(
                RelativePath::try_from(PathBuf::from("valid"))
                    .unwrap()
                    .to_string(),
                "valid"
            );
        }

        #[test]
        fn test_from_pile_name() {
            let pile_name = PileName::from_str("valid").unwrap();
            let rel_path = RelativePath::from(&pile_name);
            assert_eq!(
                rel_path,
                RelativePath::try_from(PathBuf::from("valid")).unwrap()
            );
        }

        #[test]
        fn test_as_path() {
            let path = PathBuf::from("test");
            let rel_path = RelativePath::try_from(path.clone()).unwrap();
            assert_eq!(rel_path.as_path(), Some(path.as_path()));
            assert_eq!(RelativePath::none().as_path(), None);
        }
    }
}
