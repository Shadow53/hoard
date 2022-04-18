use crate::checkers::history::operation::ItemOperation;
use crate::checkers::{history::operation::OperationImpl, Checkers, Error as ConsistencyError};
use crate::hoard::iter::Error as IterError;
use crate::hoard::pile_config::Permissions;
use crate::hoard::{Direction, Hoard};
use crate::hoard_item::HoardItem;
use crate::newtypes::HoardName;
use crate::paths::{HoardPath, RelativePath, SystemPath};
use tokio::fs;
use std::path::Path;
use thiserror::Error;

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
pub(crate) async fn run_backup<'a>(
    hoards_root: &HoardPath,
    hoards: impl IntoIterator<Item = (&'a HoardName, &'a Hoard)> + Clone,
    force: bool,
) -> Result<(), super::Error> {
    backup_or_restore(hoards_root, Direction::Backup, hoards, force).await.map_err(super::Error::Backup)
}

#[allow(single_use_lifetimes)]
pub(crate) async fn run_restore<'a>(
    hoards_root: &HoardPath,
    hoards: impl IntoIterator<Item = (&'a HoardName, &'a Hoard)> + Clone,
    force: bool,
) -> Result<(), super::Error> {
    backup_or_restore(hoards_root, Direction::Restore, hoards, force).await.map_err(super::Error::Restore)
}

#[async_recursion::async_recursion]
async fn recursively_set_hoard_permissions(root: &HoardPath, path: &RelativePath) -> Result<(), Error> {
    let full_path = root.join(path);
    set_permissions(
        &full_path,
        Permissions::file_default(),
        Permissions::folder_default(),
    ).await?;
    if path.as_path().is_some() {
        let new_rel = path.parent();
        recursively_set_hoard_permissions(root, &new_rel).await
    } else {
        Ok(())
    }
}

#[async_recursion::async_recursion]
async fn recursively_set_system_permissions(
    root: &SystemPath,
    path: &RelativePath,
    file_perms: Permissions,
    dir_perms: Permissions,
) -> Result<(), Error> {
    let full_path = root.join(path);
    set_permissions(&full_path, file_perms, dir_perms).await?;
    if path.as_path().is_some() {
        let new_rel = path.parent();
        recursively_set_system_permissions(root, &new_rel, file_perms, dir_perms).await
    } else {
        Ok(())
    }
}

async fn set_permissions(
    path: &Path,
    file_perms: Permissions,
    dir_perms: Permissions,
) -> Result<(), Error> {
    if path.is_file() {
        file_perms.set_on_path(path).await.map_err(Error::IO)
    } else {
        dir_perms.set_on_path(path).await.map_err(Error::IO)
    }
}

#[async_recursion::async_recursion]
async fn create_all_with_perms(path: &Path, perms: Permissions) -> Result<(), Error> {
    if path.is_dir() {
        perms.set_on_path(path).await.map_err(Error::IO)
    } else {
        if let Some(parent) = path.parent() {
            create_all_with_perms(parent, perms).await?;
        }

        fs::create_dir(path).await?;
        perms.set_on_path(path).await.map_err(Error::IO)
    }
}

async fn copy_file(file: &HoardItem, direction: Direction) -> Result<(), Error> {
    let (src, dest) = match direction {
        Direction::Backup => (file.system_path().as_ref(), file.hoard_path().as_ref()),
        Direction::Restore => (file.hoard_path().as_ref(), file.system_path().as_ref()),
    };
    if let Some(parent) = dest.parent() {
        tracing::trace!(?parent, "ensuring parent dirs");
        if let Err(err) = create_all_with_perms(parent, Permissions::folder_default()).await {
            tracing::error!(
                "failed to create parent directories for {}: {}",
                dest.display(),
                err
            );
            return Err(err);
        }
    }
    tracing::debug!("copying {} to {}", src.display(), dest.display());
    if let Err(err) = fs::copy(src, dest).await {
        tracing::error!(
            "failed to copy {} to {}: {}",
            src.display(),
            dest.display(),
            err
        );
        return Err(Error::IO(err));
    }

    Ok(())
}

async fn fix_permissions(
    hoard: &Hoard,
    operation: &ItemOperation,
    direction: Direction,
) -> Result<(), Error> {
    // Set permissions if file exists, regardless of if it was modified.
    if let ItemOperation::Create(file)
    | ItemOperation::Modify(file)
    | ItemOperation::Nothing(file) = operation
    {
        match direction {
            Direction::Backup => {
                recursively_set_hoard_permissions(file.hoard_prefix(), file.relative_path()).await?;
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
                recursively_set_system_permissions(
                    file.system_prefix(),
                    file.relative_path(),
                    file_perms,
                    dir_perms,
                ).await?;
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
            tracing::trace!("found operation: {:?}", operation);
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
                        if let Err(err) = fs::remove_file(to_remove).await {
                            tracing::error!("failed to delete {}: {}", to_remove.display(), err);
                            return Err(Error::IO(err));
                        }
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
