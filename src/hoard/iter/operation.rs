use super::{HoardDiffIter, HoardFile};
use crate::hoard::iter::{DiffSource, HoardFileDiff};
use crate::hoard::{Direction, Hoard};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OperationType {
    Create(HoardFile),
    Modify(HoardFile),
    Delete(HoardFile),
    Nothing(HoardFile),
}

pub(crate) struct OperationIter {
    iterator: HoardDiffIter,
    direction: Direction,
}

impl OperationIter {
    pub(crate) fn new(
        hoards_root: &Path,
        hoard_name: String,
        hoard: &Hoard,
        direction: Direction,
    ) -> Result<Self, super::Error> {
        let iterator = HoardDiffIter::new(hoards_root, hoard_name, hoard)?;
        Ok(Self {
            iterator,
            direction,
        })
    }
}

impl Iterator for OperationIter {
    type Item = Result<OperationType, super::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // For the purposes of this, Mixed counts for both local (backup) and remote (restore)
        // changes, and Unknown counts as a remote change.
        self.iterator.next().map(|diff| {
            tracing::trace!("found diff: {:?}", diff);
            #[allow(clippy::match_same_arms)]
            let op = match diff? {
                HoardFileDiff::BinaryModified { file, .. }
                | HoardFileDiff::TextModified { file, .. }
                | HoardFileDiff::PermissionsModified { file, .. } => OperationType::Modify(file),
                HoardFileDiff::Created {
                    file, diff_source, ..
                }
                | HoardFileDiff::Recreated {
                    file, diff_source, ..
                } => match (self.direction, diff_source) {
                    (_, DiffSource::Mixed) => OperationType::Create(file),
                    (Direction::Backup, DiffSource::Local) => OperationType::Create(file),
                    (Direction::Backup, DiffSource::Remote | DiffSource::Unknown) => OperationType::Delete(file),
                    (Direction::Restore, DiffSource::Remote | DiffSource::Unknown) => OperationType::Create(file),
                    (Direction::Restore, DiffSource::Local) => OperationType::Delete(file),
                },
                HoardFileDiff::Deleted {
                    file, diff_source, ..
                } => match (self.direction, diff_source) {
                    (_, DiffSource::Mixed) => OperationType::Delete(file),
                    (Direction::Backup, DiffSource::Local)
                    | (Direction::Restore, DiffSource::Remote | DiffSource::Unknown) => OperationType::Delete(file),
                    (Direction::Backup, DiffSource::Remote | DiffSource::Unknown)
                    | (Direction::Restore, DiffSource::Local) => OperationType::Create(file),
                },
                HoardFileDiff::Unchanged(file) => OperationType::Nothing(file),
            };
            Ok(op)
        })
    }
}
