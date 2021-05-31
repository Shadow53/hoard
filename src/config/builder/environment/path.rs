use serde::{Deserialize, Serialize};
use std::convert::{Infallible, TryInto};
use std::fmt;
use std::fmt::Formatter;
use std::path::PathBuf;

#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Hash)]
#[serde(transparent)]
pub struct PathExists(pub PathBuf);

impl TryInto<bool> for PathExists {
    type Error = Infallible;

    fn try_into(self) -> Result<bool, Self::Error> {
        let PathExists(path) = self;
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
}
