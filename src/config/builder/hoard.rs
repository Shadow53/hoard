use crate::config::builder::envtrie::{EnvTrie, Error as TrieError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

type ConfigMultiple = crate::config::hoard::MultipleEntries;
type ConfigSingle = crate::config::hoard::SingleEntry;
type ConfigHoard = crate::config::hoard::Hoard;

#[derive(Debug, Error)]
pub enum Error {
    #[error("error while processing environment requirements: {0}")]
    EnvTrie(#[from] TrieError),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SymmetricEncryption {
    password: String,
    password_cmd: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsymmetricEncryption {
    #[serde(rename = "encrypt_pub_key")]
    public_key: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "encrypt", rename_all = "snake_case")]
pub enum Encryption {
    Symmetric(SymmetricEncryption),
    Asymmetric(AsymmetricEncryption),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(flatten)]
    encryption: Encryption,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SingleEntry {
    config: Option<Config>,
    #[serde(flatten)]
    items: BTreeMap<String, PathBuf>,
}

impl SingleEntry {
    fn process_with(
        self,
        envs: &BTreeMap<String, bool>,
        exclusivity: &[Vec<String>],
    ) -> Result<ConfigSingle, Error> {
        let SingleEntry { config, items } = self;
        let trie = EnvTrie::new(&items, exclusivity)?;
        let path = trie.get_path(envs).map(Path::to_owned);

        Ok(ConfigSingle { config, path })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MultipleEntries {
    config: Option<Config>,
    #[serde(flatten)]
    items: BTreeMap<String, SingleEntry>,
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
                let mut entry = entry.process_with(envs, exclusivity)?;
                entry.config = entry.config.or(config.clone());
                Ok((pile, entry))
            })
            .collect::<Result<_, Error>>()?;

        Ok(ConfigMultiple { items })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Hoard {
    Single(SingleEntry),
    Multiple(MultipleEntries),
}

impl Hoard {
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
            let hoard = Hoard::Single(SingleEntry {
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
            let hoard = Hoard::Single(SingleEntry {
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
                    "item1".to_string() => SingleEntry {
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
