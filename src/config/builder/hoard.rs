//! This module contains definitions useful for working directly with [`Hoard`]s.
//!
//! A [`Hoard`] is a collection of at least one [`Pile`], where a [`Pile`] is a single file
//! or directory that may appear in different locations on a system depending on that system's
//! configuration. The path used is determined by the most specific match in the *environment
//! condition*, which is a string like `foo|bar|baz` where `foo`, `bar`, and `baz` are the
//! names of [`Environment`](super::environment::Environment)s defined in the configuration file.
//! All environments in the condition must match the current system for its matching path to be
//! used.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::builder::envtrie::{EnvTrie, Error as TrieError};
use crate::env_vars::{Error as EnvError, PathWithEnv};
use crate::hoard::PileConfig;
use crate::newtypes::{EnvironmentName, EnvironmentString, NonEmptyPileName};

type ConfigMultiple = crate::config::hoard::MultipleEntries;
type ConfigSingle = crate::config::hoard::Pile;
type ConfigHoard = crate::config::hoard::Hoard;

/// Errors that may occur while processing a [`Builder`](super::Builder) [`Hoard`] into and
/// [`Config`](crate::config::Config) [`Hoard`](crate::hoard::Hoard).
#[derive(Debug, Error)]
pub enum Error {
    /// Error while evaluating a [`Pile`]'s [`EnvTrie`].
    #[error("error while processing environment requirements: {0}")]
    EnvTrie(#[from] TrieError),
    /// Error while expanding environment variables in a path.
    #[error("error while expanding environment variables in path: {0}")]
    ExpandEnv(#[from] EnvError),
}

/// A single pile in the hoard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pile {
    /// Configuration specific to this pile.
    ///
    /// Will be merged with higher-level configuration. If no configuration is specified
    /// (i.e., merging results in `None`), a default configuration will be used.
    pub config: Option<PileConfig>,
    /// Mapping of environment strings to a string path that may contain environment variables.
    ///
    /// See [`PathWithEnv`] for more on path format.
    #[serde(flatten)]
    pub items: BTreeMap<EnvironmentString, PathWithEnv>,
}

impl Pile {
    #[tracing::instrument(level = "debug", name = "process_pile")]
    fn process_with(
        self,
        envs: &BTreeMap<EnvironmentName, bool>,
        exclusivity: &[Vec<EnvironmentName>],
    ) -> Result<ConfigSingle, Error> {
        let Pile { config, items } = self;
        let trie = EnvTrie::new(&items, exclusivity)?;
        let path = trie
            .get_path(envs)?
            .cloned()
            .map(PathWithEnv::process)
            .transpose()?;

        Ok(ConfigSingle {
            config: config.unwrap_or_default(),
            path,
        })
    }

    pub(crate) fn layer_config(&mut self, config: Option<&PileConfig>) {
        PileConfig::layer_options(&mut self.config, config);
    }
}

/// A set of multiple related piles (i.e. in a single hoard).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct MultipleEntries {
    /// Any custom configuration that applies to all contained files.
    ///
    /// If `None`, a default configuration will be used during processing.
    pub config: Option<PileConfig>,
    /// A mapping of pile name to not-yet-processed [`Pile`]s.
    #[serde(flatten)]
    pub items: BTreeMap<NonEmptyPileName, Pile>,
}

impl MultipleEntries {
    #[tracing::instrument(level = "debug", name = "process_multiple_entries")]
    fn process_with(
        self,
        envs: &BTreeMap<EnvironmentName, bool>,
        exclusivity: &[Vec<EnvironmentName>],
    ) -> Result<ConfigMultiple, super::Error> {
        let MultipleEntries { config, items } = self;
        let items = items
            .into_iter()
            .map(|(pile, mut entry)| {
                tracing::debug!(%pile, "processing pile");
                entry.layer_config(config.as_ref());
                let entry = entry.process_with(envs, exclusivity).map_err(Error::from)?;
                Ok((pile, entry))
            })
            .collect::<Result<_, super::Error>>()?;

        Ok(ConfigMultiple { piles: items })
    }

    pub(crate) fn layer_config(&mut self, config: Option<&PileConfig>) {
        PileConfig::layer_options(&mut self.config, config);
    }
}

/// A definition of a Hoard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Hoard {
    /// A single anonymous [`Pile`].
    Single(Pile),
    /// Multiple named [`Pile`]s.
    Multiple(MultipleEntries),
}

