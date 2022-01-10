use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Configuration for symmetric (password) encryption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum SymmetricEncryption {
    /// Raw password.
    #[serde(rename = "password")]
    Password(String),
    /// Command whose first line of output to stdout is the password.
    #[serde(rename = "password_cmd")]
    PasswordCmd(Vec<String>),
}

/// Configuration for asymmetric (public key) encryption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AsymmetricEncryption {
    #[serde(rename = "public_key")]
    pub public_key: String,
}

/// Configuration for hoard/pile encryption.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Encryption {
    /// Symmetric encryption.
    Symmetric(SymmetricEncryption),
    /// Asymmetric encryption.
    Asymmetric(AsymmetricEncryption),
}

#[allow(single_use_lifetimes)]
fn deserialize_glob<'de, D>(deserializer: D) -> Result<Vec<glob::Pattern>, D::Error>
where
    D: Deserializer<'de>,
{
    Vec::<String>::deserialize(deserializer)?
        .iter()
        .map(String::as_str)
        .map(glob::Pattern::new)
        .collect::<Result<_, _>>()
        .map_err(D::Error::custom)
}

#[allow(clippy::ptr_arg)]
fn serialize_glob<S>(value: &Vec<glob::Pattern>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let value = value
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<String>>();

    value.serialize(serializer)
}

/// Hoard/Pile configuration.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The [`Encryption`] configuration for a pile.
    #[serde(default, rename = "encrypt")]
    pub encryption: Option<Encryption>,
    /// A list of glob patterns matching files to ignore.
    #[serde(
        default,
        deserialize_with = "deserialize_glob",
        serialize_with = "serialize_glob"
    )]
    pub ignore: Vec<glob::Pattern>,
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
