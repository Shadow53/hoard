use super::Config;
use log::{debug, info, warn};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    convert::TryFrom,
    io::{self, Write},
    str::FromStr,
};
use std::{
    fmt,
    path::{Path, PathBuf},
};
use structopt::StructOpt;
use thiserror::Error;

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
#[serde(rename = "lower", try_from = "String", into = "String")]
pub enum GameType {
    Gog,
    Itch,
    Native,
    Steam,
}

impl From<GameType> for String {
    fn from(t: GameType) -> String {
        t.to_string()
    }
}

impl TryFrom<String> for GameType {
    type Error = Error;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
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
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "gog" => Ok(Self::Gog),
            "itch" => Ok(Self::Itch),
            "native" => Ok(Self::Native),
            "steam" => Ok(Self::Steam),
            _ => Err(Error::ParseGameType(s.to_owned())),
        }
    }
}

pub type Game = BTreeMap<GameType, PathBuf>;
pub type Games = BTreeMap<String, Game>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to add {0}: already exists at {1}")]
    GameOfTypeExists(AddGame, PathBuf),
    #[error("failed to save games list: {0}")]
    Save(io::Error),
    #[error("failed to serialize games data: {0}")]
    Serialize(toml::ser::Error),
    #[error("failed to read games list from file: {0}")]
    ReadGames(io::Error),
    #[error("failed to deserialize games file: {0}")]
    DeserializeGames(toml::de::Error),
    #[error("could not parse value as game type: {0}")]
    ParseGameType(String),
}

pub fn save_games_file(games_path: &Path, games: &Games) -> Result<(), Error> {
    info!(
        "Saving games configuration to {}",
        games_path.to_string_lossy()
    );
    let output = toml::to_string_pretty(games).map_err(Error::Serialize)?;

    let mut file = std::fs::File::create(games_path).map_err(Error::Save)?;

    file.write_all(output.as_bytes()).map_err(Error::Save)
}

pub fn read_games_file(path: &Path) -> Result<Games, Error> {
    debug!("Reading games entries from {}", path.to_string_lossy());
    let s = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(err) => match err.kind() {
            io::ErrorKind::NotFound => String::new(),
            _ => return Err(Error::ReadGames(err)),
        },
    };
    toml::from_str(&s).map_err(Error::DeserializeGames)
}

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub struct AddGame {
    pub game: String,
    pub ty: GameType,
    pub path: PathBuf,
    #[structopt(short, long)]
    pub force: bool,
}

impl fmt::Display for AddGame {
    /// This implementation on AddGame is for the purpose of error reporting.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} ({}) at {}",
            self.game,
            self.ty,
            self.path.to_string_lossy()
        )
    }
}

impl AddGame {
    pub fn add_game(&self, games_file: &Path) -> Result<(), Error> {
        let mut games = read_games_file(&games_file)?;
        // Remove game for modification
        let mut game = games.remove(&self.game).unwrap_or_default();

        // Overwriting is not enabled and item exists
        if !self.force {
            debug!("Not allowed to overwrite entries");
            if let Some(item) = game.get(&self.ty) {
                debug!("Existing item found. Returning error.");
                return Err(Error::GameOfTypeExists(self.clone(), item.to_owned()));
            }
        }

        // Insert. Log old version if present.
        if let Some(old_path) = game.insert(self.ty, self.path.clone()) {
            warn!("replaced old path {}", old_path.to_string_lossy());
        }

        // Re-insert game into collection
        games.insert(self.game.clone(), game);

        // Save to file
        save_games_file(&games_file, &games)?;

        Ok(())
    }
}

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub struct RemoveGame {
    pub game: String,
    pub ty: Option<GameType>,
}

impl RemoveGame {
    pub fn remove_game(&self, games_file: &Path) -> Result<(), Error> {
        let mut games = read_games_file(&games_file)?;
        // Remove game for modification
        let mut game = games.remove(&self.game).unwrap_or_default();

        // If type specified, remove just the type entry and re-add game
        // Otherwise, remove all game items
        match self.ty {
            Some(ty) => {
                match game.remove(&ty) {
                    Some(old_path) => info!(
                        "Removed saves path for {} {}: {}",
                        ty,
                        self.game,
                        old_path.to_string_lossy()
                    ),
                    None => warn!("No saves path found for {} {}", ty, self.game),
                }

                if !game.is_empty() {
                    // Re-insert game into collection
                    games.insert(self.game.clone(), game);
                }
            }
            None => info!("Removed all entries for {}", self.game),
        }

        // Save to file
        save_games_file(&games_file, &games)?;

        Ok(())
    }
}

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub enum Command {
    Add(AddGame),
    Remove(RemoveGame),
}

impl Command {
    pub fn run(&self, config: &Config) -> Result<(), Error> {
        match self {
            Self::Add(adder) => adder.add_game(&config.games_file),
            Self::Remove(remover) => remover.remove_game(&config.games_file),
        }
    }
}
