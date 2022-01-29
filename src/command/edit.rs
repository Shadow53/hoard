use atty::Stream;
use std::{path::Path, process::ExitStatus};
use thiserror::Error;

#[derive(Debug, Error)]
#[allow(variant_size_differences)]
pub enum Error {
    #[error("failed to start editor: {0}")]
    Start(#[from] open_cmd::Error),
    #[error("editor exited with failure status: {0}")]
    Exit(ExitStatus),
}

pub(crate) fn run_edit(path: &Path) -> Result<(), super::Error> {
    let _span = tracing::trace_span!("edit", ?path).entered();

    let mut cmd = if atty::is(Stream::Stdout) && atty::is(Stream::Stderr) && atty::is(Stream::Stdin)
    {
        open_cmd::open_editor(path.to_owned()).map_err(Error::Start)?
    } else {
        open_cmd::open(path.to_owned()).map_err(Error::Start)?
    };

    let status = cmd.status()
        .map_err(open_cmd::Error::from)
        .map_err(Error::Start)
        .map_err(super::Error::Edit)?;
    if !status.success() {
        return Err(super::Error::Edit(Error::Exit(status)));
    }

    Ok(())
}
