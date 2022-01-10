//! Utilities for checking whether files differ.
//!
//! Unified diffs are optionally available for text files. Following Git's example,
//! non-text binary files can only be detected as differing or the same.
use std::io::Read;
use std::path::Path;
use std::{fs, io};

use similar::{ChangeTag, TextDiff};

const CONTEXT_RADIUS: usize = 5;

enum FileContent {
    Text(String),
    Binary(Vec<u8>),
}

impl FileContent {
    fn read(file: fs::File) -> io::Result<Self> {
        let bytes: Vec<u8> = file.bytes().collect::<io::Result<_>>()?;
        match String::from_utf8(bytes) {
            Ok(s) => Ok(Self::Text(s)),
            Err(err) => Ok(Self::Binary(err.into_bytes())),
        }
    }

    fn into_bytes(self) -> Vec<u8> {
        match self {
            Self::Text(s) => s.into_bytes(),
            Self::Binary(v) => v,
        }
    }
}

pub(crate) enum Diff {
    /// Text content differs. Contains the generated unified diff.
    Text(String),
    /// Binary content differs. Also occurs if a file changes between text and binary formats.
    Binary,
    /// Content is the same, but permissions differ.
    Permissions,
}

pub(crate) fn diff_files(left_path: &Path, right_path: &Path) -> io::Result<Option<Diff>> {
    let left_file = fs::File::open(left_path)?;
    let left_meta = left_file.metadata()?;
    let right_file = fs::File::open(right_path)?;
    let right_meta = right_file.metadata()?;

    let left = FileContent::read(left_file)?;
    let right = FileContent::read(right_file)?;

    let permissions_diff =
        (left_meta.permissions() != right_meta.permissions()).then(|| Diff::Permissions);

    let diff = match (left, right) {
        (FileContent::Text(left_text), FileContent::Text(right_text)) => {
            let text_diff = TextDiff::from_lines(&left_text, &right_text);

            let has_diff = !text_diff
                .iter_all_changes()
                .all(|op| op.tag() == ChangeTag::Equal);

            if has_diff {
                let udiff = text_diff
                    .unified_diff()
                    .context_radius(CONTEXT_RADIUS)
                    .header(&left_path.to_string_lossy(), &right_path.to_string_lossy())
                    .to_string();
                Some(Diff::Text(udiff))
            } else {
                permissions_diff
            }
        }
        (left, right) => {
            let left = left.into_bytes();
            let right = right.into_bytes();

            left.into_iter()
                .zip(right)
                .any(|(l, r)| l != r)
                .then(|| Diff::Binary)
                .or(permissions_diff)
        }
    };

    Ok(diff)
}
