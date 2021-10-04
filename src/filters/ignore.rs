use crate::config::builder::hoard::Config;
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

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct IgnoreFilter {
    globs: Vec<Pattern>,
}

impl Filter for IgnoreFilter {
    type Error = Error;

    fn new(pile_config: &Config) -> Result<Self, Self::Error> {
        pile_config
            .ignore
            .iter()
            .map(|pattern| {
                Pattern::new(pattern).map_err(|error| Error::InvalidGlob {
                    pattern: pattern.clone(),
                    error,
                })
            })
            .collect::<Result<_, _>>()
            .map(|globs| IgnoreFilter { globs })
    }

    fn keep(&self, path: &std::path::Path) -> bool {
        self.globs.iter().all(|glob| !glob.matches_path(path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn test_invalid_glob() {
        let config = Config { encryption: None, ignore: vec!["invalid**".to_string()] };
        let err = IgnoreFilter::new(&config).expect_err("glob pattern should be invalid");
        let Error::InvalidGlob { pattern, .. } = err;
        assert_eq!(&pattern, config.ignore.first().unwrap());
    }

    #[test]
    fn test_error_derives() {
        let config = Config { encryption: None, ignore: vec!["invalid**".to_string()] };
        let err = IgnoreFilter::new(&config).expect_err("glob pattern should be invalid");
        assert!(format!("{:?}", err).contains("InvalidGlob"));
        assert!(err.source().is_some());
        assert!(err.to_string().contains("invalid glob pattern"));
    }

    #[test]
    fn test_filter_derives() {
        let filter = {
            let config = Config { encryption: None, ignore: vec!["testing/**".to_string()] };
            IgnoreFilter::new(&config).expect("filter should be valid")
        };
        let other = {
            let config = Config { encryption: None, ignore: vec!["test/**".to_string()] };
            IgnoreFilter::new(&config).expect("filter should be valid")
        };
        assert!(format!("{:?}", filter).contains("IgnoreFilter"));
        assert_eq!(filter, filter.clone());
        assert_ne!(filter, other);
    }
}
