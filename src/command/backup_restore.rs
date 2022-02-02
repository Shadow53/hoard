use crate::checkers::Checkers;
use crate::hoard::{Direction, Hoard};
use std::path::Path;

#[allow(single_use_lifetimes)]
pub(crate) fn run_backup<'a, S: AsRef<str>>(
    hoards_root: &Path,
    hoards: impl IntoIterator<Item = (S, &'a Hoard)> + Clone,
    force: bool,
) -> Result<(), super::Error> {
    backup_or_restore(hoards_root, Direction::Backup, hoards, force)
}

#[allow(single_use_lifetimes)]
pub(crate) fn run_restore<'a, S: AsRef<str>>(
    hoards_root: &Path,
    hoards: impl IntoIterator<Item = (S, &'a Hoard)> + Clone,
    force: bool,
) -> Result<(), super::Error> {
    backup_or_restore(hoards_root, Direction::Restore, hoards, force)
}

#[allow(single_use_lifetimes)]
fn backup_or_restore<'a, S: AsRef<str>>(
    hoards_root: &Path,
    direction: Direction,
    hoards: impl IntoIterator<Item = (S, &'a Hoard)> + Clone,
    force: bool,
) -> Result<(), super::Error> {
    let mut checkers = Checkers::new(hoards_root, hoards.clone(), direction)?;
    if !force {
        checkers.check()?;
    }

    for (name, hoard) in hoards {
        let name = name.as_ref();
        let prefix = hoards_root.join(name);

        match direction {
            Direction::Backup => {
                tracing::info!(hoard = %name, "backing up");
                let _span = tracing::info_span!("backup", hoard = %name).entered();
                hoard
                    .backup(&prefix)
                    .map_err(|error| super::Error::Backup {
                        name: name.to_string(),
                        error,
                    })?;
            }
            Direction::Restore => {
                tracing::info!(hoard = %name, "restoring");
                let _span = tracing::info_span!("restore", hoard = %name).entered();
                hoard
                    .restore(&prefix)
                    .map_err(|error| super::Error::Restore {
                        name: name.to_string(),
                        error,
                    })?;
            }
        }
    }

    checkers.commit_to_disk().map_err(super::Error::Checkers)
}
