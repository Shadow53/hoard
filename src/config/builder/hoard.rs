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
use std::collections::HashMap;
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
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The [`Encryption`] configuration for a pile.
    #[serde(default)]
    pub encryption: Option<Encryption>,
    /// A list of glob patterns matching files to ignore.
    #[serde(default)]
    pub ignore: Vec<String>,
}

impl Config {
    /// Merge the `other` configuration with this one, preferring the content of this one, when
    /// appropriate.
    fn layer(&mut self, other: &Self) {
        // Overlay a more general encryption config, if a specific one doesn't exist.
        if self.encryption.is_none() {
            self.encryption = other.encryption.clone();
        }

        // Merge ignore lists.
        self.ignore.extend(other.ignore.clone());
        self.ignore.sort_unstable();
        self.ignore.dedup();
    }

    /// Layer the `general` config with the `specific` one, modifying the `specific` one in place.
    pub fn layer_options(specific: &mut Option<Self>, general: Option<&Self>) {
        if let Some(general) = general {
            match specific {
                None => {
                    specific.replace(general.clone());
                }
                Some(this_config) => this_config.layer(general),
            }
        }
    }
}

/// A single pile in the hoard.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pile {
    config: Option<Config>,
    #[serde(flatten)]
    items: HashMap<String, String>,
}

impl Pile {
    fn process_with(
        self,
        envs: &HashMap<String, bool>,
        exclusivity: &[Vec<String>],
    ) -> Result<ConfigSingle, Error> {
        let _span = tracing::debug_span!(
            "process_pile",
            pile = ?self
        )
        .entered();

        let Pile { config, items } = self;
        let trie = EnvTrie::new(&items, exclusivity)?;
        let path = trie.get_path(envs)?.map(expand_env_in_path).transpose()?;

        Ok(ConfigSingle { config, path })
    }

    pub(crate) fn layer_config(&mut self, config: Option<&Config>) {
        Config::layer_options(&mut self.config, config);
    }
}

/// A set of multiple related piles (i.e. in a single hoard).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultipleEntries {
    config: Option<Config>,
    #[serde(flatten)]
    items: HashMap<String, Pile>,
}

impl MultipleEntries {
    fn process_with(
        self,
        envs: &HashMap<String, bool>,
        exclusivity: &[Vec<String>],
    ) -> Result<ConfigMultiple, Error> {
        let MultipleEntries { config, items } = self;
        let items = items
            .into_iter()
            .map(|(pile, mut entry)| {
                tracing::debug!(%pile, "processing pile");
                entry.layer_config(config.as_ref());
                let _span = tracing::debug_span!("processing_span_outer", name=%pile).entered();
                let entry = entry.process_with(envs, exclusivity)?;
                Ok((pile, entry))
            })
            .collect::<Result<_, Error>>()?;

        Ok(ConfigMultiple { piles: items })
    }

    pub(crate) fn layer_config(&mut self, config: Option<&Config>) {
        Config::layer_options(&mut self.config, config);
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
        envs: &HashMap<String, bool>,
        exclusivity: &[Vec<String>],
    ) -> Result<crate::config::hoard::Hoard, Error> {
        match self {
            Hoard::Single(single) => {
                tracing::debug!("processing anonymous pile");
                Ok(ConfigHoard::Anonymous(
                    single.process_with(envs, exclusivity)?,
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

    pub(crate) fn layer_config(&mut self, config: Option<&Config>) {
        match self {
            Hoard::Single(pile) => pile.layer_config(config),
            Hoard::Multiple(multi) => multi.layer_config(config),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod config {
        use super::{AsymmetricEncryption, Config, Encryption, SymmetricEncryption};

        #[test]
        fn test_layer_configs_both_none() {
            let mut specific = None;
            let general = None;
            Config::layer_options(&mut specific, general);
            assert!(specific.is_none());
        }

        #[test]
        fn test_layer_specific_some_general_none() {
            let mut specific = Some(Config {
                encryption: Some(Encryption::Symmetric(SymmetricEncryption::Password(
                    "password".into(),
                ))),
                ignore: vec!["ignore me".into()],
            });
            let old_specific = specific.clone();
            let general = None;
            Config::layer_options(&mut specific, general);
            assert_eq!(specific, old_specific);
        }

        #[test]
        fn test_layer_specific_none_general_some() {
            let mut specific = None;
            let general = Some(Config {
                encryption: Some(Encryption::Symmetric(SymmetricEncryption::Password(
                    "password".into(),
                ))),
                ignore: vec!["ignore me".into()],
            });
            Config::layer_options(&mut specific, general.as_ref());
            assert_eq!(specific, general);
        }

        #[test]
        fn test_layer_configs_both_some() {
            let mut specific = Some(Config {
                encryption: Some(Encryption::Symmetric(SymmetricEncryption::Password(
                    "password".into(),
                ))),
                ignore: vec!["ignore me".into(), "duplicate".into()],
            });
            let old_specific = specific.clone();
            let general = Some(Config {
                encryption: Some(Encryption::Asymmetric(AsymmetricEncryption {
                    public_key: "somekey".into(),
                })),
                ignore: vec!["me too".into(), "duplicate".into()],
            });
            Config::layer_options(&mut specific, general.as_ref());
            assert!(specific.is_some());
            assert_eq!(
                specific.as_ref().unwrap().encryption,
                old_specific.unwrap().encryption
            );
            assert_eq!(
                specific.unwrap().ignore,
                vec![
                    "duplicate".to_string(),
                    "ignore me".to_string(),
                    "me too".to_string()
                ]
            );
        }
    }

    mod process {
        use super::*;
        use crate::config::hoard::Pile as ConfigPile;
        use maplit::hashmap;
        use std::path::PathBuf;

        #[test]
        #[serial_test::serial]
        fn env_vars_are_expanded() {
            let pile = Pile {
                config: None,
                items: hashmap! {
                    "foo".into() => "${HOME}/something".into()
                },
            };

            let home = std::env::var("HOME").expect("failed to read $HOME");
            let expected = ConfigPile {
                config: None,
                path: Some(PathBuf::from(format!("{}/something", home))),
            };

            let envs = hashmap! { "foo".into() =>  true };
            let result = pile
                .process_with(&envs, &[])
                .expect("pile should process without issues");

            assert_eq!(result, expected);
        }
    }

    mod serde {
        use super::*;
        use maplit::hashmap;
        use serde_test::{assert_tokens, Token};

        #[test]
        fn single_entry_no_config() {
            let hoard = Hoard::Single(Pile {
                config: None,
                items: hashmap! {
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
                    encryption: Some(Encryption::Asymmetric(AsymmetricEncryption {
                        public_key: "public key".to_string(),
                    })),
                    ignore: Vec::new(),
                }),
                items: hashmap! {
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
                    Token::Str("ignore"),
                    Token::Seq { len: Some(0) },
                    Token::SeqEnd,
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
                items: hashmap! {
                    "item1".to_string() => Pile {
                        config: None,
                        items: hashmap! {
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
                    encryption: Some(Encryption::Symmetric(SymmetricEncryption::Password(
                        "correcthorsebatterystaple".into(),
                    ))),
                    ignore: Vec::new(),
                }),
                items: hashmap! {
                    "item1".to_string() => Pile {
                        config: None,
                        items: hashmap! {
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
                    Token::Str("ignore"),
                    Token::Seq { len: Some(0) },
                    Token::SeqEnd,
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
