//! Provides filters for determining whether a path should be backed up or not.

use thiserror::Error;

use crate::hoard::PileConfig;
use crate::paths::{RelativePath, SystemPath};

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
    fn new(pile_config: &PileConfig) -> Result<Self, Self::Error>;
    /// Whether or not the file should be kept (backed up).
    fn keep(&self, prefix: &SystemPath, path: &RelativePath) -> bool;
}

/// Any errors that may occur while filtering.
#[derive(Debug, Error)]
pub enum Error {
    /// An error occurred in the Ignore filter.
    #[error("error occurred in the ignore filter: {0}")]
    Ignore(#[from] ignore::Error),
}

/// A wrapper for all implmented filters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Filters {
    ignore: ignore::IgnoreFilter,
}

impl Filter for Filters {
    type Error = Error;

    #[tracing::instrument]
    fn new(pile_config: &PileConfig) -> Result<Self, Self::Error> {
        let ignore = ignore::IgnoreFilter::new(pile_config)?;
        Ok(Self { ignore })
    }

    #[tracing::instrument(name = "run_filters")]
    fn keep(&self, prefix: &SystemPath, path: &RelativePath) -> bool {
        self.ignore.keep(prefix, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filters_derives() {
        let config = PileConfig {
            ignore: vec![glob::Pattern::new("valid/**").unwrap()],
            ..PileConfig::default()
        };
        let filters = Filters::new(&config).expect("config should be valid");
        assert!(format!("{:?}", filters).contains("Filters"));
        assert_eq!(filters.clone().ignore, filters.ignore);
    }
}
