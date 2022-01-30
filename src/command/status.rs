use crate::hoard::iter::{file_diffs, DiffSource, HoardDiff};
use crate::hoard::Hoard;
use std::path::Path;

pub(crate) fn run_status<'a>(
    hoards_root: &Path,
    hoards: impl IntoIterator<Item = (&'a str, &'a Hoard)>,
) -> Result<(), super::Error> {
    for (hoard_name, hoard) in hoards {
        let source = file_diffs(hoards_root, hoard_name, hoard)
            .map_err(super::Error::Status)?
            .into_iter()
            .map(|hoard_diff| {
                #[allow(clippy::match_same_arms)]
                match hoard_diff {
                    HoardDiff::BinaryModified { diff_source, .. } => diff_source,
                    HoardDiff::TextModified { diff_source, .. } => diff_source,
                    HoardDiff::PermissionsModified { diff_source, .. } => diff_source,
                    HoardDiff::Created { diff_source, .. } => diff_source,
                    HoardDiff::Recreated { diff_source, .. } => diff_source,
                    HoardDiff::Deleted { diff_source, .. } => diff_source,
                }
            })
            .reduce(|acc, source| {
                if acc == DiffSource::Unknown || source == DiffSource::Unknown {
                    DiffSource::Unknown
                } else if acc == source {
                    acc
                } else {
                    DiffSource::Mixed
                }
            });

        match source {
            None => println!("{}: up to date", hoard_name),
            Some(source) => match source {
                DiffSource::Local => println!(
                    "{}: modified {} -- sync with `hoard backup {}`",
                    hoard_name, source, hoard_name
                ),
                DiffSource::Remote => println!(
                    "{}: modified {} -- sync with `hoard restore {}`",
                    hoard_name, source, hoard_name
                ),
                DiffSource::Mixed => println!(
                    "{}: mixed changes -- manual intervention recommended (see `hoard diff`)",
                    hoard_name
                ),
                DiffSource::Unknown => println!(
                    "{}: unexpected changes -- manual intervention recommended (see `hoard diff`)",
                    hoard_name
                ),
            },
        }
    }

    Ok(())
}
