use super::diff_stream;
use crate::checkers::history::operation::ItemOperation;
use crate::hoard::iter::{DiffSource, HoardFileDiff};
use crate::hoard::{Direction, Hoard};
use crate::hoard_item::CachedHoardItem;
use crate::newtypes::HoardName;
use crate::paths::HoardPath;
use futures::{TryStream, TryStreamExt};

/// Stream returning all [`ItemOperation`]s for the given hoard.
///
/// # Errors
///
/// Any errors that may occur while initially creating the stream.
#[allow(clippy::module_name_repetitions)]
#[tracing::instrument]
pub async fn operation_stream(
    hoards_root: &HoardPath,
    hoard_name: HoardName,
    hoard: &Hoard,
    direction: Direction,
) -> Result<impl TryStream<Ok = ItemOperation<CachedHoardItem>, Error = super::Error>, super::Error>
{
    diff_stream(hoards_root, hoard_name, hoard)
        .await
        .map(move |stream| {
            stream.and_then(move |diff| async move {
                tracing::trace!("found diff: {:?}", diff);
                #[allow(clippy::match_same_arms)]
                let op = match diff {
                    HoardFileDiff::BinaryModified { file, .. }
                    | HoardFileDiff::TextModified { file, .. } => ItemOperation::Modify(file),
                    HoardFileDiff::Created {
                        file, diff_source, ..
                    } => match (direction, diff_source) {
                        (_, DiffSource::Mixed) => ItemOperation::Create(file),
                        (Direction::Backup, DiffSource::Local) => ItemOperation::Create(file),
                        (Direction::Backup, DiffSource::Remote | DiffSource::Unknown) => {
                            ItemOperation::Delete(file)
                        }
                        (Direction::Restore, DiffSource::Remote | DiffSource::Unknown) => {
                            ItemOperation::Create(file)
                        }
                        (Direction::Restore, DiffSource::Local) => ItemOperation::Delete(file),
                    },
                    HoardFileDiff::Deleted {
                        file, diff_source, ..
                    } => match (direction, diff_source) {
                        (_, DiffSource::Mixed) => ItemOperation::Delete(file),
                        (Direction::Backup, DiffSource::Local)
                        | (Direction::Restore, DiffSource::Remote | DiffSource::Unknown) => {
                            ItemOperation::Delete(file)
                        }
                        (Direction::Backup, DiffSource::Remote | DiffSource::Unknown)
                        | (Direction::Restore, DiffSource::Local) => ItemOperation::Create(file),
                    },
                    HoardFileDiff::Unchanged(file) => ItemOperation::Nothing(file),
                    HoardFileDiff::Nonexistent(file) => ItemOperation::DoesNotExist(file),
                };
                Ok(op)
            })
        })
}
