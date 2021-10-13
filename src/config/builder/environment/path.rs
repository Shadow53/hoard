//! See [`PathExists`].

use crate::env_vars::expand_env_in_path;
use serde::{Deserialize, Serialize, de};
use std::convert::{Infallible, TryInto};
use std::fmt;
use std::fmt::Formatter;
use std::path::PathBuf;

struct PathExistsVisitor;

impl de::Visitor<'_> for PathExistsVisitor {
    type Value = PathExists;

    fn expecting(&self, formatter: &mut Formatter) -> fmt::Result {
        formatter.write_str("a path that may or may not contain environment variables")
    }

    fn visit_str<E>(self, s: &str) -> Result<Self::Value, E> where E: de::Error {
        expand_env_in_path(s)
            .map(PathExists)
            .map_err(de::Error::custom)
    }
}

/// A conditional structure that tests whether or not the contained path exists.
///
/// The path can be anything from a file, directory, symbolic link, or otherwise, so long as
/// *something* with that name exists.
#[derive(Clone, PartialEq, Debug, Hash, Serialize)]
#[serde(transparent)]
#[repr(transparent)]
#[allow(clippy::module_name_repetitions)]
pub struct PathExists(pub PathBuf);

impl<'de> Deserialize<'de> for PathExists {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de> {
        deserializer.deserialize_str(PathExistsVisitor)
    }
}

impl TryInto<bool> for PathExists {
    type Error = Infallible;

    fn try_into(self) -> Result<bool, Self::Error> {
        let PathExists(path) = self;
        tracing::trace!("checking if path \"{}\" exists", path.to_string_lossy());
        Ok(path.exists())
    }
}

impl fmt::Display for PathExists {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let PathExists(path) = self;
        write!(f, "PATH EXISTS {}", path.to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::{tempdir, NamedTempFile};
    use serde_test::{Token, assert_tokens, assert_de_tokens};

    #[test]
    fn test_file_does_exist() {
        let temp = NamedTempFile::new().expect("failed to create temporary file");
        let exists: bool = PathExists(temp.path().to_path_buf())
            .try_into()
            .expect("failed to check if path exists");
        assert!(exists);
    }

    #[test]
    fn test_dir_does_exist() {
        let temp = tempdir().expect("failed to create temporary directory");
        let exists: bool = PathExists(temp.path().to_path_buf())
            .try_into()
            .expect("failed to check if path exists");
        assert!(exists);
    }

    #[test]
    fn test_file_does_not_exist() {
        let temp = NamedTempFile::new().expect("failed to create temporary file");
        fs::remove_file(temp.path()).expect("failed to remove temporary file");
        let exists: bool = PathExists(temp.path().to_path_buf())
            .try_into()
            .expect("failed to check if path exists");
        assert!(!exists);
    }

    #[test]
    fn test_dir_does_not_exist() {
        let temp = tempdir().expect("failed to create temporary directory");
        fs::remove_dir(temp.path()).expect("failed to remove temporary directory");
        let exists: bool = PathExists(temp.path().to_path_buf())
            .try_into()
            .expect("failed to check if path exists");
        assert!(!exists);
    }

    #[test]
    fn test_custom_deserialize() {
        let path_str = "/test/path/example";
        let path = PathExists(PathBuf::from(path_str));
        assert_tokens(&path, &[
            Token::Str(path_str)
        ]);
    }

    #[test]
    #[serial_test::serial]
    fn test_env_is_expanded_in_path() {
        std::env::set_var("HOARD_TEST_ENV", "hoard-test");
        let path_with_env = "/test/path/${HOARD_TEST_ENV}/leaf";
        let path_resolved = "/test/path/hoard-test/leaf";
        let path = PathExists(PathBuf::from(path_resolved));
        assert_de_tokens(&path, &[
            Token::Str(path_with_env)
        ]);
    }
}
