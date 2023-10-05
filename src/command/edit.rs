use std::{
    io::{stderr, stdin, stdout, IsTerminal},
    path::{Path, PathBuf},
    process::ExitStatus,
};

use tap::TapFallible;
use thiserror::Error;
use tokio::{fs, io};

use super::DEFAULT_CONFIG;

/// Errors that may occur while running the edit command.
#[derive(Debug, Error)]
#[allow(variant_size_differences)]
pub enum Error {
    /// An error occurred while trying to start the editor.
    #[error("failed to start editor: {0}")]
    Start(#[from] open_cmd::Error),
    /// The editor exited with an error status.
    #[error("editor exited with failure status: {0}")]
    Exit(ExitStatus),
    /// An I/O error occurred while working with the temporary file.
    #[error("an I/O error occurred while setting up the temporary file: {0}")]
    IO(#[from] io::Error),
    /// A directory was provided as the configuration file path.
    #[error("expected a configuration file, found a directory: {0}")]
    IsDirectory(PathBuf),
}

/// Edit the configuration file at `path`.
///
/// This function:
///
/// 1. Creates a temporary file by either copying the existing file at `path` or, if
///    the file does not exist, populating it with the example configuration.
/// 2. Opens the file...
///    1. In `$EDITOR` if the variable exists and `hoard` is running in a terminal.
///    2. Or in the system default graphical editor for the file
/// 3. If the editor process exits without failure...
///    1. The temporary file is copied to the given `path`.
/// 4. The temporary file is deleted.
///
/// # Errors
///
/// See [`Error`].
#[tracing::instrument]
pub(crate) async fn run_edit(path: &Path) -> Result<(), super::Error> {
    let tmp_dir = tempfile::tempdir().map_err(Error::IO).tap_err(|error| {
        tracing::error!(%error, "failed to create temporary file for editing");
    })?;
    let tmp_file = tmp_dir.path().join(
        path.file_name()
            .ok_or_else(|| Error::IsDirectory(path.to_path_buf()))?,
    );

    if path.exists() {
        fs::copy(path, &tmp_file).await.map_err(|error| {
            tracing::error!(%error, "failed to copy config file ({}) to temporary file ({})", path.display(), tmp_file.display());
            Error::IO(error)
        })?;
    } else {
        fs::write(&tmp_file, DEFAULT_CONFIG.as_bytes())
            .await
            .map_err(|error| {
                tracing::error!(%error, "failed to write default sample config to temporary file ({})", tmp_file.display());
                Error::IO(error)
            })?;
    }

    let mut cmd = if stdin().is_terminal() && stderr().is_terminal() && stdout().is_terminal() {
        open_cmd::open_editor(tmp_file.clone()).map_err(|error| {
            tracing::error!(%error, "failed to generate CLI editor command");
            Error::Start(error)
        })?
    } else {
        open_cmd::open(tmp_file.clone()).map_err(|error| {
            tracing::error!(%error, "failed to generate editor command");
            Error::Start(error)
        })?
    };

    let status = cmd.status().map_err(|error| {
        tracing::error!(%error, "failed to run editor command");
        super::Error::Edit(Error::Start(open_cmd::Error::from(error)))
    })?;

    if status.success() {
        tracing::debug!("editing exited without error, copying temporary file back to original");
        fs::copy(&tmp_file, path).await.map_err(|error| {
            tracing::error!(%error, "failed to copy temporary file ({}) to config file location ({})", tmp_file.display(), path.display());
            Error::IO(error)
        })?;
    } else {
        tracing::error!("edit command exited with status {}", status);
        return Err(super::Error::Edit(Error::Exit(status)));
    }

    Ok(())
}
