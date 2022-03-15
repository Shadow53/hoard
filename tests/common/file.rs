use std::fs::Permissions;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::{
    fs::{File, Metadata},
    io::{Read, Seek, SeekFrom},
};

use hoard::hoard_item::{Checksum, HoardItem};
use tempfile::{NamedTempFile, TempDir};

pub fn get_temp_file() -> NamedTempFile {
    NamedTempFile::new().expect("failed to create temp file")
}

pub fn get_temp_dir() -> TempDir {
    TempDir::new().expect("failed to create temp dir")
}

fn get_metadata(left: &File, right: &File) -> (Metadata, Metadata) {
    let left_meta = left
        .metadata()
        .expect("failed to get metadata for left file");
    let right_meta = right
        .metadata()
        .expect("failed to get metadata for right file");

    (left_meta, right_meta)
}

pub fn assert_eq_files(left: &mut File, right: &mut File) {
    assert_eq_file_types(left, right);
    assert_eq_file_permissions(left, right);
    assert_eq_file_contents(left, right);
}

pub fn assert_eq_file_types(left: &File, right: &File) {
    let (left_meta, right_meta) = get_metadata(left, right);
    assert_eq!(
        left_meta.file_type(),
        right_meta.file_type(),
        "files are not the samee type (dir, file, symlink)"
    );
}

pub fn assert_eq_file_contents(left: &mut File, right: &mut File) {
    let (left_meta, right_meta) = get_metadata(left, right);
    assert_eq!(
        left_meta.len(),
        right_meta.len(),
        "files are not the same length"
    );

    // Ensure seek to beginning of file
    left.seek(SeekFrom::Start(0))
        .expect("failed to seek to beginning of left file (beginning)");
    right
        .seek(SeekFrom::Start(0))
        .expect("failed to seek to beginning of right file (beginning)");

    // Create iterator over bytes
    let is_equal = left
        .bytes()
        .zip(right.bytes())
        .map(|(l, r)| {
            (
                l.expect("failed to read from left file"),
                r.expect("failed to read from right file"),
            )
        })
        .all(|(l, r)| l == r);

    assert!(is_equal, "file contents differ");

    // Return to beginning of file before returning
    left.seek(SeekFrom::Start(0))
        .expect("failed to seek to beginning of left file (end)");
    right
        .seek(SeekFrom::Start(0))
        .expect("failed to seek to beginning of right file (end)");
}

#[cfg(not(unix))]
fn assert_mode(left_perm: &Permissions, right_perm: &Permissions) {}

#[cfg(unix)]
fn assert_mode(left_perm: &Permissions, right_perm: &Permissions) {
    assert_eq!(
        left_perm.mode(),
        right_perm.mode(),
        "Unix file modes differ"
    );
}

pub fn assert_eq_file_permissions(left: &File, right: &File) {
    let (left_meta, right_meta) = get_metadata(left, right);

    let left_perm = left_meta.permissions();
    let right_perm = right_meta.permissions();

    // The only permission currently available on all systems
    assert_eq!(
        left_perm.readonly(),
        right_perm.readonly(),
        "exactly one of the files is readonly"
    );

    assert_mode(&left_perm, &right_perm);
}
