use crate::games::GameType;
use directories::ProjectDirs;
use log::Level;
use serde::{
    de::{Deserializer, Error as DeserializeError, Visitor},
    Deserialize,
};
use std::{
    ops::{BitOr, BitOrAssign},
    path::PathBuf,
};
use structopt::StructOpt;

pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const GAMES_LIST_NAME: &str = "games.toml";
pub const GAMES_DIR_SLUG: &str = "saves";

pub fn get_dirs() -> ProjectDirs {
    ProjectDirs::from("com", "shadow53", "backup-game-saves")
        .expect("could not detect user home directory to place program files")
}

struct LevelVisitor;

impl<'de> Visitor<'de> for LevelVisitor {
    type Value = Option<Level>;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(formatter, "A valid log level")
    }

    fn visit_str<E: DeserializeError>(self, s: &str) -> Result<Self::Value, E> {
        Ok(Some(
            s.parse::<Level>()
                .map_err(|err| E::custom(err.to_string()))?,
        ))
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<Self::Value, D::Error> {
        deserializer.deserialize_str(self)
    }

    fn visit_none<E: DeserializeError>(self) -> Result<Option<Level>, E> {
        Ok(None)
    }
}

fn deserialize_level<'de, D>(deserializer: D) -> Result<Option<Level>, D::Error>
where
    D: Deserializer<'de>,
{
    deserializer.deserialize_option(LevelVisitor)
}

fn default_level() -> Option<Level> {
    Some(Level::Info)
}

#[derive(Clone, Deserialize, StructOpt)]
pub struct Config {
    #[structopt(short, long)]
    root: Option<PathBuf>,
    #[structopt(short, long)]
    #[serde(skip)]
    config: Option<PathBuf>,
    #[structopt(short = "g", long = "games-list")]
    games_path: Option<PathBuf>,
    #[structopt(short, long)]
    #[serde(deserialize_with = "deserialize_level", default = "default_level")]
    pub log_level: Option<Level>,
    #[structopt(subcommand)]
    #[serde(skip)]
    pub command: Option<Command>,
}

impl Config {
    pub fn get_config_path(&self) -> PathBuf {
        self.config
            .clone()
            .unwrap_or_else(|| get_dirs().config_dir().join(CONFIG_FILE_NAME))
    }

    pub fn get_root_path(&self) -> PathBuf {
        self.root
            .clone()
            .unwrap_or_else(|| get_dirs().data_dir().join(GAMES_DIR_SLUG))
    }

    pub fn get_games_path(&self) -> PathBuf {
        self.games_path.clone().unwrap_or_else(|| {
            // If not provided, assume next to the config file.
            self.get_config_path()
                .parent()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| {
                    // If parent doesn't exist for some reason
                    // It always should, but this is preferable to unwrap(), IMO
                    get_dirs().config_dir().to_owned()
                })
                .join(GAMES_LIST_NAME)
        })
    }
}

impl BitOr for Config {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        Self {
            root: self.root.or(rhs.root),
            config: self.config.or(rhs.config),
            games_path: self.games_path.or(rhs.games_path),
            log_level: self.log_level.or(rhs.log_level),
            command: self.command.or(rhs.command),
        }
    }
}

impl BitOrAssign for Config {
    fn bitor_assign(&mut self, rhs: Self) {
        *self = self.clone() | rhs
    }
}

#[derive(Clone, StructOpt)]
pub struct AddGame {
    pub game: String,
    pub ty: GameType,
    pub path: PathBuf,
    #[structopt(short, long)]
    pub force: bool,
}

#[derive(Clone, StructOpt)]
pub struct RemoveGame {
    pub game: String,
    pub ty: Option<GameType>,
}

#[derive(Clone, StructOpt)]
pub enum Command {
    Help,
    Backup,
    Restore,
    Add(AddGame),
    Remove(RemoveGame),
}

impl Default for Command {
    fn default() -> Self {
        Self::Help
    }
}
