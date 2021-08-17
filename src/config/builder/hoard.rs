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
use crate::env_vars::{expand_env_in_path, Error as EnvError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
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
    /// Error while expanding environment variables in a path.
    #[error("error while expanding environment variables in path: {0}")]
    ExpandEnv(#[from] EnvError),
}

/// Configuration for symmetric (password) encryption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SymmetricEncryption {
    /// Raw password.
    #[serde(rename = "encrypt_pass")]
    Password(String),
    /// Command whose first line of output to stdout is the password.
    #[serde(rename = "encrypt_pass_cmd")]
    PasswordCmd(Vec<String>),
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
    items: BTreeMap<String, String>,
}

impl Pile {
    fn process_with(
        self,
        envs: &BTreeMap<String, bool>,
        exclusivity: &[Vec<String>],
    ) -> Result<ConfigSingle, Error> {
        let _span = tracing::debug_span!(
            "process_pile",
            pile = ?self
        )
        .entered();

        let Pile { config, items } = self;
        let trie = EnvTrie::new(&items, exclusivity)?;
        let path = trie.get_path(envs)?
            .map(expand_env_in_path)
            .transpose()?;

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
                tracing::debug!(%pile, "processing pile");
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
                tracing::debug!("processing anonymous pile");
                Ok(ConfigHoard::Single(single.process_with(envs, exclusivity)?))
            }
            Hoard::Multiple(multiple) => {
                tracing::debug!("processing named pile(s)");
                Ok(ConfigHoard::Multiple(
                    multiple.process_with(envs, exclusivity)?,
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod process {
        use super::*;
        use crate::config::hoard::Pile as ConfigPile;
        use maplit::btreemap;
        use std::path::PathBuf;

        #[test]
        fn env_vars_are_expanded() {
            let pile = Pile {
                config: None,
                items: btreemap! {
                    "foo".into() => "${HOME}/something".into()
                },
            };

            let home = std::env::var("HOME").expect("failed to read $HOME");
            let expected = ConfigPile {
                config: None,
                path: Some(PathBuf::from(format!("{}/something", home))),
            };

            let envs = btreemap! { "foo".into() =>  true };
            let result = pile
                .process_with(&envs, &[])
                .expect("pile should process without issues");

            assert_eq!(result, expected);
        }
    }

    mod serde {
        use super::*;
        use maplit::btreemap;
        use serde_test::{assert_tokens, Token};

        #[test]
        fn single_entry_no_config() {
            let hoard = Hoard::Single(Pile {
                config: None,
                items: btreemap! {
                    "bar_env|foo_env".to_string() => "/some/path".to_string()
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
                    "bar_env|foo_env".to_string() => "/some/path".to_string()
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
        fn multiple_entry_no_config() {
            let hoard = Hoard::Multiple(MultipleEntries {
                config: None,
                items: btreemap! {
                    "item1".to_string() => Pile {
                        config: None,
                        items: btreemap! {
                            "bar_env|foo_env".to_string() => "/some/path".to_string()
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
                config: Some(Config {
                    encryption: Encryption::Symmetric(SymmetricEncryption::Password(
                        "correcthorsebatterystaple".into(),
                    )),
                }),
                items: btreemap! {
                    "item1".to_string() => Pile {
                        config: None,
                        items: btreemap! {
                            "bar_env|foo_env".to_string() => "/some/path".to_string()
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
                    Token::Map { len: None },
                    Token::Str("encrypt"),
                    Token::Str("symmetric"),
                    Token::Str("encrypt_pass"),
                    Token::Str("correcthorsebatterystaple"),
                    Token::MapEnd,
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
