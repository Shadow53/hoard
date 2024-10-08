//! Helpful functions to use while working with [`Operation`](super::Operation) log files.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use futures::{StreamExt, TryStream, TryStreamExt};
use once_cell::sync::Lazy;
use regex::Regex;
use tap::TapFallible;
use time::format_description::FormatItem;
use tokio::fs;
use tokio_stream::wrappers::ReadDirStream;
use uuid::Uuid;

use crate::checkers::history::get_history_root_dir;
use crate::checkers::history::operation::OperationImpl;
use crate::checkers::Checker;
use crate::hoard::Direction;

use super::{Error, Operation};

/// The format that should be used when converting an [`Operation`](super::Operation)'s timestamp
/// into a file name.
pub static TIME_FORMAT: Lazy<Vec<FormatItem<'static>>> = Lazy::new(|| {
    time::format_description::parse(
        "[year]_[month]_[day]-[hour repr:24]_[minute]_[second].[subsecond digits:6]",
    )
    .unwrap()
});

/// A regular expression that can be used to determine that a file name represents an
/// [`Operation`](super::Operation) log file.
///
/// Rather than using this directly, see [`file_is_log`].
pub(crate) static LOG_FILE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new("^[0-9]{4}(_[0-9]{2}){2}-([0-9]{2}_){2}([0-9]{2})\\.[0-9]{6}\\.log$")
        .expect("invalid log file regex")
});

/// Inspects the file name portion of the `path` to determine if it matches the format used
/// for [`Operation`](super::Operation) log files.
#[must_use]
#[tracing::instrument]
pub fn file_is_log(path: &Path) -> bool {
    let result = path.is_file()
        && match path.file_name() {
            None => false, // grcov: ignore
            Some(name) => match name.to_str() {
                None => false, // grcov: ignore
                Some(name) => LOG_FILE_REGEX.is_match(name),
            },
        };
    if result {
        tracing::trace!("file is operation log");
    } else {
        tracing::trace!("file is NOT operation log");
    }
    result
}

// Async because it is used in a stream mapping method, which requires async
#[allow(clippy::unused_async)]
#[tracing::instrument(level = "trace")]
async fn only_valid_uuid_path(entry: fs::DirEntry) -> Result<Option<fs::DirEntry>, Error> {
    tracing::trace!(
        "checking if {} is a system directory",
        entry.path().display()
    );
    if entry.path().is_dir() {
        entry.file_name().to_str().map_or_else(
            || Ok(None),
            |s| {
                tracing::trace!("checking if {} is a valid UUID", s);
                Uuid::parse_str(s).is_ok().then_some(Ok(entry)).transpose()
            },
        )
    } else {
        Ok(None)
    }
}

#[tracing::instrument(level = "trace")]
async fn log_files_to_delete_from_dir(
    path: PathBuf,
) -> Result<impl TryStream<Ok = PathBuf, Error = Error>, Error> {
    tracing::trace!("checking files in directory: {}", path.display());
    let mut files: Vec<PathBuf> = fs::read_dir(&path)
        .await
        .map(ReadDirStream::new)?
        .map_err(Error::IO)
        .try_filter_map(|subentry| async move {
            tracing::trace!("checking if {} is a log file", subentry.path().display());
            Ok(file_is_log(&subentry.path()).then(|| subentry.path()))
        })
        .try_collect()
        .await
        .tap_err(|error| {
            tracing::error!(%error, "failed to read contents of {}", path.display());
        })?;

    files.sort_unstable();

    // The last item is the latest operation for this hoard, so keep it.
    let recent = files.pop();

    // Make sure the most recent backup is (also) retained.
    if let Some(recent) = recent {
        let recent = Operation::from_file(&recent).await?;
        if recent.direction() == Direction::Restore {
            tracing::debug!(
                "most recent log is not a backup, making sure to retain a backup log too"
            );
            // Find the index of the latest backup
            let index = Box::pin(
                tokio_stream::iter(files.iter().enumerate().rev().map(Ok)).try_filter_map(
                    |(i, path)| async move {
                        Operation::from_file(path)
                            .await
                            .map(|op| (op.direction() == Direction::Backup).then_some(i))
                    },
                ),
            )
            .try_next()
            .await
            .tap_err(|error| {
                tracing::error!(%error, "error while finding most recent backup");
            })?;

            if let Some(index) = index {
                // Found index of latest backup, remove it from deletion list
                files.remove(index);
            }
        }
    } // grcov: ignore

    Ok(tokio_stream::iter(files).map(Ok))
}

// For each system folder, make a list of all log files, excluding 1 or 2 to keep.
#[tracing::instrument]
async fn log_files_to_delete(
    entry: fs::DirEntry,
) -> Result<impl TryStream<Ok = PathBuf, Error = Error>, Error> {
    let stream = fs::read_dir(entry.path())
        .await
        .map(ReadDirStream::new)?
        .map_err(Error::IO)
        .and_then(|entry| async move {
            let path = entry.path();
            tracing::trace!("found hoard directory: {}", path.display());
            Ok(path)
        })
        .try_filter_map(|path| async move { Ok(path.is_dir().then_some(path)) })
        .and_then(log_files_to_delete_from_dir)
        .try_flatten();

    Ok(stream)
}

