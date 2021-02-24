use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename = "lower")]
pub enum GameType {
    Gog,
    Itch,
    Native,
    Steam,
}

impl fmt::Display for GameType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Gog => write!(f, "gog"),
            Self::Itch => write!(f, "itch"),
            Self::Native => write!(f, "native"),
            Self::Steam => write!(f, "steam"),
        }
    }
}

impl FromStr for GameType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "gog" => Ok(Self::Gog),
            "itch" => Ok(Self::Itch),
            "native" => Ok(Self::Native),
            "steam" => Ok(Self::Steam),
            _ => Err(format!("unexpected game type: {}", s)),
        }
    }
}

pub type Game = BTreeMap<GameType, PathBuf>;
pub type Games = BTreeMap<String, Game>;
