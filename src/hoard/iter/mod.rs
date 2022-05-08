//! This module provides async streams of hoard-managed files and associated information.

use thiserror::Error;

pub use all_files::all_files_stream;
pub use diff_files::{changed_diff_only_stream, diff_stream, DiffSource, HoardFileDiff};
pub use operation::operation_stream;

use crate::checkers::history::operation::Error as OperationError;

mod all_files;
mod diff_files;
mod operation;

/// Errors that may occur while using a stream.
#[derive(Debug, Error)]
#[allow(variant_size_differences)]
pub enum Error {
    /// Some I/O error occurred.
    #[error("I/O error occurred: {0}")]
    IO(#[from] tokio::io::Error),
    /// Error occurred while loading operation logs.
    #[error("failed to check hoard operations: {0}")]
    Operation(#[from] Box<OperationError>),
}
