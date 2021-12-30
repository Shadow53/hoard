use atty::Stream;
use thiserror::Error;
use std::{path::Path, process::ExitStatus};

#[derive(Debug, Error)]
#[allow(variant_size_differences)]
pub enum Error {
    #[error("failed to start editor: {0}")]
    Start(#[from] open_cmd::Error),
    #[error("editor exited with failure status: {0}")]
    Exit(ExitStatus)
}

pub(crate) fn edit(path: &Path) -> Result<(), Error> {
    let _span = tracing::trace_span!("edit", ?path).entered();

    let mut cmd = if atty::is(Stream::Stdout) && atty::is(Stream::Stderr) && atty::is(Stream::Stdin) {
        open_cmd::open_editor(path.to_owned())?
    } else {
        open_cmd::open(path.to_owned())?
    };

    let status = cmd.status().map_err(open_cmd::Error::from)?;
    if !status.success() {
        return Err(Error::Exit(status));
    }

    Ok(())
}
