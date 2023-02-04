//! Provides filters for determining whether a path should be backed up or not.

use crate::hoard::PileConfig;
use crate::paths::{RelativePath, SystemPath};

pub(crate) mod ignore;

/// The [`Filter`] trait provides a common interface for all filters.
pub trait Filter: Sized {
    /// Creates a new instance of something that implements [`Filter`].
    ///
    /// # Errors
    ///
    /// Any errors that may occur while creating the new filter.
    fn new(pile_config: &PileConfig) -> Self;
    /// Whether or not the file should be kept (backed up).
    fn keep(&self, prefix: &SystemPath, path: &RelativePath) -> bool;
}

/// A wrapper for all implmented filters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Filters {
    ignore: ignore::IgnoreFilter,
}

impl Filter for Filters {
    #[tracing::instrument]
    fn new(pile_config: &PileConfig) -> Self {
        let ignore = ignore::IgnoreFilter::new(pile_config);
        Self { ignore }
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
        let filters = Filters::new(&config);
        assert!(format!("{filters:?}").contains("Filters"));
        assert_eq!(filters.clone().ignore, filters.ignore);
    }
}
