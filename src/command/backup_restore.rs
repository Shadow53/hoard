use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};

use tap::TapFallible;
//use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::fs;

use crate::checkers::history::operation::ItemOperation;
use crate::checkers::{history::operation::OperationImpl, Checkers, Error as ConsistencyError};
use crate::hoard::iter::Error as IterError;
use crate::hoard::pile_config::Permissions;
use crate::hoard::{Direction, Hoard};
use crate::hoard_item::HoardItem;
use crate::newtypes::HoardName;
use crate::paths::{HoardPath, RelativePath, SystemPath};

/// Errors that may occur while backing up or restoring hoards.
#[derive(Debug, Error)]
pub enum Error {
    /// A [`Checkers`] consistency check failed.
    #[error("consistency check failed: {0}")]
    Consistency(#[from] ConsistencyError),
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    IO(#[from] tokio::io::Error),
    /// An error while iterating files to modify.
    #[error("failed to iterate files: {0}")]
    Iterator(#[from] IterError),
}

#[allow(single_use_lifetimes)]
#[tracing::instrument(skip(hoards))]
pub(crate) async fn run_backup<'a>(
    hoards_root: &HoardPath,
    hoards: impl IntoIterator<Item = (&'a HoardName, &'a Hoard)> + Clone,
    force: bool,
) -> Result<(), super::Error> {
    backup_or_restore(hoards_root, Direction::Backup, hoards, force)
        .await
        .map_err(super::Error::Backup)
}

#[allow(single_use_lifetimes)]
#[tracing::instrument(skip(hoards))]
pub(crate) async fn run_restore<'a>(
    hoards_root: &HoardPath,
    hoards: impl IntoIterator<Item = (&'a HoardName, &'a Hoard)> + Clone,
    force: bool,
) -> Result<(), super::Error> {
    backup_or_restore(hoards_root, Direction::Restore, hoards, force)
        .await
        .map_err(super::Error::Restore)
}

struct ParentIter {
    root: Option<PathBuf>,
    segments: Vec<OsString>,
}

impl Iterator for ParentIter {
    type Item = PathBuf;

