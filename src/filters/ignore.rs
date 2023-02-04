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
use glob::Pattern;

use crate::hoard::PileConfig;
use crate::paths::{RelativePath, SystemPath};

use super::Filter;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub(crate) struct IgnoreFilter {
    globs: Vec<Pattern>,
}

impl Filter for IgnoreFilter {
    fn new(pile_config: &PileConfig) -> Self {
        IgnoreFilter {
            globs: pile_config.ignore.clone(),
        }
    }

    #[tracing::instrument(name = "run_ignore_filter", skip(self, _prefix))]
    fn keep(&self, _prefix: &SystemPath, rel_path: &RelativePath) -> bool {
        self.globs.iter().all(|glob| {
            let matches = glob.matches_path(&rel_path.to_path_buf());
            tracing::trace!(
                "{:?} {} glob {:?}",
                rel_path,
                if matches { "matches" } else { "does not match" },
                glob
            );
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
                ..PileConfig::default()
            };
            IgnoreFilter::new(&config)
        };
        let other = {
            let config = PileConfig {
                ignore: vec![Pattern::new("test/**").unwrap()],
                ..PileConfig::default()
            };
            IgnoreFilter::new(&config)
        };
        assert!(format!("{filter:?}").contains("IgnoreFilter"));
        assert_eq!(filter, filter.clone());
        assert_ne!(filter, other);
    }
}
