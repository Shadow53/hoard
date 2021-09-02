//! Provides filters for determining whether a path should be backed up or not.

use std::path::Path;
use crate::config::builder::hoard::Config;
use thiserror::Error;

pub(crate) mod ignore;

/// The [`Filter`] trait provides a common interface for all filters.
pub trait Filter: Sized {
    /// Any errors that may occur while creating a new filter.
    type Error: std::error::Error;
    /// Creates a new instance of something that implements [`Filter`].
    ///
    /// # Errors
    ///
    /// Any errors that may occur while creating the new filter.
    fn new(pile_config: &Config) -> Result<Self, Self::Error>;
    /// Whether or not the file should be kept (backed up).
    fn keep(&self, path: &Path) -> bool;
}

/// Any errors that may occur while filtering.
#[derive(Debug, Error)]
pub enum Error {
    /// An error occurred in the Ignore filter.
    #[error("error occurred in the ignore filter: {0}")]
    Ignore(#[from] ignore::Error),
}

/// A wrapper for all implmented filters.
#[derive(Debug, Clone)]
pub struct Filters {
    ignore: ignore::IgnoreFilter,
}

impl Filter for Filters {
    type Error = Error;

    fn new(pile_config: &Config) -> Result<Self, Self::Error> {
        let ignore = ignore::IgnoreFilter::new(pile_config)?;
        Ok(Self{ ignore })
    }

    fn keep(&self, path: &Path) -> bool {
        self.ignore.keep(path)
    }
}
