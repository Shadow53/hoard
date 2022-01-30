use crate::hoard::iter::{file_diffs, HoardDiff};
use crate::hoard::Hoard;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

pub(crate) fn run_diff(
    hoard: &Hoard,
    hoard_name: &str,
    hoards_root: &Path,
    verbose: bool,
) -> Result<(), super::Error> {
    let diff_iterator = file_diffs(hoards_root, hoard_name, hoard).map_err(super::Error::Diff)?;
    for hoard_diff in diff_iterator {
        match hoard_diff {
            HoardDiff::BinaryModified { file, diff_source } => {
                tracing::info!("{}: binary file changed {}", file.system_path.display(), diff_source);
            }
            HoardDiff::TextModified {
                file,
                unified_diff,
                diff_source,
            } => {
                tracing::info!("{}: text file changed {}", file.system_path.display(), diff_source);
                if verbose {
                    tracing::info!("{}", unified_diff);
                }
            }
            HoardDiff::PermissionsModified {
                file,
                hoard_perms,
                system_perms,
                ..
            } => {
                #[cfg(unix)]
                tracing::info!(
                    "{}: permissions changed: hoard ({:o}), system ({:o})",
                    file.system_path.display(),
                    hoard_perms.mode(),
                    system_perms.mode(),
                );
                #[cfg(not(unix))]
                tracing::info!(
                    "{}: permissions changed: hoard ({}), system ({})",
                    file.system_path.display(),
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
            HoardDiff::Created { file, diff_source } => {
                tracing::info!("{}: created {}", file.system_path.display(), diff_source);
            }
            HoardDiff::Recreated { file, diff_source } => {
                tracing::info!("{}: recreated {}", file.system_path.display(), diff_source);
            }
            HoardDiff::Deleted { file, diff_source } => {
                tracing::info!("{}: deleted {}", file.system_path.display(), diff_source);
            }
        }
    }

    Ok(())
}
