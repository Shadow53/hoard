use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::ops::Deref;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use thiserror::Error;

fn inner_hoards_dir() -> PathBuf {
    crate::dirs::data_dir()
        .join("hoards")
}

/// Returns the default root for hoard files.
#[must_use]
pub fn hoards_dir() -> HoardPath {
    HoardPath::try_from(inner_hoards_dir())
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
                if matches!(ret.components().last(), None | Some(Component::ParentDir)) {
                    ret.push(Component::ParentDir);
                } else {
                    ret.pop();
                }
            }
            Component::Normal(c) => {
                ret.push(c);
            }
        }
    }
    ret
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
#[derive(Debug, Error)]
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

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        let hoard_root = inner_hoards_dir();
        if value.strip_prefix(&hoard_root).is_ok() {
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

    fn try_from(value: PathBuf) -> Result<Self, Self::Error> {
        let hoard_root = hoards_dir();
        match value.strip_prefix(hoard_root.as_ref()) {
            Ok(_) => Err(Error::InvalidSystemPath(value)),
            Err(_) => Ok(Self(value)),
        }
    }
}

impl SystemPath {
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
        if path.to_str() == Some("") {
            Ok(Self(None))
        } else {
            Ok(Self(Some(path)))
        }
    }
}

impl TryFrom<PathBuf> for RelativePath {
    type Error = Error;

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

impl Deref for RelativePath {
    type Target = Option<PathBuf>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl RelativePath {
    #[must_use]
    pub fn to_path_buf(&self) -> PathBuf {
        match &self.0 {
            None => PathBuf::new(),
            Some(path) => path.clone(),
        }
    }

    #[must_use]
    pub fn none() -> Self {
        Self(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod relative_path {
        use super::*;

        #[test]
        fn test_absolute_paths_not_allowed() {
            #[cfg(unix)]
            let bin_path = PathBuf::from("/bin/sh");
            #[cfg(windows)]
            let bin_path = PathBuf::from("C:\\Windows\\System32\\cmd.exe");

            let home_path = directories::UserDirs::new()
                .expect("should be able to find user dirs")
                .home_dir()
                .join("file.txt");

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
    }
}
