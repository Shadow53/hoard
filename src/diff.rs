//! Utilities for checking whether files differ.
//!
//! Unified diffs are optionally available for text files. Following Git's example,
//! non-text binary files can only be detected as differing or the same.
use std::io::Read;
use std::path::Path;
use std::{fs, io};

use crate::paths::{HoardPath, SystemPath};
use similar::{ChangeTag, TextDiff};

const CONTEXT_RADIUS: usize = 5;

/// Represents the existing file content for a given file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FileContent {
    Text(String),
    Binary(Vec<u8>),
    Missing,
}

impl FileContent {
    fn read(file: fs::File) -> io::Result<Self> {
        let bytes: Vec<u8> = file.bytes().collect::<io::Result<_>>()?;
        match String::from_utf8(bytes) {
            Ok(s) => Ok(Self::Text(s)),
            Err(err) => Ok(Self::Binary(err.into_bytes())),
        }
    }

    fn into_bytes(self) -> Option<Vec<u8>> {
        match self {
            Self::Text(s) => Some(s.into_bytes()),
            Self::Binary(v) => Some(v),
            Self::Missing => None,
        }
    }
}

#[allow(variant_size_differences)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Diff {
    /// Text content differs. Contains the generated unified diff.
    Text(String),
    /// Binary content differs. Also occurs if a file changes between text and binary formats.
    Binary,
    /// The left path to diff_files did not exist, but the right path did.
    HoardNotExists,
    /// The left path to diff_paths existed, but the right path did not.
    SystemNotExists,
}

fn content_for(path: &Path) -> io::Result<FileContent> {
    match fs::File::open(path) {
        Ok(file) => {
            FileContent::read(file)
        }
        Err(err) => match err.kind() {
            io::ErrorKind::NotFound => Ok(FileContent::Missing),
            _ => Err(err),
        },
    }
}

pub(crate) fn diff_files(
    hoard_path: &HoardPath,
    system_path: &SystemPath,
) -> io::Result<Option<Diff>> {
    let hoard_content = content_for(hoard_path)?;
    let system_content = content_for(system_path)?;

    let diff = match (hoard_content, system_content) {
        (FileContent::Missing, FileContent::Missing) => None,
        (FileContent::Missing, _) => Some(Diff::HoardNotExists),
        (_, FileContent::Missing) => Some(Diff::SystemNotExists),
        (FileContent::Text(hoard_text), FileContent::Text(system_text)) => {
            let text_diff = TextDiff::from_lines(&hoard_text, &system_text);

            let has_diff = text_diff
                .iter_all_changes()
                .any(|op| op.tag() != ChangeTag::Equal);

            if has_diff {
                let udiff = text_diff
                    .unified_diff()
                    .context_radius(CONTEXT_RADIUS)
                    .header(
                        &hoard_path.to_string_lossy(),
                        &system_path.to_string_lossy(),
                    )
                    .to_string();
                Some(Diff::Text(udiff))
            } else {
                None
            }
        }
        (left, right) => {
            let left = left.into_bytes();
            let right = right.into_bytes();

            left.into_iter()
                .zip(right)
                .any(|(l, r)| l != r)
                .then(|| Diff::Binary)
        }
    };

    Ok(diff)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::paths::RelativePath;
    use std::path::PathBuf;

    #[test]
    fn test_diff_non_existent_files() {
        let hoard_path = crate::paths::hoards_dir()
            .join(&RelativePath::try_from(PathBuf::from("does_not_exist")).unwrap());
        let system_path = SystemPath::try_from(PathBuf::from("/also/does/not/exist")).unwrap();
        let diff = diff_files(&hoard_path, &system_path).expect("diff should not fail");

        assert!(diff.is_none());
    }

    mod file_content {
        use super::*;

        #[test]
        fn test_text_into_bytes() {
            let string_content = String::from("text content");
            let s = FileContent::Text(string_content.clone());
            assert_eq!(s.into_bytes(), Some(string_content.into_bytes()));
        }

        #[test]
        fn test_binary_into_bytes() {
            let bytes = vec![23u8, 244u8, 0u8, 12u8, 17u8];
            let b = FileContent::Binary(bytes.clone());
            assert_eq!(b.into_bytes(), Some(bytes));
        }

        #[test]
        fn test_missing_into_bytes() {
            assert_eq!(FileContent::Missing.into_bytes(), None);
        }
    }
}
