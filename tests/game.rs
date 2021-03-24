use hoard::game::{self, AddGame, GameType, RemoveGame};
use hoard::{Command, Config};
use maplit::btreemap;
use std::path::PathBuf;

mod common;

use common::toml;

fn generate_games() -> game::Games {
    btreemap! {
        "test-1".to_owned() => btreemap! {
            GameType::Gog => PathBuf::from("/tmp/games/gog/test-1.sav"),
            GameType::Steam => PathBuf::from("/tmp/games/steam/test-1.sav"),
        },
        "test-2".to_owned() => btreemap! {
            GameType::Native => PathBuf::from("/tmp/games/native/test-2.sav"),
        }
    }
}

fn generate_games_file() -> tempfile::NamedTempFile {
    let file = common::file::get_temp_file();
    let games = generate_games();
    game::save_games_file(file.path(), &games).expect("failed to save games file");
    file
}

#[test]
fn test_save_and_read_games_file() {
    // Makes sure saving doesn't fail
    let file = generate_games_file();

    let games = generate_games();

    toml::assert_file_contains_deserializable(file.path(), &games);
}

#[test]
fn test_add_game() {
    let file = generate_games_file();
    let id = String::from("test-3");
    let ty = GameType::Native;
    let path = PathBuf::from("/nonexist/test-3/saves");
    let games = {
        let mut games = generate_games();
        games.insert(id.clone(), btreemap! { ty => path.clone() });
        games
    };

    let add_game = AddGame {
        ty: ty,
        path: path.clone(),
        game: id.clone(),
        force: false,
    };

    let config = Config::builder()
        .set_saves_root(PathBuf::from("/nonexist/saves"))
        .set_config_file(PathBuf::from("/nonexist/config.toml"))
        .set_games_file(file.path().to_owned())
        .set_log_level(log::Level::Info)
        .set_command(Command::Game {
            command: game::Command::Add(add_game),
        })
        .build();

    config
        .command
        .run(&config)
        .expect("failed to add game to file");

    toml::assert_file_contains_deserializable(file.path(), &games);
}

#[test]
fn test_add_game_no_overwrite_without_force() {
    let file = generate_games_file();
    let id = String::from("test-2");
    let ty = GameType::Native;
    let path = PathBuf::from("/nonexist/test-2/saves");
    let games = generate_games();

    let add_game = AddGame {
        ty: ty,
        path: path.clone(),
        game: id.clone(),
        force: false,
    };

    let config = Config::builder()
        .set_saves_root(PathBuf::from("/nonexist/saves"))
        .set_config_file(PathBuf::from("/nonexist/config.toml"))
        .set_games_file(file.path().to_owned())
        .set_log_level(log::Level::Info)
        .set_command(Command::Game {
            command: game::Command::Add(add_game),
        })
        .build();

    let err = config
        .command
        .run(&config)
        .expect_err("failed to add game to file");

    match err {
        hoard::Error::GameCmd(game::Error::GameOfTypeExists(_, _)) => {}
        _ => panic!("unexpected error: {}", err),
    }

    toml::assert_file_contains_deserializable(file.path(), &games);
}

#[test]
fn test_add_game_overwrite_with_force() {
    let file = generate_games_file();
    let id = String::from("test-2");
    let ty = GameType::Native;
    let path = PathBuf::from("/nonexist/test-2/saves");
    let games = {
        let mut games = generate_games();
        games.insert(id.clone(), btreemap! { ty => path.clone() });
        games
    };

    let add_game = AddGame {
        ty: ty,
        path: path.clone(),
        game: id.clone(),
        force: true,
    };

    let config = Config::builder()
        .set_saves_root(PathBuf::from("/nonexist/saves"))
        .set_config_file(PathBuf::from("/nonexist/config.toml"))
        .set_games_file(file.path().to_owned())
        .set_log_level(log::Level::Info)
        .set_command(Command::Game {
            command: game::Command::Add(add_game),
        })
        .build();

    config
        .command
        .run(&config)
        .expect("failed to add game to file");

    toml::assert_file_contains_deserializable(file.path(), &games);
}

#[test]
fn test_remove_game() {
    let file = generate_games_file();
    let id = String::from("test-2");
    let games = {
        let mut games = generate_games();
        games.remove(&id);
        games
    };

    let rm_game = RemoveGame {
        ty: None,
        game: id.clone(),
    };

    let config = Config::builder()
        .set_saves_root(PathBuf::from("/nonexist/saves"))
        .set_config_file(PathBuf::from("/nonexist/config.toml"))
        .set_games_file(file.path().to_owned())
        .set_log_level(log::Level::Info)
        .set_command(Command::Game {
            command: game::Command::Remove(rm_game),
        })
        .build();

    config
        .command
        .run(&config)
        .expect("failed to add game to file");

    toml::assert_file_contains_deserializable(file.path(), &games);
}

#[test]
fn test_remove_game_remove_only_type() {
    let file = generate_games_file();
    let id = String::from("test-1");
    let games = {
        let mut games = generate_games();
        games.get_mut(&id).unwrap().remove(&GameType::Gog);
        games
    };

    let rm_game = RemoveGame {
        ty: Some(GameType::Gog),
        game: id.clone(),
    };

    let config = Config::builder()
        .set_saves_root(PathBuf::from("/nonexist/saves"))
        .set_config_file(PathBuf::from("/nonexist/config.toml"))
        .set_games_file(file.path().to_owned())
        .set_log_level(log::Level::Info)
        .set_command(Command::Game {
            command: game::Command::Remove(rm_game),
        })
        .build();

    config
        .command
        .run(&config)
        .expect("failed to add game to file");

    toml::assert_file_contains_deserializable(file.path(), &games);
}

#[test]
fn test_remove_game_only_type_removes_all() {
    let file = generate_games_file();
    let id = String::from("test-2");
    let games = {
        let mut games = generate_games();
        games.remove(&id);
        games
    };

    let rm_game = RemoveGame {
        ty: Some(GameType::Native),
        game: id.clone(),
    };

    let config = Config::builder()
        .set_saves_root(PathBuf::from("/nonexist/saves"))
        .set_config_file(PathBuf::from("/nonexist/config.toml"))
        .set_games_file(file.path().to_owned())
        .set_log_level(log::Level::Info)
        .set_command(Command::Game {
            command: game::Command::Remove(rm_game),
        })
        .build();

    config
        .command
        .run(&config)
        .expect("failed to add game to file");

    toml::assert_file_contains_deserializable(file.path(), &games);
}
