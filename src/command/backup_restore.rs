use crate::checkers::{Checkers, Error as ConsistencyError, history::operation::OperationImpl};
use crate::hoard::iter::{Error as IterError, ItemOperation};
use crate::hoard::{Direction, Hoard};
use crate::newtypes::HoardName;
use crate::paths::{HoardPath, RelativePath};
use std::fs;
use thiserror::Error;

/// Errors that may occur while backing up or restoring hoards.
#[derive(Debug, Error)]
pub enum Error {
    /// A [`Checkers`] consistency check failed.
    #[error("consistency check failed: {0}")]
    Consistency(#[from] ConsistencyError),
    /// An I/O error occurred.
    #[error("I/O error: {0}")]
    IO(#[from] std::io::Error),
    /// An error while iterating files to modify.
    #[error("failed to iterate files: {0}")]
    Iterator(#[from] IterError),
}

#[allow(single_use_lifetimes)]
pub(crate) fn run_backup<'a>(
    hoards_root: &HoardPath,
    hoards: impl IntoIterator<Item = (&'a HoardName, &'a Hoard)> + Clone,
    force: bool,
) -> Result<(), super::Error> {
    backup_or_restore(hoards_root, Direction::Backup, hoards, force).map_err(super::Error::Backup)
}

#[allow(single_use_lifetimes)]
pub(crate) fn run_restore<'a>(
    hoards_root: &HoardPath,
    hoards: impl IntoIterator<Item = (&'a HoardName, &'a Hoard)> + Clone,
    force: bool,
) -> Result<(), super::Error> {
    backup_or_restore(hoards_root, Direction::Restore, hoards, force).map_err(super::Error::Restore)
}

#[allow(single_use_lifetimes)]
fn backup_or_restore<'a>(
    hoards_root: &HoardPath,
    direction: Direction,
    hoards: impl IntoIterator<Item = (&'a HoardName, &'a Hoard)> + Clone,
    force: bool,
) -> Result<(), Error> {
    let mut checkers = Checkers::new(hoards_root, hoards.clone(), direction)?;
    tracing::debug!(?checkers, "================");
    if !force {
        checkers.check()?;
    }

    // TODO: decrease runtime by using computed values from `checkers` instead of running
    // the iterator again.
    for (name, hoard) in hoards {
        match direction {
            Direction::Backup => tracing::info!(hoard=%name, "backing up"),
            Direction::Restore => tracing::info!(hoard=%name, "restoring"),
        }

        let hoard_prefix = hoards_root.join(&RelativePath::from(name));
        let op = checkers.get_operation_for(name).expect("operation should exist for hoard");
        let iter = op.hoard_operations_iter(&hoard_prefix, hoard).map_err(ConsistencyError::Operation)?;
        for operation in iter {
            tracing::trace!("found operation: {:?}", operation);
            match operation {
                ItemOperation::Create(file) | ItemOperation::Modify(file) => {
                    let (src, dest) = match direction {
                        Direction::Backup => {
                            (file.system_path().as_ref(), file.hoard_path().as_ref())
                        }
                        Direction::Restore => {
                            (file.hoard_path().as_ref(), file.system_path().as_ref())
                        }
                    };
                    if let Some(parent) = dest.parent() {
                        tracing::trace!(?parent, "ensuring parent dirs");
                        if let Err(err) = fs::create_dir_all(parent) {
                            tracing::error!(
                                "failed to create parent directories for {}: {}",
                                dest.display(),
                                err
                            );
                            return Err(Error::IO(err));
                        }
                    }
                    tracing::debug!("copying {} to {}", src.display(), dest.display());
                    if let Err(err) = fs::copy(src, dest) {
                        tracing::error!(
                            "failed to copy {} to {}: {}",
                            src.display(),
                            dest.display(),
                            err
                        );
                        return Err(Error::IO(err));
                    }
                }
                ItemOperation::Delete(file) => {
                    let to_remove = match direction {
                        Direction::Backup => file.hoard_path().as_ref(),
                        Direction::Restore => file.system_path().as_ref(),
                    };
                    tracing::debug!("deleting {}", to_remove.display());
                    if let Err(err) = fs::remove_file(to_remove) {
                        tracing::error!("failed to delete {}: {}", to_remove.display(), err);
                        return Err(Error::IO(err));
                    }
                }
                ItemOperation::Nothing(file) => {
                    tracing::debug!("file {} is unchanged", file.system_path().display());
                }
            }
        }
    }

    checkers.commit_to_disk().map_err(Error::Consistency)
}
