//! Utilities for checking whether files differ.
//!
//! Unified diffs are optionally available for text files. Following Git's example,
//! non-text binary files can only be detected as differing or the same.
use std::path::Path;
use tokio::{fs, io, io::AsyncReadExt};

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
    #[tracing::instrument(level = "debug")]
    pub async fn read(mut file: fs::File) -> io::Result<Self> {
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).await?;
        match String::from_utf8(bytes) {
            Ok(s) => Ok(Self::Text(s)),
            Err(err) => Ok(Self::Binary(err.into_bytes())),
        }
    }

    #[tracing::instrument(level = "debug")]
    pub async fn read_path(path: &Path) -> io::Result<Self> {
        match fs::File::open(path).await {
            Ok(file) => FileContent::read(file).await,
            Err(err) => match err.kind() {
                io::ErrorKind::NotFound => Ok(FileContent::Missing),
                _ => Err(err),
            },
        }
    }

    #[tracing::instrument(level = "debug")]
    pub fn as_bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Text(s) => Some(s.as_bytes()),
            Self::Binary(v) => Some(v.as_slice()),
            Self::Missing => None,
        }
    }
}

#[allow(variant_size_differences)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Diff {
    /// Text content differs. Contains the generated unified diff.
    Text(String),
    /// Binary content differs. Also occurs if a file changes between text and binary formats.
    Binary,
    /// The left path to `diff_files` did not exist, but the right path did.
    HoardNotExists,
    /// The left path to `diff_paths` existed, but the right path did not.
    SystemNotExists,
}

pub(crate) fn str_diff(
    (hoard_path, hoard_text): (&HoardPath, &str),
    (system_path, system_text): (&SystemPath, &str),
) -> Option<Diff> {
    let text_diff = TextDiff::from_lines(hoard_text, system_text);

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

#[cfg(test)]
mod test {
    use super::*;

    mod file_content {
        use super::*;

        #[test]
        fn test_text_into_bytes() {
            let string_content = String::from("text content");
            let s = FileContent::Text(string_content.clone());
            assert_eq!(s.as_bytes(), Some(string_content.as_bytes()));
        }

        #[test]
        fn test_binary_into_bytes() {
            let bytes = vec![23u8, 244u8, 0u8, 12u8, 17u8];
            let b = FileContent::Binary(bytes.clone());
            assert_eq!(b.as_bytes(), Some(bytes.as_slice()));
        }

        #[test]
        fn test_missing_into_bytes() {
            assert_eq!(FileContent::Missing.as_bytes(), None);
        }
    }
}
