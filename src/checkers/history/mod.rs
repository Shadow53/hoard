//! Keep records of previous operations (including on other system) to prevent inconsistencies
//! and accidental overwrites or deletions.

use std::path::PathBuf;
use std::{fs, io};
use uuid::Uuid;
use crate::paths::{HoardPath, RelativePath};

pub mod last_paths;
pub mod operation;

const UUID_FILE_NAME: &str = "uuid";
const HISTORY_DIR_NAME: &str = "history";

fn get_uuid_file() -> PathBuf {
    let _span = tracing::debug_span!("get_uuid_file").entered();
    crate::dirs::config_dir().join(UUID_FILE_NAME)
}

fn get_history_root_dir() -> HoardPath {
    let _span = tracing::debug_span!("get_history_root_dir").entered();
    HoardPath::try_from(crate::dirs::data_dir().join(HISTORY_DIR_NAME))
        .expect("directory rooted in the data dir is always a valid hoard path")
}

fn get_history_dir_for_id(id: Uuid) -> HoardPath {
    let _span = tracing::debug_span!("get_history_dir_for_id", %id).entered();
    get_history_root_dir().join(&RelativePath::try_from(PathBuf::from(id.to_string())).expect("uuid is always a valid relative path"))
}

fn get_history_dirs_not_for_id(id: &Uuid) -> Result<Vec<HoardPath>, io::Error> {
    let _span = tracing::debug_span!("get_history_dir_not_for_id", %id).entered();
    let root = get_history_root_dir();
    if !root.exists() {
        tracing::trace!("history root dir does not exist");
        return Ok(Vec::new());
    }

    fs::read_dir(root)?
        .filter_map(|entry| {
            match entry {
                Err(err) => Some(Err(err)),
                Ok(entry) => {
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
                    })
                }
            }
        })
        .collect()
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
pub fn get_or_generate_uuid() -> Result<Uuid, io::Error> {
    let uuid_file = get_uuid_file();
    let _span = tracing::debug_span!("get_or_generate_uuid", file = ?uuid_file);

    tracing::trace!("attempting to read uuid from file");
    let id: Option<Uuid> = match fs::read_to_string(&uuid_file) {
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
        Err(err) => {
            if err.kind() == io::ErrorKind::NotFound {
                tracing::trace!("no uuid file found: creating one");
                None
            } else {
                tracing::error!(error = %err, "error while reading uuid file");
                return Err(err);
            }
        }
    };

    // Return existing id or generate, save to file, and return.
    id.map_or_else(
        || {
            let new_id = Uuid::new_v4();
            tracing::debug!(new_uuid = %new_id, "generated new uuid");
            if let Err(err) = fs::create_dir_all(
                uuid_file
                    .parent()
                    .expect("uuid file should always have a parent directory"),
            ) {
                tracing::error!(error = %err, "error while create parent dir");
                return Err(err);
            }
            if let Err(err) = fs::write(&uuid_file, new_id.to_string()) {
                tracing::error!(error = %err, "error while saving uuid to file");
                return Err(err);
            }
            Ok(new_id)
        },
        Ok,
    )
}
