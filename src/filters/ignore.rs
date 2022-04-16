use crate::hoard::PileConfig;
/// Provides a [`Filter`] based on glob ignore patterns.
///
/// To use this filter, add an list of glob patterns to `ignore` under `config`. For example:
///
/// ```ignore
/// [config]
///     ignore = ["some*glob"]
/// ```
///
/// This can be put under global, hoard, or pile scope.
use glob::{Pattern, PatternError};

use super::Filter;
use crate::paths::{RelativePath, SystemPath};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// An invalid glob was provided in the configuration file
    #[error("invalid glob pattern \"{pattern}\": {error}")]
    InvalidGlob {
        pattern: String,
        #[source]
        error: PatternError,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub(crate) struct IgnoreFilter {
    globs: Vec<Pattern>,
}

impl Filter for IgnoreFilter {
    type Error = Error;

    fn new(pile_config: &PileConfig) -> Result<Self, Self::Error> {
        Ok(IgnoreFilter {
            globs: pile_config.ignore.clone(),
        })
    }

    fn keep(&self, prefix: &SystemPath, rel_path: &RelativePath) -> bool {
        let _span = tracing::trace_span!("ignore_filter", ?prefix, ?rel_path).entered();
        self.globs.iter().all(|glob| {
            let matches = glob.matches_path(&rel_path.to_path_buf());
            tracing::trace!("{:?} matches glob {:?}: {}", rel_path, glob, matches);
            !matches
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_derives() {
        let filter = {
            let config = PileConfig {
                ignore: vec![Pattern::new("testing/**").unwrap()],
                .. PileConfig::default()
            };
            IgnoreFilter::new(&config).expect("filter should be valid")
        };
        let other = {
            let config = PileConfig {
                ignore: vec![Pattern::new("test/**").unwrap()],
                .. PileConfig::default()
            };
            IgnoreFilter::new(&config).expect("filter should be valid")
        };
        assert!(format!("{:?}", filter).contains("IgnoreFilter"));
        assert_eq!(filter, filter.clone());
        assert_ne!(filter, other);
    }
}
