use crate::hoard::iter::{HoardDiffIter, HoardFileDiff};
use crate::hoard::Hoard;
use crate::paths::HoardPath;
use std::collections::BTreeSet;

use crate::newtypes::HoardName;

pub(crate) fn run_diff(
    hoard: &Hoard,
    hoard_name: &HoardName,
    hoards_root: &HoardPath,
    verbose: bool,
) -> Result<(), super::Error> {
    let _span = tracing::trace_span!("run_diff").entered();
    tracing::trace!("running the diff command");
    let diffs: BTreeSet<HoardFileDiff> = HoardDiffIter::new(hoards_root, hoard_name.clone(), hoard)
        .map_err(|err| {
            tracing::error!("failed to create diff iterator: {}", err);
            super::Error::Diff(err)
        })?
        .only_changed()
        .collect::<Result<_, _>>()
        .map_err(super::Error::Diff)?;
    for hoard_diff in diffs {
        tracing::trace!("printing diff: {:?}", hoard_diff);
        match hoard_diff {
            HoardFileDiff::BinaryModified { file, diff_source } => {
                tracing::info!(
                    "{}: binary file changed {}",
                    file.system_path().display(),
                    diff_source
                );
            }
            HoardFileDiff::TextModified {
                file,
                unified_diff,
                diff_source,
            } => {
                tracing::info!(
                    "{}: text file changed {}",
                    file.system_path().display(),
                    diff_source
                );
                if let (true, Some(unified_diff)) = (verbose, unified_diff) {
                    tracing::info!("{}", unified_diff);
                }
            }
            HoardFileDiff::Created {
                file,
                diff_source,
                unified_diff,
            } => {
                tracing::info!(
                    "{}: (re)created {}",
                    file.system_path().display(),
                    diff_source
                );
                if let (true, Some(unified_diff)) = (verbose, unified_diff) {
                    tracing::info!("{}", unified_diff);
                }
            }
            HoardFileDiff::Deleted { file, diff_source } => {
                tracing::info!("{}: deleted {}", file.system_path().display(), diff_source);
            }
            HoardFileDiff::Unchanged(file) => {
                tracing::debug!("{}: unmodified", file.system_path().display());
            }
            HoardFileDiff::Nonexistent(_) => {}
        }
    }

    Ok(())
}
