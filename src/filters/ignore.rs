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
use crate::config::builder::hoard::Config;

use super::Filter;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    /// An invalid glob was provided in the configuration file
    #[error("invalid glob pattern \"{pattern}\": {error}")]
    InvalidGlob {
        pattern: String,
        #[source]
        error: PatternError,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct IgnoreFilter {
    globs: Vec<Pattern>
}

impl Filter for IgnoreFilter {
    type Error = Error;

    fn new(pile_config: &Config) -> Result<Self, Self::Error> {
        pile_config.ignore
            .iter()
            .map(|pattern| {
                Pattern::new(pattern).map_err(|error| {
                    Error::InvalidGlob {
                        pattern: pattern.clone(),
                        error,
                    }
                })
            })
            .collect::<Result<_, _>>()
            .map(|globs| IgnoreFilter { globs })
    }

    fn keep(&self, path: &std::path::Path) -> bool {
        self.globs.iter().all(|glob| !glob.matches_path(path))
    }
}
