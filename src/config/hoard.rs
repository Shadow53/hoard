use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

// "hoard": {
//     "hoard_name" Hoard: {
//         "config": Config,
//         "envconf": "path",
//         ...
//     }
// }

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
#[serde(tag = "encrypt")]
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
pub struct Hoard {
    config: Option<Config>,
    #[serde(flatten)]
    items: HoardEntry,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HoardEntry {
    Single(Entry),
    Multiple(BTreeMap<String, Entry>),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    environments: Vec<String>,
    destination: PathBuf,
}
