use std::path::Path;
use crate::hoard::{Direction, Hoard};
use crate::hoard::iter::{DiffSource, HoardFileDiff};
use super::{HoardFile, HoardDiffIter};

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
    pub(crate) fn new(hoards_root: &Path, hoard_name: String, hoard: &Hoard, direction: Direction) -> Result<Self, super::Error> {
        let iterator = HoardDiffIter::new(hoards_root, hoard_name, hoard)?;
        Ok(Self{ iterator, direction })
    }
}

impl Iterator for OperationIter {
    type Item = Result<OperationType, super::Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.iterator.next().map(|diff| {
            let op = match diff? {
                HoardFileDiff::BinaryModified { file, .. } => OperationType::Modify(file),
                HoardFileDiff::TextModified { file, .. } => OperationType::Modify(file),
                HoardFileDiff::PermissionsModified { file, .. } => OperationType::Modify(file),
                HoardFileDiff::Created { file, diff_source, .. } | HoardFileDiff::Recreated { file, diff_source, .. } => match (self.direction, diff_source) {
                    (Direction::Backup, DiffSource::Local | DiffSource::Mixed) => OperationType::Create(file),
                    (Direction::Backup, DiffSource::Remote | DiffSource::Unknown) => OperationType::Delete(file),
                    (Direction::Restore, DiffSource::Local) => OperationType::Delete(file),
                    (Direction::Restore, DiffSource::Remote | DiffSource::Mixed | DiffSource::Unknown) => OperationType::Create(file),
                },
                HoardFileDiff::Deleted { file, diff_source, .. } => match (self.direction, diff_source) {
                    (Direction::Backup, DiffSource::Local | DiffSource::Mixed) => OperationType::Delete(file),
                    (Direction::Backup, DiffSource::Remote | DiffSource::Unknown) => OperationType::Create(file),
                    (Direction::Restore, DiffSource::Local) => OperationType::Create(file),
                    (Direction::Restore, DiffSource::Remote | DiffSource::Mixed | DiffSource::Unknown) => OperationType::Delete(file),
                },
                HoardFileDiff::Unchanged(file) => OperationType::Nothing(file),
            };
            Ok(op)
        })
    }
}