use crate::checkers::history::operation::Error as OperationError;
use crate::filters::Error as FilterError;
use thiserror::Error;

mod all_files;
mod diff_files;

//pub(crate) use all_files::AllFilesIter;
pub(crate) use diff_files::{file_diffs, DiffSource, HoardDiff};

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to create diff: {0}")]
    Diff(#[from] FilterError),
    #[error("I/O error occurred: {0}")]
    IO(#[from] std::io::Error),
    #[error("failed to check hoard operations: {0}")]
    Operation(#[from] OperationError),
}
