use crate::checkers::history::operation::Error as OperationError;
use crate::filters::Error as FilterError;
use thiserror::Error;

mod all_files;
mod diff_files;
mod operation;

//pub(crate) use all_files::AllFilesIter;
pub(crate) use diff_files::{DiffSource, HoardDiffIter, HoardFileDiff};
pub(crate) use crate::hoard_file::HoardFile;
pub(crate) use operation::{OperationIter, OperationType};
 use macros::propagate_error;

mod macros {
    macro_rules! propagate_error {
        ($result:expr) => {
            match $result {
                Ok(val) => val,
                Err(err) => return Some(Err(err)),
            }
        }
    }

    pub(crate) use propagate_error;
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to create diff: {0}")]
    Diff(#[from] FilterError),
    #[error("I/O error occurred: {0}")]
    IO(#[from] std::io::Error),
    #[error("failed to check hoard operations: {0}")]
    Operation(#[from] Box<OperationError>),
}