impl Hoard {
    /// Resolve with path(s) to use for the `Hoard`.
    ///
    /// Uses the provided information to determine which environment combination is the best match
    /// for each [`Pile`] and thus which path to use for each one.
    ///
    /// # Errors
    ///
    /// Any [`enum@Error`] that occurs while evaluating the `Hoard`.
    #[tracing::instrument(level = "debug", name = "process_hoard")]
    pub fn process_with(
        self,
        envs: &BTreeMap<EnvironmentName, bool>,
        exclusivity: &[Vec<EnvironmentName>],
    ) -> Result<crate::config::hoard::Hoard, super::Error> {
        match self {
            Hoard::Single(single) => {
                tracing::debug!("processing anonymous pile");
                Ok(ConfigHoard::Anonymous(
                    single
                        .process_with(envs, exclusivity)
                        .map_err(super::Error::from)?,
                ))
            }
            Hoard::Multiple(multiple) => {
                tracing::debug!("processing named pile(s)");
                Ok(ConfigHoard::Named(
                    multiple.process_with(envs, exclusivity)?,
                ))
            }
        }
    }

    pub(crate) fn layer_config(&mut self, config: Option<&PileConfig>) {
        match self {
            Hoard::Single(pile) => pile.layer_config(config),
            Hoard::Multiple(multi) => multi.layer_config(config),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::hoard::pile_config::{
        AsymmetricEncryption, Config as PileConfig, Encryption, SymmetricEncryption,
    };

    use super::*;

    mod process {
        use std::path::PathBuf;

        use maplit::btreemap;

        use crate::hoard::Pile as RealPile;
        use crate::paths::SystemPath;

        use super::*;

        #[test]
        fn env_vars_are_expanded() {
            let pile = Pile {
                config: None,
                #[cfg(unix)]
                items: btreemap! {
                    "foo".parse().unwrap() => "${HOME}/something".into()
                },
                #[cfg(windows)]
                items: btreemap! {
                    "foo".parse().unwrap() => "${USERPROFILE}/something".into()
                },
            };

            #[cfg(unix)]
            let home = std::env::var("HOME").expect("failed to read $HOME");
            #[cfg(windows)]
            let home = std::env::var("USERPROFILE").expect("failed to read $USERPROFILE");
            let expected = RealPile {
                config: PileConfig::default(),
                path: Some(
                    SystemPath::try_from(PathBuf::from(format!("{}/something", home))).unwrap(),
                ),
            };

            let envs = btreemap! { "foo".parse().unwrap() =>  true };
            let result = pile
                .process_with(&envs, &[])
                .expect("pile should process without issues");

            assert_eq!(result, expected);
        }
    }

    mod serde {
        use maplit::btreemap;
        use serde_test::{assert_de_tokens_error, assert_tokens, Token};

        use super::*;

        #[test]
        fn single_entry_no_config() {
            let hoard = Hoard::Single(Pile {
                config: None,
                items: btreemap! {
                    "bar_env|foo_env".parse().unwrap() => "/some/path".into()
                },
            });

            assert_tokens(
                &hoard,
                &[
                    Token::Map { len: None },
                    Token::Str("config"),
                    Token::None,
                    Token::Str("bar_env|foo_env"),
                    Token::Str("/some/path"),
                    Token::MapEnd,
                ],
            );
        }

        #[test]
        fn single_entry_with_config() {
            let hoard = Hoard::Single(Pile {
                config: Some(PileConfig {
                    encryption: Some(Encryption::Asymmetric(AsymmetricEncryption {
                        public_key: "public key".to_string(),
                    })),
                    ..PileConfig::default()
                }),
                items: btreemap! {
                    "bar_env|foo_env".parse().unwrap() => "/some/path".into()
                },
            });

            assert_tokens(
                &hoard,
                &[
                    Token::Map { len: None },
                    Token::Str("config"),
                    Token::Some,
                    Token::Struct {
                        name: "Config",
                        len: 5,
                    },
                    Token::Str("hash_algorithm"),
                    Token::None,
                    Token::Str("encrypt"),
                    Token::Some,
                    Token::Struct {
                        name: "AsymmetricEncryption",
                        len: 2,
                    },
                    Token::Str("type"),
                    Token::Str("asymmetric"),
                    Token::Str("public_key"),
                    Token::Str("public key"),
                    Token::StructEnd,
                    Token::Str("ignore"),
                    Token::Seq { len: Some(0) },
                    Token::SeqEnd,
                    Token::Str("file_permissions"),
                    Token::None,
                    Token::Str("folder_permissions"),
                    Token::None,
                    Token::StructEnd,
                    Token::Str("bar_env|foo_env"),
                    Token::Str("/some/path"),
                    Token::MapEnd,
                ],
            );
        }

        #[test]
        fn multiple_entry_no_config() {
            let hoard = Hoard::Multiple(MultipleEntries {
                config: None,
                items: btreemap! {
                    "item1".parse().unwrap() => Pile {
                        config: None,
                        items: btreemap! {
                            "bar_env|foo_env".parse().unwrap() => "/some/path".into()
                        }
                    },
                },
            });

            assert_tokens(
                &hoard,
                &[
                    Token::Map { len: None },
                    Token::Str("config"),
                    Token::None,
                    Token::Str("item1"),
                    Token::Map { len: None },
                    Token::Str("config"),
                    Token::None,
                    Token::Str("bar_env|foo_env"),
                    Token::Str("/some/path"),
                    Token::MapEnd,
                    Token::MapEnd,
                ],
            );
        }

        #[test]
        fn multiple_entry_with_config() {
            let hoard = Hoard::Multiple(MultipleEntries {
                config: Some(PileConfig {
                    encryption: Some(Encryption::Symmetric(SymmetricEncryption::Password(
                        "correcthorsebatterystaple".into(),
                    ))),
                    ..PileConfig::default()
                }),
                items: btreemap! {
                    "item1".parse().unwrap() => Pile {
                        config: None,
                        items: btreemap! {
                            "bar_env|foo_env".parse().unwrap() => "/some/path".into()
                        }
                    },
                },
            });

            assert_tokens(
                &hoard,
                &[
                    Token::Map { len: None },
                    Token::Str("config"),
                    Token::Some,
                    Token::Struct {
                        name: "Config",
                        len: 5,
                    },
                    Token::Str("hash_algorithm"),
                    Token::None,
                    Token::Str("encrypt"),
                    Token::Some,
                    Token::Map { len: Some(2) },
                    Token::Str("type"),
                    Token::Str("symmetric"),
                    Token::Str("password"),
                    Token::Str("correcthorsebatterystaple"),
                    Token::MapEnd,
                    Token::Str("ignore"),
                    Token::Seq { len: Some(0) },
                    Token::SeqEnd,
                    Token::Str("file_permissions"),
                    Token::None,
                    Token::Str("folder_permissions"),
                    Token::None,
                    Token::StructEnd,
                    Token::Str("item1"),
                    Token::Map { len: None },
                    Token::Str("config"),
                    Token::None,
                    Token::Str("bar_env|foo_env"),
                    Token::Str("/some/path"),
                    Token::MapEnd,
                    Token::MapEnd,
                ],
            );
        }

        #[test]
        fn test_invalid_glob() {
            assert_de_tokens_error::<PileConfig>(
                &[
                    Token::Struct {
                        name: "Config",
                        len: 5,
                    },
                    Token::Str("hash_algorithm"),
                    Token::None,
                    Token::Str("encrypt"),
                    Token::None,
                    Token::Str("file_permissions"),
                    Token::None,
                    Token::Str("folder_permissions"),
                    Token::None,
                    Token::Str("ignore"),
                    Token::Seq { len: Some(2) },
                    Token::Str("**/valid*"),
                    Token::Str("invalid**"),
                    Token::SeqEnd,
                    Token::StructEnd,
                ],
                "Pattern syntax error near position 6: recursive wildcards must form a single path component",
            );
        }

        #[test]
        fn test_valid_globs() {
            let config = PileConfig {
                ignore: vec![
                    glob::Pattern::new("**/valid*").unwrap(),
                    glob::Pattern::new("*/also_valid/**").unwrap(),
                ],
                ..PileConfig::default()
            };

            assert_tokens::<PileConfig>(
                &config,
                &[
                    Token::Struct {
                        name: "Config",
                        len: 5,
                    },
                    Token::Str("hash_algorithm"),
                    Token::None,
                    Token::Str("encrypt"),
                    Token::None,
                    Token::Str("ignore"),
                    Token::Seq { len: Some(2) },
                    Token::Str("**/valid*"),
                    Token::Str("*/also_valid/**"),
                    Token::SeqEnd,
                    Token::Str("file_permissions"),
                    Token::None,
                    Token::Str("folder_permissions"),
                    Token::None,
                    Token::StructEnd,
                ],
            );
        }
    }
}
