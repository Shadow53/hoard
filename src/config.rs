use crate::games::GameType;
use directories::ProjectDirs;
use log::Level;
use serde::{
    de::{Deserializer, Error as DeserializeError, Visitor},
    Deserialize,
};
use std::{
    ops::BitOr,
    path::PathBuf,
};
use structopt::StructOpt;

#[cfg(test)]
mod tests {
    use super::*;

    /// Contains tests relating to the implementation of Default
    mod defaults {
        use super::*;

        #[test]
        fn test_config_default_uses_default_folders() {
            let config = Config::default();
            let empty = Config::empty();

            assert_eq!(Some(empty.get_config_path()), config.config);
            assert_eq!(Some(empty.get_root_path()), config.root);
            assert_eq!(Some(empty.get_games_path()), config.games_path);
        }

        #[test]
        fn test_config_default_log_level_is_info() {
            let config = Config::default();

            assert_eq!(Some(Level::Info), config.log_level);
        }

        #[test]
        fn test_config_default_command_is_none() {
            let config = Config::default();
            assert_eq!(None, config.command);
        }
    }

    /// Contains tests relating to the logic of finding configured directories
    mod folders {
        use super::*;

        #[test]
        fn test_get_config_file_uses_override() {
            let mut config = Config::empty();
            let expected = PathBuf::from("/testing/override");
            config.config = Some(expected.clone());
            assert_eq!(expected, config.get_config_path(), "configuration file path should use provided value, if exists");
        }

        #[test]
        fn test_get_config_file_defaults_to_system_dir() {
            let dirs = get_dirs();
            let expected = dirs.config_dir().join("config.toml");
            let empty = Config::empty();

            assert_eq!(None, empty.config, "config should be None for test to be valid");
            assert_eq!(expected, empty.get_config_path(), "configuration file should default to config.toml in project config path");
        }

        #[test]
        fn test_get_root_dir_uses_override() {
            let mut config = Config::empty();
            let expected = PathBuf::from("/testing/override");
            config.root = Some(expected.clone());
            assert_eq!(expected, config.get_root_path(), "saves backup path should use provided value, if exists");
        }

        #[test]
        fn test_get_root_dir_defaults_to_system_dir() {
            let dirs = get_dirs();
            let expected = dirs.data_dir().join("saves");
            let empty = Config::empty();

            assert_eq!(None, empty.config, "root should be None for test to be valid");
            assert_eq!(expected, empty.get_root_path(), "saves backup path should default to `saves/` in project data path");
        }

        #[test]
        fn test_get_games_file_uses_override() {
            let mut config = Config::empty();
            let expected = PathBuf::from("/testing/override");
            config.games_path = Some(expected.clone());
            assert_eq!(expected, config.get_games_path(), "games file path should use provided value, if exists");
        }

        #[test]
        fn test_get_games_file_defaults_to_config_sibling() {
            let mut empty = Config::empty();
            empty.config = Some(PathBuf::from("/path/to/config.toml"));
            let expected = PathBuf::from("/path/to/games.toml");

            assert_eq!(None, empty.games_path, "games_path must be None for test to be valid");
            assert_eq!(expected, empty.get_games_path(), "games file should default to games.toml next to config file");
        }

        #[test]
        fn test_get_games_file_uses_config_dir_if_no_parent() {
            // If the config file is set to a path with no parent (i.e. `/` on Linux),
            // this avoids crashing by providing a super default.
            let dirs = get_dirs();
            let mut empty = Config::empty();
            empty.config = Some(PathBuf::from("/"));
            let expected = dirs.config_dir().join("games.toml");

            assert_eq!(None, empty.games_path, "games_path must be None for test to be valid");
            assert_eq!(expected, empty.get_games_path(), "games file should default to games.toml in project config dir when config file has no parent");
        }
    }

    #[test]
    fn test_config_empty_is_all_none() {
        let expected = Config {
            root: None,
            config: None,
            games_path: None,
            log_level: None,
            command: None,
        };

        let empty = Config::empty();

        assert_eq!(expected, empty);
    }

    #[test]
    fn test_config_bitor_prefers_left() {
        let none = Config::empty();

        let some_one = Config {
            root: Some(PathBuf::from("/some/one/root")),
            config: Some(PathBuf::from("/some/one/config")),
            games_path: Some(PathBuf::from("/some/one/games")),
            log_level: Some(Level::Warn),
            command: Some(Command::Help),
        };

        let some_two = Config {
            root: Some(PathBuf::from("/some/two/root")),
            config: Some(PathBuf::from("/some/two/config")),
            games_path: Some(PathBuf::from("/some/two/games")),
            log_level: Some(Level::Error),
            command: Some(Command::Backup),
        };

        assert_eq!(none.clone() | some_one.clone(), some_one, "All empty values on left should be replaced by ones on right");
        assert_eq!(none.clone() | some_two.clone(), some_two, "All empty values on left should be replaced by ones on right");
        assert_eq!(some_one.clone() | some_two.clone(), some_one, "Non-empty values on left should be preferred");
        assert_eq!(some_two.clone() | some_one.clone(), some_two, "Non-empty values on left should be preferred");
    }
}

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

#[derive(Clone, Debug, PartialEq, Deserialize, StructOpt)]
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

impl Default for Config {
    fn default() -> Self {
        let empty = Config::empty();

        Config {
            root: Some(empty.get_root_path()),
            config: Some(empty.get_config_path()),
            games_path: Some(empty.get_games_path()),
            log_level: Some(Level::Info),
            command: None,
        }
    }
}

impl Config {
    pub fn empty() -> Self {
        Config {
            root: None,
            config: None,
            games_path: None,
            log_level: None,
            command: None,
        }
    }

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

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub struct AddGame {
    pub game: String,
    pub ty: GameType,
    pub path: PathBuf,
    #[structopt(short, long)]
    pub force: bool,
}

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub struct RemoveGame {
    pub game: String,
    pub ty: Option<GameType>,
}

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub enum GameSubcommand {
    Add(AddGame),
    Remove(RemoveGame),
}

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub enum Command {
    Help,
    Backup,
    Restore,
    Game{ #[structopt(subcommand)] command: GameSubcommand },
}

impl Default for Command {
    fn default() -> Self {
        Self::Help
    }
}
