use crate::checkers::history::operation::ItemOperation;
use super::HoardDiffIter;
use crate::hoard::iter::{DiffSource, HoardFileDiff};
use crate::hoard::{Direction, Hoard};
use crate::newtypes::HoardName;
use crate::paths::HoardPath;

pub(crate) struct OperationIter {
    iterator: HoardDiffIter,
    direction: Direction,
}

impl OperationIter {
    pub(crate) fn new(
        hoards_root: &HoardPath,
        hoard_name: HoardName,
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
    type Item = Result<ItemOperation, super::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        // For the purposes of this, Mixed counts for both local (backup) and remote (restore)
        // changes, and Unknown counts as a remote change.
        self.iterator.next().map(|diff| {
            tracing::trace!("found diff: {:?}", diff);
            #[allow(clippy::match_same_arms)]
            let op = match diff? {
                HoardFileDiff::BinaryModified { file, .. }
                | HoardFileDiff::TextModified { file, .. } => ItemOperation::Modify(file.into()),
                HoardFileDiff::Created {
                    file, diff_source, ..
                } => match (self.direction, diff_source) {
                    (_, DiffSource::Mixed) => ItemOperation::Create(file.into()),
                    (Direction::Backup, DiffSource::Local) => ItemOperation::Create(file.into()),
                    (Direction::Backup, DiffSource::Remote | DiffSource::Unknown) => {
                        ItemOperation::Delete(file.into())
                    }
                    (Direction::Restore, DiffSource::Remote | DiffSource::Unknown) => {
                        ItemOperation::Create(file.into())
                    }
                    (Direction::Restore, DiffSource::Local) => ItemOperation::Delete(file.into()),
                },
                HoardFileDiff::Deleted {
                    file, diff_source, ..
                } => match (self.direction, diff_source) {
                    (_, DiffSource::Mixed) => ItemOperation::Delete(file.into()),
                    (Direction::Backup, DiffSource::Local)
                    | (Direction::Restore, DiffSource::Remote | DiffSource::Unknown) => {
                        ItemOperation::Delete(file.into())
                    }
                    (Direction::Backup, DiffSource::Remote | DiffSource::Unknown)
                    | (Direction::Restore, DiffSource::Local) => ItemOperation::Create(file.into()),
                },
                HoardFileDiff::Unchanged(file) => ItemOperation::Nothing(file.into()),
                HoardFileDiff::Nonexistent(file) => ItemOperation::DoesNotExist(file.into()),
            };
            Ok(op)
        })
    }
}
