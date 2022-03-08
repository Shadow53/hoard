use atty::Stream;
use std::{fs, path::Path, process::ExitStatus};
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(variant_size_differences)]
pub enum Error {
    #[error("failed to start editor: {0}")]
    Start(#[from] open_cmd::Error),
    #[error("editor exited with failure status: {0}")]
    Exit(ExitStatus),
    #[error("an I/O error occurred while setting up the temporary file: {0}")]
    IO(#[from] std::io::Error),
}

const DEFAULT_CONFIG: &str = include_str!("../../config.toml.sample");

/// Edit the given `path`.
///
/// The path is `Result::expect`ed to have a file name segment.
pub(crate) fn run_edit(path: &Path) -> Result<(), super::Error> {
    let _span = tracing::trace_span!("edit", ?path).entered();

    let tmp_dir = tempfile::tempdir().map_err(Error::IO)?;
    let tmp_file = tmp_dir.path().join(
        path.file_name()
            .expect("path should always have a file name"),
    );

    if path.exists() {
        fs::copy(path, &tmp_file).map_err(Error::IO)?;
    } else {
        fs::write(&tmp_file, DEFAULT_CONFIG.as_bytes()).map_err(Error::IO)?;
    }

    let mut cmd = if atty::is(Stream::Stdout) && atty::is(Stream::Stderr) && atty::is(Stream::Stdin)
    {
        open_cmd::open_editor(tmp_file.clone()).map_err(Error::Start)?
    } else {
        open_cmd::open(tmp_file.clone()).map_err(Error::Start)?
    };

    let status = cmd
        .status()
        .map_err(open_cmd::Error::from)
        .map_err(Error::Start)
        .map_err(super::Error::Edit)?;

    if status.success() {
        tracing::debug!("editing exited without error, copying temporary file back to original");
        fs::copy(tmp_file, path).map_err(Error::IO)?;
    } else {
        tracing::error!("edit command exited with status {}", status);
        return Err(super::Error::Edit(Error::Exit(status)));
    }

    Ok(())
}