/// Cleans up residual operation logs, leaving the latest per (system, hoard) pair.
///
/// Technically speaking, this function may leave up to two log files behind per pair.
/// If the most recent log file is for a *restore* operation, the most recent *backup* will
/// also be retained. If the most recent log file is a *backup*, it will be the only one
/// retained.
///
/// # Errors
///
/// - Any I/O error from working with and deleting multiple files
/// - Any [`Error`]s from parsing files to determine whether or not to keep them
#[tracing::instrument(level = "trace")]
pub(crate) async fn cleanup_operations() -> Result<u32, (u32, Error)> {
    // Get hoard history root
    // Iterate over every uuid in the directory
    let root = get_history_root_dir();

    // The .fold() at the end creates a new error based on the old one, which
    // is not compatible with try_fold()
    #[allow(clippy::manual_try_fold)]
    fs::read_dir(&root)
        .await
        .map(ReadDirStream::new)
        .map_err(|error| {
            tracing::error!(%error, "failed to list items in {}", root.display());
            (0, Error::IO(error))
        })?
        .map_err(|error| {
            tracing::error!(%error, "failed to read item in {}", root.display());
            Error::IO(error)
        })
        .try_filter_map(only_valid_uuid_path)
        .and_then(log_files_to_delete)
        .try_flatten()
        // Delete each file.
        .and_then(|path| async move {
            tracing::trace!("deleting {}", path.display());
            fs::remove_file(&path).await.map_err(|error| {
                tracing::error!(%error, "failed to delete {}", path.display());
                Error::IO(error)
            })
        })
        // Return the first error or the number of files deleted.
        .fold(Ok((0, ())), |acc, res2| async move {
            let (count, ()) = acc?;
            res2.map_err(|err| (count, err))?;
            Ok((count + 1, ()))
        })
        .await
        .map(|(count, ())| count)
}

#[tracing::instrument(level = "trace")]
async fn all_operations() -> Result<impl TryStream<Ok = Operation, Error = Error>, Error> {
    let history_dir = get_history_root_dir();
    tracing::trace!(?history_dir);
    let iter = fs::read_dir(&history_dir)
        .await
        .map(ReadDirStream::new)
        .tap_err(|error| {
            tracing::error!(
                %error,
                "failed to list items in history dir {}",
                history_dir.display(),
            );
        })?
        .try_filter_map(|uuid_entry| async move {
            let is_uuid = uuid_entry
                .file_name()
                .to_str()
                .map(Uuid::parse_str)
                .transpose()
                .ok()
                .flatten()
                .is_some();
            let uuid_path = uuid_entry.path();
            (is_uuid && uuid_path.is_dir())
                .then_some(uuid_path)
                .map(Ok)
                .transpose()
        })
        .and_then(|entry| async move {
            fs::read_dir(&entry)
                .await
                .map(ReadDirStream::new)
                .tap_err(|error| {
                    tracing::error!(
                        %error,
                        "failed to list items in system history dir {}",
                        entry.display(),
                    );
                })
        })
        .try_flatten()
        .try_filter_map(|hoard_entry| async move {
            hoard_entry
                .path()
                .is_dir()
                .then(|| hoard_entry.path())
                .map(Ok)
                .transpose()
        }) // Iterator of PathBuf
        .and_then(|entry| async move {
            fs::read_dir(&entry)
                .await
                .map(ReadDirStream::new)
                .tap_err(|error| {
                    tracing::error!(
                        %error,
                        "failed to list items in hoard history dir {}",
                        entry.display(),
                    );
                })
        })
        .try_flatten() // Iterator of DirEntry (log files)
        .map_ok(|hoard_entry| hoard_entry.path()) // Iterator of PathBuf
        .try_filter_map(|path| async move { Ok(file_is_log(&path).then_some(path)) }) // Only those paths that are log files
        .map_err(Error::IO)
        .and_then(|path| async move { Operation::from_file(&path).await });

    Ok(iter)
}

#[tracing::instrument(level = "trace")]
async fn sorted_operations() -> Result<Vec<Operation>, Error> {
    let mut list: Vec<Operation> = all_operations().await?.try_collect().await?;
    list.sort_unstable_by_key(Operation::timestamp);
    Ok(list)
}

#[tracing::instrument(level = "trace")]
pub(crate) async fn upgrade_operations() -> Result<(), Error> {
    tracing::debug!("upgrading operation files to latest version");
    let mut top_file_checksum_map = HashMap::new();
    let mut top_file_set = HashMap::new();

    let all_ops: Vec<_> = sorted_operations().await?;
    tracing::trace!("found operations: {:?}", all_ops);

    for operation in all_ops {
        if !top_file_checksum_map.contains_key(operation.hoard_name()) {
            top_file_checksum_map.insert(operation.hoard_name().clone(), HashMap::new());
            top_file_set.insert(operation.hoard_name().clone(), HashSet::new());
        }
        let file_checksum_map = top_file_checksum_map
            .get_mut(operation.hoard_name())
            .expect("checksum map should always exist");
        let file_set = top_file_set
            .get_mut(operation.hoard_name())
            .expect("file set should always exist");
        tracing::trace!(?operation, "converting operation");
        let operation = operation.convert_to_latest_version(file_checksum_map, file_set);
        tracing::trace!(?operation, "converted operation");
        operation.commit_to_disk().await?;
    }

    Ok(())
}
