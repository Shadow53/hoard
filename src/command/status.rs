use crate::hoard::iter::{DiffSource, HoardDiffIter, HoardFileDiff};
use crate::hoard::Hoard;
use crate::newtypes::HoardName;
use crate::paths::HoardPath;

pub(crate) fn run_status<'a>(
    hoards_root: &HoardPath,
    hoards: impl IntoIterator<Item = (&'a HoardName, &'a Hoard)>,
) -> Result<(), super::Error> {
    for (hoard_name, hoard) in hoards {
        let _span = tracing::error_span!("run_status", hoard=%hoard_name).entered();
        let source = HoardDiffIter::new(hoards_root, hoard_name.clone(), hoard)
            .map_err(super::Error::Status)?
            .filter_map(|hoard_diff| {
                let hoard_diff = match hoard_diff {
                    Ok(hoard_diff) => hoard_diff,
                    Err(err) => return Some(Err(err)),
                };

                #[allow(clippy::match_same_arms)]
                let source = match hoard_diff {
                    HoardFileDiff::BinaryModified { diff_source, .. } => Some(diff_source),
                    HoardFileDiff::TextModified { diff_source, .. } => Some(diff_source),
                    HoardFileDiff::Created { diff_source, .. } => Some(diff_source),
                    HoardFileDiff::Deleted { diff_source, .. } => Some(diff_source),
                    HoardFileDiff::Unchanged(_) | HoardFileDiff::Nonexistent(_) => return None,
                };

                source.map(Ok)
            })
            .reduce(|acc, source| {
                let acc = acc?;
                let source = source?;
                if acc == DiffSource::Unknown || source == DiffSource::Unknown {
                    Ok(DiffSource::Unknown)
                } else if acc == source {
                    Ok(acc)
                } else {
                    Ok(DiffSource::Mixed)
                }
            });

        match source {
            None => tracing::info!("{}: up to date", hoard_name),
            Some(source) => {
                let source = source.map_err(super::Error::Status)?;
                match source {
                    DiffSource::Local => tracing::info!(
                        "{}: modified {} -- sync with `hoard backup {}`",
                        hoard_name, source, hoard_name
                    ),
                    DiffSource::Remote => tracing::info!(
                        "{}: modified {} -- sync with `hoard restore {}`",
                        hoard_name, source, hoard_name
                    ),
                    DiffSource::Mixed => tracing::info!(
                        "{0}: mixed changes -- manual intervention recommended (see `hoard diff {0}`)",
                        hoard_name
                    ),
                    DiffSource::Unknown => tracing::info!(
                        "{0}: unexpected changes -- manual intervention recommended (see `hoard diff {0}`)",
                        hoard_name
                    ),
                }
            }
        }
    }

    Ok(())
}
