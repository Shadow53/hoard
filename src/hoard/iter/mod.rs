//! This module provides async streams of hoard-managed files and associated information.

use crate::checkers::history::operation::Error as OperationError;
use crate::filters::Error as FilterError;
use thiserror::Error;

mod all_files;
mod diff_files;
mod operation;

pub use all_files::all_files_stream;
pub use diff_files::{DiffSource, diff_stream, changed_diff_only_stream, HoardFileDiff};
pub use operation::operation_stream;

/// Errors that may occur while using a stream.
#[derive(Debug, Error)]
#[allow(variant_size_differences)]
pub enum Error {
    /// Failed to create a [`Filters`](crate::filters::Filters) instance.
    #[error("failed to create filters: {0}")]
    Filter(#[from] FilterError),
    /// Some I/O error occurred.
    #[error("I/O error occurred: {0}")]
    IO(#[from] tokio::io::Error),
    /// Error occurred while loading operation logs.
    #[error("failed to check hoard operations: {0}")]
    Operation(#[from] Box<OperationError>),
}
