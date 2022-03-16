use crate::hoard::iter::{HoardDiffIter, HoardFileDiff};
use crate::hoard::Hoard;
use crate::paths::HoardPath;
use std::collections::BTreeSet;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub(crate) fn run_diff(
    hoard: &Hoard,
    hoard_name: &str,
    hoards_root: &HoardPath,
    verbose: bool,
) -> Result<(), super::Error> {
    let _span = tracing::trace_span!("run_diff").entered();
    tracing::trace!("running the diff command");
    let diffs: BTreeSet<HoardFileDiff> =
        HoardDiffIter::new(hoards_root, hoard_name.to_string(), hoard)
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
                if verbose {
                    tracing::info!("{}", unified_diff);
                }
            }
            HoardFileDiff::PermissionsModified {
                file,
                hoard_perms,
                system_perms,
                ..
            } => {
                #[cfg(unix)]
                tracing::info!(
                    "{}: permissions changed: hoard ({:o}), system ({:o})",
                    file.system_path().display(),
                    hoard_perms.mode(),
                    system_perms.mode(),
                );
                #[cfg(not(unix))]
                tracing::info!(
                    "{}: permissions changed: hoard ({}), system ({})",
                    file.system_path().display(),
                    if hoard_perms.readonly() {
                        "readonly"
                    } else {
                        "writable"
                    },
                    if system_perms.readonly() {
                        "readonly"
                    } else {
                        "writable"
                    },
                );
            }
            HoardFileDiff::Created {
                file,
                diff_source,
                unified_diff,
            } => {
                tracing::info!("{}: created {}", file.system_path().display(), diff_source);
                if let (true, Some(unified_diff)) = (verbose, unified_diff) {
                    tracing::info!("{}", unified_diff);
                }
            }
            HoardFileDiff::Recreated {
                file,
                diff_source,
                unified_diff,
            } => {
                tracing::info!(
                    "{}: recreated {}",
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
        }
    }

    Ok(())
}
