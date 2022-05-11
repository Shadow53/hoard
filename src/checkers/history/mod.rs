//! Keep records of previous operations (including on other system) to prevent inconsistencies
//! and accidental overwrites or deletions.

use std::path::PathBuf;

use futures::TryStreamExt;
use tap::TapFallible;
use tokio::{fs, io};
use tokio_stream::wrappers::ReadDirStream;
use uuid::Uuid;

use crate::paths::{HoardPath, RelativePath};

pub mod last_paths;
pub mod operation;

const UUID_FILE_NAME: &str = "uuid";
const HISTORY_DIR_NAME: &str = "history";

#[tracing::instrument(level = "debug")]
fn get_uuid_file() -> PathBuf {
    crate::dirs::config_dir().join(UUID_FILE_NAME)
}

#[tracing::instrument(level = "debug")]
fn get_history_root_dir() -> HoardPath {
    HoardPath::try_from(crate::dirs::data_dir().join(HISTORY_DIR_NAME))
        .expect("directory rooted in the data dir is always a valid hoard path")
}

#[tracing::instrument(level = "debug")]
fn get_history_dir_for_id(id: Uuid) -> HoardPath {
    get_history_root_dir().join(
        &RelativePath::try_from(PathBuf::from(id.to_string()))
            .expect("uuid is always a valid relative path"),
    )
}

#[tracing::instrument(level = "debug")]
async fn get_history_dirs_not_for_id(id: &Uuid) -> Result<Vec<HoardPath>, io::Error> {
    let root = get_history_root_dir();
    if !root.exists() {
        tracing::trace!("history root dir does not exist");
        return Ok(Vec::new());
    }

    fs::read_dir(&root)
        .await
        .map(ReadDirStream::new)
        .tap_err(|error| {
            tracing::error!(%error, "failed to list items in history root directory {}", root.display());
        })?
        .try_filter_map(|entry| async move {
            let path = entry.path();
            path.file_name().and_then(|file_name| {
                file_name.to_str().and_then(|file_str| {
                    // Only directories that have UUIDs for names and do not match "this"
                    // id.
                    Uuid::parse_str(file_str)
                        .ok()
                        .and_then(|other_id| (&other_id != id).then(|| Ok({
                            HoardPath::try_from(path.clone())
                                .expect("dir entries based in a HoardPath are always valid HoardPaths")
                        })))
                })
            }).transpose()
        })
        .try_collect()
        .await
        .tap_err(|error| {
            tracing::error!(%error, "failed to read metadata for system history directory");
        })
}

/// Get this machine's unique UUID, creating if necessary.
///
/// The UUID can be found in a file called "uuid" in the `hoard`
/// configuration directory. If the file cannot be found or its contents are invalid,
/// a new file is created.
///
/// # Errors
///
/// Any I/O unexpected errors that may occur while reading and/or
/// writing the UUID file.
pub async fn get_or_generate_uuid() -> Result<Uuid, io::Error> {
    let uuid_file = get_uuid_file();
    let _span = tracing::debug_span!("get_or_generate_uuid", file = ?uuid_file);

    tracing::trace!("attempting to read uuid from file");
    let id: Option<Uuid> = match fs::read_to_string(&uuid_file).await {
        Ok(id) => match id.parse() {
            Ok(id) => {
                tracing::trace!(uuid = %id, "successfully read uuid from file");
                Some(id)
            }
            Err(err) => {
                tracing::warn!(error = %err, bad_id = %id, "failed to parse uuid in file");
                None
            }
        },
        Err(error) => {
            if error.kind() == io::ErrorKind::NotFound {
                tracing::trace!("no uuid file found: creating one");
                None
            } else {
                tracing::error!(%error, "error while reading uuid file {}", uuid_file.display());
                return Err(error);
            }
        }
    };

    // Return existing id or generate, save to file, and return.
    match id {
        None => {
            let new_id = Uuid::new_v4();
            tracing::debug!(new_uuid = %new_id, "generated new uuid");
            fs::create_dir_all(
                uuid_file
                    .parent()
                    .expect("uuid file should always have a parent directory"),
            )
            .await
            .tap_err(|error| {
                tracing::error!(%error, "error while create parent dir");
            })?;
            fs::write(&uuid_file, new_id.as_hyphenated().to_string())
                .await.tap_err(|error| {
                tracing::error!(%error, "error while saving uuid to file {}", uuid_file.display());
            })?;
            Ok(new_id)
        }
        Some(id) => Ok(id),
    }
}
