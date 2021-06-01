//! This module contains definitions useful for working directly with [`Hoard`]s.
//!
//! A [`Hoard`] is a collection of at least one [`Pile`], where a [`Pile`] is a single file
//! or directory that may appear in different locations on a system depending on that system's
//! configuration. The path used is determined by the most specific match in the *environment
//! condition*, which is a string like `foo|bar|baz` where `foo`, `bar`, and `baz` are the
//! names of [`Environment`](super::environment::Environment)s defined in the configuration file.
//! All environments in the condition must match the current system for its matching path to be
//! used.

use crate::config::builder::envtrie::{EnvTrie, Error as TrieError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

type ConfigMultiple = crate::config::hoard::MultipleEntries;
type ConfigSingle = crate::config::hoard::Pile;
type ConfigHoard = crate::config::hoard::Hoard;

/// Errors that may occur while processing a [`Builder`](super::Builder) [`Hoard`] into a [`Config`]
/// [`Hoard`](crate::config::hoard::Hoard).
#[derive(Debug, Error)]
pub enum Error {
    /// Error while evaluating a [`Pile`]'s [`EnvTrie`].
    #[error("error while processing environment requirements: {0}")]
    EnvTrie(#[from] TrieError),
}

/// Configuration for symmetric (password) encryption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymmetricEncryption {
    password: String,
    password_cmd: Vec<String>,
}

/// Configuration for asymmetric (public key) encryption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsymmetricEncryption {
    #[serde(rename = "encrypt_pub_key")]
    public_key: String,
}

/// Configuration for hoard/pile encryption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "encrypt", rename_all = "snake_case")]
pub enum Encryption {
    /// Symmetric encryption.
    Symmetric(SymmetricEncryption),
    /// Asymmetric encryption.
    Asymmetric(AsymmetricEncryption),
}

/// Hoard/Pile configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(flatten)]
    encryption: Encryption,
}

/// A single pile in the hoard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pile {
    config: Option<Config>,
    #[serde(flatten)]
    items: BTreeMap<String, PathBuf>,
}

impl Pile {
    fn process_with(
        self,
        envs: &BTreeMap<String, bool>,
        exclusivity: &[Vec<String>],
    ) -> Result<ConfigSingle, Error> {
        let Pile { config, items } = self;
        let trie = EnvTrie::new(&items, exclusivity)?;
        let path = trie.get_path(envs).map(Path::to_owned);

        Ok(ConfigSingle { config, path })
    }
}

/// A set of multiple related piles (i.e. in a single hoard).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultipleEntries {
    config: Option<Config>,
    #[serde(flatten)]
    items: BTreeMap<String, Pile>,
}

impl MultipleEntries {
    fn process_with(
        self,
        envs: &BTreeMap<String, bool>,
        exclusivity: &[Vec<String>],
    ) -> Result<ConfigMultiple, Error> {
        let MultipleEntries { config, items } = self;
        let items = items
            .into_iter()
            .map(|(pile, entry)| {
                log::trace!("Processing pile \"{}\"", pile);
                let mut entry = entry.process_with(envs, exclusivity)?;
                entry.config = entry.config.or_else(|| config.clone());
                Ok((pile, entry))
            })
            .collect::<Result<_, Error>>()?;

        Ok(ConfigMultiple { piles: items })
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
    pub fn process_with(
        self,
        envs: &BTreeMap<String, bool>,
        exclusivity: &[Vec<String>],
    ) -> Result<crate::config::hoard::Hoard, Error> {
        match self {
            Hoard::Single(single) => {
                Ok(ConfigHoard::Single(single.process_with(envs, exclusivity)?))
            }
            Hoard::Multiple(multiple) => Ok(ConfigHoard::Multiple(
                multiple.process_with(envs, exclusivity)?,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod serde {
        use super::*;
        use maplit::btreemap;
        use serde_test::{assert_tokens, Token};

        #[test]
        fn single_entry_no_config() {
            let hoard = Hoard::Single(Pile {
                config: None,
                items: btreemap! {
                    "bar_env|foo_env".to_string() => PathBuf::from("/some/path")
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
                config: Some(Config {
                    encryption: Encryption::Asymmetric(AsymmetricEncryption {
                        public_key: "public key".to_string(),
                    }),
                }),
                items: btreemap! {
                    "bar_env|foo_env".to_string() => PathBuf::from("/some/path")
                },
            });

            assert_tokens(
                &hoard,
                &[
                    Token::Map { len: None },
                    Token::Str("config"),
                    Token::Some,
                    Token::Map { len: None },
                    Token::Str("encrypt"),
                    Token::Str("asymmetric"),
                    Token::Str("encrypt_pub_key"),
                    Token::Str("public key"),
                    Token::MapEnd,
                    Token::Str("bar_env|foo_env"),
                    Token::Str("/some/path"),
                    Token::MapEnd,
                ],
            );
        }

        #[test]
        fn no_config_multiple_entry() {
            let hoard = Hoard::Multiple(MultipleEntries {
                config: None,
                items: btreemap! {
                    "item1".to_string() => Pile {
                        config: None,
                        items: btreemap! {
                            "bar_env|foo_env".to_string() => PathBuf::from("/some/path")
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
    }
}
