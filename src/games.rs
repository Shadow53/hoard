use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_type_display() {
        assert_eq!("gog", GameType::Gog.to_string());
        assert_eq!("itch", GameType::Itch.to_string());
        assert_eq!("native", GameType::Native.to_string());
        assert_eq!("steam", GameType::Steam.to_string());
    }

    #[test]
    fn test_game_type_from_str() {
        assert_eq!("gog".parse::<GameType>().unwrap(), GameType::Gog);
        assert_eq!("itch".parse::<GameType>().unwrap(), GameType::Itch);
        assert_eq!("native".parse::<GameType>().unwrap(), GameType::Native);
        assert_eq!("steam".parse::<GameType>().unwrap(), GameType::Steam);
    }

    #[test]
    fn test_game_type_from_str_is_case_sensitive() {
        "GOG".parse::<GameType>().unwrap_err();
        "ITCH".parse::<GameType>().unwrap_err();
        "NATIVE".parse::<GameType>().unwrap_err();
        "STEAM".parse::<GameType>().unwrap_err();
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
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