    fn next(&mut self) -> Option<Self::Item> {
        match (self.root.take(), self.segments.pop()) {
            (None, _) => None,
            (Some(root), None) => Some(root),
            (Some(root), Some(seg)) => {
                let result = root.clone();
                self.root = Some(root.join(seg));
                Some(result)
            }
        }
    }
}

impl ParentIter {
    fn new(root: PathBuf, start: &Path) -> Self {
        let segments = if start == root {
            Vec::new()
        } else {
            let rel_path = start
                .strip_prefix(&root)
                .map(Path::to_path_buf)
                .expect("start path should always be rooted in root");
            rel_path
                .components()
                .filter_map(|comp| {
                    if let Component::Normal(seg) = comp {
                        Some(seg.to_os_string())
                    } else if let Component::ParentDir = comp {
                        panic!("Paths with parent directory marker `..` are not supported")
                    } else {
                        None
                    }
                })
                .rev()
                .collect()
        };

        Self {
            root: Some(root),
            segments,
        }
    }
}

async fn looped_set_permissions(
    root: PathBuf,
    path: &Path,
    file_perms: Permissions,
    dir_perms: Permissions,
) -> Result<(), Error> {
    for path in ParentIter::new(root, path) {
        if path.is_dir() {
            dir_perms.set_on_path(&path).await?;
        } else if path.is_file() {
            file_perms.set_on_path(&path).await?;
        }
    }

    Ok(())
}

async fn looped_set_hoard_permissions(root: &HoardPath, path: &RelativePath) -> Result<(), Error> {
    looped_set_permissions(
        root.to_path_buf(),
        root.join(path).to_path_buf().as_path(),
        Permissions::file_default(),
        Permissions::folder_default(),
    )
    .await
}

async fn looped_set_system_permissions(
    root: &SystemPath,
    path: &RelativePath,
    file_perms: Permissions,
    dir_perms: Permissions,
) -> Result<(), Error> {
    looped_set_permissions(
        root.to_path_buf(),
        root.join(path).to_path_buf().as_path(),
        file_perms,
        dir_perms,
    )
    .await
}

async fn create_all_with_perms(
    root: PathBuf,
    path: &Path,
    perms: Permissions,
) -> Result<(), Error> {
    // Create all directories above root with system default permissions
    fs::create_dir_all(&root).await.tap_err(|error| {
        tracing::error!(%error, "failed to create pile root {} with system default permissions", root.display());
    })?;

    for path in ParentIter::new(root, path) {
        if !path.is_dir() {
            fs::create_dir(&path).await.tap_err(|error| {
                tracing::error!(%error, "failed to create {}", path.display());
            })?;
        }

        perms.set_on_path(&path).await?;
    }

    Ok(())
}

#[tracing::instrument(fields(file = ?file.system_path()))]
async fn copy_file(file: &HoardItem, direction: Direction) -> Result<(), Error> {
    let (src, dest, dest_root) = match direction {
        Direction::Backup => (
            file.system_path().as_ref(),
            file.hoard_path().as_ref(),
            file.hoard_prefix().as_ref(),
        ),
        Direction::Restore => (
            file.hoard_path().as_ref(),
            file.system_path().as_ref(),
            file.system_prefix().as_ref(),
        ),
    };
    if let Some(parent) = dest.parent() {
        tracing::trace!(?parent, "ensuring parent dirs");
        // Handle cases where pile == file and prefix == dest path
        let root = if dest_root == dest {
            parent.to_path_buf()
        } else {
            dest_root.to_path_buf()
        };
        create_all_with_perms(root, parent, Permissions::folder_default()).await?;
    }
    tracing::debug!("copying {} to {}", src.display(), dest.display());
    fs::copy(src, dest).await.tap_err(|error| {
        tracing::error!(
            %error,
            "failed to copy {} to {}",
            src.display(),
            dest.display(),
        );
    })?;

    Ok(())
}

#[tracing::instrument(skip(hoard))]
async fn fix_permissions(
    hoard: &Hoard,
    operation: &ItemOperation<HoardItem>,
    direction: Direction,
) -> Result<(), Error> {
    // Set permissions if file exists, regardless of if it was modified.
    if let ItemOperation::Create(file)
    | ItemOperation::Modify(file)
    | ItemOperation::Nothing(file) = operation
    {
        match direction {
            Direction::Backup => {
                looped_set_hoard_permissions(file.hoard_prefix(), file.relative_path()).await?;
            }
            Direction::Restore => {
                let pile = hoard
                    .get_pile(file.pile_name())
                    .expect("pile name should always be valid here");
                let file_perms = pile
                    .config
                    .file_permissions
                    .unwrap_or_else(Permissions::file_default);
                let dir_perms = pile
                    .config
                    .folder_permissions
                    .unwrap_or_else(Permissions::folder_default);

                tracing::debug!(
                    "setting file ({:o}) and folder ({:o}) permissions",
                    file_perms.as_mode(),
                    dir_perms.as_mode()
                );
                looped_set_system_permissions(
                    file.system_prefix(),
                    file.relative_path(),
                    file_perms,
                    dir_perms,
                )
                .await?;
            }
        }
    }

    Ok(())
}

#[allow(single_use_lifetimes)]
async fn backup_or_restore<'a>(
    hoards_root: &HoardPath,
    direction: Direction,
    hoards: impl IntoIterator<Item = (&'a HoardName, &'a Hoard)> + Clone,
    force: bool,
) -> Result<(), Error> {
    tracing::info!("processing files before {}", direction);
    let mut checkers = Checkers::new(hoards_root, hoards.clone(), direction).await?;
    tracing::debug!(?checkers, "================");
    if !force {
        checkers.check().await?;
    }

    for (name, hoard) in hoards {
        match direction {
            Direction::Backup => tracing::info!(hoard=%name, "backing up"),
            Direction::Restore => tracing::info!(hoard=%name, "restoring"),
        }

        let hoard_prefix = hoards_root.join(&RelativePath::from(name));
        let op = checkers
            .get_operation_for(name)
            .expect("operation should exist for hoard");
        let iter = op
            .hoard_operations_iter(&hoard_prefix, hoard)
            .map_err(ConsistencyError::Operation)?;
        for operation in iter {
            match &operation {
                ItemOperation::Create(file) | ItemOperation::Modify(file) => {
                    copy_file(file, direction).await?;
                }
                ItemOperation::Delete(file) => {
                    let to_remove = match direction {
                        Direction::Backup => file.hoard_path().as_ref(),
                        Direction::Restore => file.system_path().as_ref(),
                    };
                    if to_remove.exists() {
                        tracing::debug!("deleting {}", to_remove.display());
                        fs::remove_file(to_remove).await.tap_err(|error| {
                            tracing::error!(%error, "failed to delete {}", to_remove.display());
                        })?;
                    }
                }
                ItemOperation::Nothing(file) => {
                    tracing::debug!("file {} is unchanged", file.system_path().display());
                }
                ItemOperation::DoesNotExist(file) => {
                    tracing::trace!("file {} does not exist", file.system_path().display());
                }
            }

            fix_permissions(hoard, &operation, direction).await?;
        }
    }

    checkers.commit_to_disk().await.map_err(Error::Consistency)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod parent_iter {
        use crate::test::path_string;

        use super::*;

        #[test]
        fn test_single_path() {
            let path = PathBuf::from("/some/test/path");
            let returned: Vec<PathBuf> = ParentIter::new(path.clone(), &path).collect();
            assert_eq!(returned, vec![path]);
        }

        #[test]
        fn test_with_parent() {
            let parent = PathBuf::from("/some/parent/path");
            let path = parent.join("child");
            let returned: Vec<PathBuf> = ParentIter::new(parent.clone(), &path).collect();
            assert_eq!(returned, vec![parent, path]);
        }

        #[test]
        fn test_with_grandparent() {
            let grandparent = PathBuf::from("/some/grandparent");
            let parent = grandparent.join("parent");
            let child = parent.join("child");
            let returned: Vec<PathBuf> = ParentIter::new(grandparent.clone(), &child).collect();
            let expected = vec![grandparent, parent, child];
            assert_eq!(returned, expected);
        }

        #[test]
        fn test_cannot_have_parent_str_in_paths() {
            // This test serves as a canary for the panic!() inside of ParentIter.
            SystemPath::try_from(PathBuf::from(path_string!("/../test")))
                .expect_err("hoard path should not allow non-canonicalized ..");
            let valid = SystemPath::try_from(PathBuf::from(path_string!("/valid/../test")))
                .expect(".. should get removed");
            let expected = SystemPath::try_from(PathBuf::from(path_string!("/test"))).unwrap();
            assert_eq!(valid, expected);
        }
    }
}
