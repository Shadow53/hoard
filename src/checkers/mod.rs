//! Types that validate (check) Hoard configurations
//!
//! This module includes a single trait, [`Checker`], and all types that implement it.
//! Currently, that is only the [`LastPaths`](history::last_paths::LastPaths) checker.

pub mod history;

use crate::checkers::history::last_paths::{Error as LastPathsError, LastPaths};
use crate::checkers::history::operation::{Error as OperationError, Operation};
use crate::hoard::{Direction, Hoard};
use std::collections::HashMap;
use std::path::Path;
use thiserror::Error;

/// Trait for validating [`Hoard`]s.
///
/// A [`Checker`] takes a [`Hoard`] and its name (as [`&str`]) as parameters and uses that
/// information plus any internal state to validate that it is safe to operate on that [`Hoard`].
pub trait Checker: Sized {
    /// The error type returned from the check.
    type Error: std::error::Error;
    /// Returns a new instance of the implementing Checker type.
    ///
    /// # Errors
    ///
    /// Any errors that may occur while creating an instance, such as I/O or consistency errors.
    fn new(
        hoard_root: &Path,
        hoard_name: &str,
        hoard: &Hoard,
        direction: Direction,
    ) -> Result<Self, Self::Error>;
    /// Returns an error if it is not safe to operate on the given [`Hoard`].
    ///
    /// # Errors
    ///
    /// Any error that prevents operations on the given [`Hoard`], or any errors that
    /// occur while performing the check.
    fn check(&mut self) -> Result<(), Self::Error>;
    /// Saves any persistent data to disk.
    ///
    /// # Errors
    ///
    /// Generally, any I/O errors that occur while persisting data.
    fn commit_to_disk(self) -> Result<(), Self::Error>;
}

/// Errors that may occur while using [`Checkers`].
#[derive(Debug, Error)]
pub enum Error {
    /// An error occurred while comparing paths for this run to the previous one.
    #[error("error while comparing previous run to current run: {0}")]
    LastPaths(#[from] LastPathsError),
    /// An error occurred while checking against remote operations.
    #[error("error while checking against recent remote operations: {0}")]
    Operation(#[from] OperationError),
}

pub(crate) struct Checkers {
    last_paths: HashMap<String, LastPaths>,
    operations: HashMap<String, Operation>,
}

impl Checkers {
    #[allow(single_use_lifetimes)]
    pub(crate) fn new<'a, S: AsRef<str>>(
        hoards_root: &Path,
        hoards: impl IntoIterator<Item = (S, &'a Hoard)>,
        direction: Direction,
    ) -> Result<Self, Error> {
        let mut last_paths = HashMap::new();
        let mut operations = HashMap::new();

        for (name, hoard) in hoards {
            let name = name.as_ref();
            let lp = LastPaths::new(hoards_root, name, hoard, direction)?;
            let op = Operation::new(hoards_root, name, hoard, direction)?;
            last_paths.insert(name.to_string(), lp);
            operations.insert(name.to_string(), op);
        }

        Ok(Self {
            last_paths,
            operations,
        })
    }

    pub(crate) fn check(&mut self) -> Result<(), Error> {
        let _span = tracing::info_span!("running_checks").entered();
        for last_path in &mut self.last_paths.values_mut() {
            last_path.check()?;
        }
        for operation in self.operations.values_mut() {
            operation.check()?;
        }
        Ok(())
    }

    pub(crate) fn commit_to_disk(self) -> Result<(), Error> {
        let Self {
            last_paths,
            operations,
            ..
        } = self;
        for (_, last_path) in last_paths {
            last_path.commit_to_disk()?;
        }
        for (_, operation) in operations {
            operation.commit_to_disk()?;
        }
        Ok(())
    }
}
