pub mod backup;
pub mod config;
pub mod games;

use std::{
    io,
    path::{Path, PathBuf},
};

use backup::{Direction, Error as BackupError};
use config::{AddGame, Command, Config, RemoveGame};
use games::Games;
use io::Write;
use log::{debug, info, warn};
use structopt::clap::Error as ClapError;
use structopt::StructOpt;

pub enum Error {
    Backup(BackupError),
    PrintHelp(ClapError),
    GameOfTypeExists(AddGame, PathBuf),
    SaveGames(io::Error),
    ReadGames(io::Error),
    DeserializeGames(toml::de::Error),
    SerializeGames(toml::ser::Error),
    ReadConfig(io::Error),
    DeserializeConfig(toml::de::Error),
}

fn backup(root: &Path, games: &Games) -> Result<(), Error> {
    for (name, game) in games {
        info!("Backing up {}", name);
        backup::copy_game(root, name, game, Direction::Backup).map_err(Error::Backup)?;
    }

    Ok(())
}

fn restore(root: &Path, games: &Games) -> Result<(), Error> {
    for (name, game) in games {
        info!("Restoring {}", name);
        backup::copy_game(root, name, game, Direction::Restore).map_err(Error::Backup)?;
    }

    Ok(())
}

fn save_games_file(games_path: &Path, games: &Games) -> Result<(), Error> {
    info!(
        "Saving games configuration to {}",
        games_path.to_string_lossy()
    );
    let output = toml::to_string_pretty(games).map_err(Error::SerializeGames)?;

    let mut file = std::fs::File::create(games_path).map_err(Error::SaveGames)?;

    file.write_all(output.as_bytes()).map_err(Error::SaveGames)
}

fn add_game(games_path: &Path, mut games: Games, to_add: AddGame) -> Result<(), Error> {
    // Remove game for modification
    let mut game = games.remove(&to_add.game).unwrap_or_default();

    // Overwriting is not enabled and item exists
    if !to_add.force {
        debug!("Not allowed to overwrite entries");
        if let Some(item) = game.get(&to_add.ty) {
            debug!("Existing item found. Returning error.");
            return Err(Error::GameOfTypeExists(to_add, item.to_owned()));
        }
    }

    // Insert. Log old version if present.
    if let Some(old_path) = game.insert(to_add.ty, to_add.path) {
        warn!("replaced old path {}", old_path.to_string_lossy());
    }

    // Re-insert game into collection
    games.insert(to_add.game, game);

    // Save to file
    save_games_file(games_path, &games)?;

    Ok(())
}

fn rm_game(games_path: &Path, mut games: Games, to_rm: RemoveGame) -> Result<(), Error> {
    // Remove game for modification
    let mut game = games.remove(&to_rm.game).unwrap_or_default();

    // If type specified, remove just the type entry and re-add game
    // Otherwise, remove all game items
    match to_rm.ty {
        Some(ty) => {
            match game.remove(&ty) {
                Some(old_path) => info!(
                    "Removed saves path for {} {}: {}",
                    ty,
                    to_rm.game,
                    old_path.to_string_lossy()
                ),
                None => warn!("No saves path found for {} {}", ty, to_rm.game),
            }

            // Re-insert game into collection
            games.insert(to_rm.game, game);
        }
        None => info!("Removed all entries for {}", to_rm.game),
    }

    // Save to file
    save_games_file(games_path, &games)?;

    Ok(())
}

pub fn get_config() -> Result<Config, Error> {
    // Get config from args
    let arg_config = Config::from_args();

    // Get config from file
    let file_config: Config = {
        let config_path = arg_config.get_config_path();
        let s = std::fs::read_to_string(config_path).map_err(Error::ReadConfig)?;
        toml::from_str(&s).map_err(Error::DeserializeConfig)?
    };

    Ok(arg_config | file_config)
}

pub fn run(config: Config) -> Result<(), Error> {
    let games_path = config.get_games_path();
    let games: Games = {
        debug!(
            "Reading games entries from {}",
            games_path.to_string_lossy()
        );
        let s = std::fs::read_to_string(&games_path).map_err(Error::ReadGames)?;
        toml::from_str(&s).map_err(Error::DeserializeGames)?
    };

    let root = config.get_root_path();
    debug!("Game saves directory: {}", root.to_string_lossy());

    if let Some(command) = config.command {
        match command {
            Command::Help => Config::clap().print_long_help().map_err(Error::PrintHelp)?,
            Command::Backup => backup(&root, &games)?,
            Command::Restore => restore(&root, &games)?,
            Command::Add(to_add) => add_game(&games_path, games, to_add)?,
            Command::Remove(to_rm) => rm_game(&games_path, games, to_rm)?,
        }
    }

    Ok(())
}
