//! Provides filters for determining whether a path should be backed up or not.

use crate::hoard::PileConfig;
use std::path::Path;
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
    fn new(pile_config: &PileConfig) -> Result<Self, Self::Error>;
    /// Whether or not the file should be kept (backed up).
    fn keep(&self, prefix: &Path, path: &Path) -> bool;
}

/// Any errors that may occur while filtering.
#[derive(Debug, Error)]
pub enum Error {
    /// An error occurred in the Ignore filter.
    #[error("error occurred in the ignore filter: {0}")]
    Ignore(#[from] ignore::Error),
}

/// A wrapper for all implmented filters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Filters {
    ignore: ignore::IgnoreFilter,
}

impl Filter for Filters {
    type Error = Error;

    fn new(pile_config: &PileConfig) -> Result<Self, Self::Error> {
        let ignore = ignore::IgnoreFilter::new(pile_config)?;
        Ok(Self { ignore })
    }

    fn keep(&self, prefix: &Path, path: &Path) -> bool {
        let _span = tracing::trace_span!("run_filters", ?prefix, ?path).entered();
        self.ignore.keep(prefix, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hoard_item::ChecksumType;

    #[test]
    fn test_filters_derives() {
        let config = PileConfig {
            checksum_type: ChecksumType::default(),
            encryption: None,
            ignore: vec![glob::Pattern::new("valid/**").unwrap()],
        };
        let filters = Filters::new(&config).expect("config should be valid");
        assert!(format!("{:?}", filters).contains("Filters"));
        assert_eq!(filters.clone().ignore, filters.ignore);
    }
}
