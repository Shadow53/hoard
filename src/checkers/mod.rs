//! Types that validate (check) Hoard configurations
//!
//! This module includes a single trait, [`Checker`], and all types that implement it.
//! Currently, that is only the [`LastPaths`](history::last_paths::LastPaths) checker.

use std::collections::HashMap;

use thiserror::Error;

use crate::checkers::history::last_paths::{Error as LastPathsError, LastPaths};
use crate::checkers::history::operation::{Error as OperationError, Operation};
use crate::hoard::{Direction, Hoard};
use crate::newtypes::HoardName;
use crate::paths::HoardPath;

pub mod history;

/// Trait for validating [`Hoard`]s.
///
/// A [`Checker`] takes a [`Hoard`] and its name (as [`&str`]) as parameters and uses that
/// information plus any internal state to validate that it is safe to operate on that [`Hoard`].
#[async_trait::async_trait(? Send)]
pub trait Checker: Sized + Unpin {
    /// The error type returned from the check.
    type Error: std::error::Error;
    /// Returns a new instance of the implementing Checker type.
    ///
    /// # Errors
    ///
    /// Any errors that may occur while creating an instance, such as I/O or consistency errors.
    async fn new(
        hoard_root: &HoardPath,
        hoard_name: &HoardName,
        hoard: &Hoard,
        direction: Direction,
    ) -> Result<Self, Self::Error>;
    /// Returns an error if it is not safe to operate on the given [`Hoard`].
    ///
    /// # Errors
    ///
    /// Any error that prevents operations on the given [`Hoard`], or any errors that
    /// occur while performing the check.
    async fn check(&mut self) -> Result<(), Self::Error>;
    /// Saves any persistent data to disk.
    ///
    /// # Errors
    ///
    /// Generally, any I/O errors that occur while persisting data.
    async fn commit_to_disk(self) -> Result<(), Self::Error>;
}

/// Errors that may occur while using [`Checkers`].
#[derive(Debug, Error)]
#[allow(variant_size_differences)]
pub enum Error {
    /// An error occurred while comparing paths for this run to the previous one.
    #[error("error while comparing previous run to current run: {0}")]
    LastPaths(#[from] LastPathsError),
    /// An error occurred while checking against remote operations.
    #[error("error while checking against recent remote operations: {0}")]
    Operation(#[from] OperationError),
}

/// A wrapper type for running all implemented [`Checker`] types at once.
#[derive(Debug, Clone, PartialEq)]
pub struct Checkers {
    last_paths: HashMap<HoardName, LastPaths>,
    operations: HashMap<HoardName, Operation>,
}

impl Checkers {
    #[allow(single_use_lifetimes)]
    #[tracing::instrument(level = "debug", name = "checkers_new", skip(hoards))]
    pub(crate) async fn new<'a>(
        hoards_root: &HoardPath,
        hoards: impl IntoIterator<Item = (&'a HoardName, &'a Hoard)>,
        direction: Direction,
    ) -> Result<Self, Error> {
        let mut last_paths = HashMap::new();
        let mut operations = HashMap::new();

        for (name, hoard) in hoards {
            tracing::debug!(%name, ?hoard, "processing hoard");
            let lp = LastPaths::new(hoards_root, name, hoard, direction).await?;
            let op = Operation::new(hoards_root, name, hoard, direction).await?;
            last_paths.insert(name.clone(), lp);
            operations.insert(name.clone(), op);
        }

        Ok(Self {
            last_paths,
            operations,
        })
    }

    #[tracing::instrument(level = "debug", name = "checkers_check", skip_all)]
    pub(crate) async fn check(&mut self) -> Result<(), Error> {
        for last_path in &mut self.last_paths.values_mut() {
            last_path.check().await?;
        }
        for operation in self.operations.values_mut() {
            operation.check().await?;
        }
        Ok(())
    }

    #[tracing::instrument(level = "debug", name = "checkers_commit", skip_all)]
    pub(crate) async fn commit_to_disk(self) -> Result<(), Error> {
        let Self {
            last_paths,
            operations,
            ..
        } = self;
        for (_, last_path) in last_paths {
            last_path.commit_to_disk().await?;
        }
        for (_, operation) in operations {
            operation.commit_to_disk().await?;
        }
        Ok(())
    }

    pub(crate) fn get_operation_for(&self, hoard_name: &HoardName) -> Option<&Operation> {
        self.operations.get(hoard_name)
    }
}
