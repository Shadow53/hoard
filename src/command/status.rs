use crate::hoard::iter::{DiffSource, diff_stream, HoardFileDiff};
use crate::hoard::Hoard;
use crate::newtypes::HoardName;
use crate::paths::HoardPath;
use futures::TryStreamExt;

pub(crate) async fn run_status<'a>(
    hoards_root: &HoardPath,
    hoards: impl IntoIterator<Item = (&'a HoardName, &'a Hoard)>,
) -> Result<(), super::Error> {
    for (hoard_name, hoard) in hoards {
        let _span = tracing::error_span!("run_status", hoard=%hoard_name).entered();
        let source = diff_stream(hoards_root, hoard_name.clone(), hoard)
            .await
            .map_err(super::Error::Status)?
            .map_err(super::Error::Status)
            .try_filter_map(|hoard_diff| async move {
                #[allow(clippy::match_same_arms)]
                let source = match hoard_diff {
                    HoardFileDiff::BinaryModified { diff_source, .. } => Some(diff_source),
                    HoardFileDiff::TextModified { diff_source, .. } => Some(diff_source),
                    HoardFileDiff::Created { diff_source, .. } => Some(diff_source),
                    HoardFileDiff::Deleted { diff_source, .. } => Some(diff_source),
                    HoardFileDiff::Unchanged(_) | HoardFileDiff::Nonexistent(_) => None,
                };

                Ok(source)
            })
            .try_fold(None, |acc, source| async move {
                match acc {
                    None => Ok(Some(source)),
                    Some(acc) => {
                        let new_source = if acc == DiffSource::Unknown || source == DiffSource::Unknown {
                            DiffSource::Unknown
                        } else if acc == source {
                            acc
                        } else {
                            DiffSource::Mixed
                        };

                        Ok(Some(new_source))
                    }
                }
            }).await?;

        match source {
            None => tracing::info!("{}: up to date", hoard_name),
            Some(source) => {
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
