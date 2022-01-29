use std::path::Path;
use crate::hoard::Hoard;
use crate::hoard::iter::{HoardDiff, HoardFilesIter};

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub(crate) fn run_diff(hoard: &Hoard, hoard_name: &str, hoards_root: &Path, verbose: bool) -> Result<(), super::Error> {
    let diff_iterator = HoardFilesIter::file_diffs(hoards_root, hoard_name, hoard).map_err(super::Error::Diff)?;
    for hoard_diff in diff_iterator {
        match hoard_diff {
            HoardDiff::BinaryModified { path, diff_source } => {
                tracing::info!(
                    "{}: binary file changed {}",
                    path.display(),
                    diff_source
                );
            }
            HoardDiff::TextModified {
                path,
                unified_diff,
                diff_source,
            } => {
                tracing::info!("{}: text file changed {}", path.display(), diff_source);
                if verbose {
                    tracing::info!("{}", unified_diff);
                }
            }
            HoardDiff::PermissionsModified {
                path,
                hoard_perms,
                system_perms,
                ..
            } => {
                #[cfg(unix)]
                tracing::info!(
                    "{}: permissions changed: hoard ({:o}), system ({:o})",
                    path.display(),
                    hoard_perms.mode(),
                    system_perms.mode(),
                );
                #[cfg(not(unix))]
                tracing::info!(
                    "{}: permissions changed: hoard ({}), system ({})",
                    path.display(),
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
            HoardDiff::Created { path, diff_source } => {
                tracing::info!("{}: created {}", path.display(), diff_source);
            }
            HoardDiff::Recreated { path, diff_source } => {
                tracing::info!("{}: recreated {}", path.display(), diff_source);
            }
            HoardDiff::Deleted { path, diff_source } => {
                tracing::info!("{}: deleted {}", path.display(), diff_source);
            }
        }
    }

    Ok(())
}
