//! This module contains integration tests for the `config` subcommand.

mod envtrie;

/*
use common::{file, toml};

use std::path::PathBuf;

use hoard::config::builder::Builder as ConfigBuilder;

fn strip_non_config_file(builder: ConfigBuilder) -> ConfigBuilder {
    builder.unset_config_file().unset_command()
}

#[test]
fn test_init_config() {
    let file = file::get_temp_file();

    let cmd = Command::Config {
        command: ConfigCmd::Init,
    };

    let run_config = ConfigBuilder::new()
        .set_config_file(file.path().to_owned())
        .set_command(cmd)
        .build();

    run_config
        .command
        .run(&run_config)
        .expect("failed to run init command");

    let expected = ConfigBuilder::from(Config::default())
        // Expect games file to be next to the config file
        .set_games_file(file.path().parent().unwrap().join("games.toml"));

    file.reopen().expect("failed to reopen file after writing");

    toml::assert_file_contains_deserializable(
        file.path(),
        &strip_non_config_file(expected.clone()),
    );

    std::fs::remove_file(file.path()).expect("failed to delete file");

    run_config
        .command
        .run(&run_config)
        .expect("failed to run init command");

    // Reopening fails because it's not the original file
    // file.reopen().expect("failed to reopen file after writing");

    toml::assert_file_contains_deserializable(file.path(), &strip_non_config_file(expected));

    // Cleanup
    std::fs::remove_file(file.path()).expect("failed to delete file");
}

#[test]
fn test_reset_config() {
    let file = file::get_temp_file();

    let cmd = Command::Config {
        command: ConfigCmd::Reset,
    };

    let run_config = ConfigBuilder::new()
        .set_config_file(file.path().to_owned())
        .set_command(cmd)
        .build();

    run_config
        .command
        .run(&run_config)
        .expect("failed to run reset command");

    let expected = ConfigBuilder::from(Config::default())
        // Expect games file to be next to the config file
        .set_games_file(file.path().parent().unwrap().join("games.toml"));

    file.reopen().expect("failed to reopen file after writing");

    toml::assert_file_contains_deserializable(file.path(), &strip_non_config_file(expected));
}

#[test]
fn test_add_to_config() {
    let file = file::get_temp_file();

    let saves_root = PathBuf::from("/testing/saves");

    let set_config = SetConfig {
        field: ConfigField::SavesRoot,
        value: saves_root.to_string_lossy().to_string(),
    };

    let config_cmd = ConfigCmd::Set(set_config);

    let cmd = Command::Config {
        command: config_cmd,
    };

    let run_config = ConfigBuilder::new()
        .set_config_file(file.path().to_owned())
        .set_command(cmd)
        .build();

    let builder = ConfigBuilder::from(run_config.clone()).set_saves_root(saves_root);

    run_config
        .command
        .run(&run_config)
        .expect("failed to run command");

    toml::assert_file_contains_deserializable(file.path(), &strip_non_config_file(builder));
}

#[test]
fn test_remove_from_config() {
    let file = file::get_temp_file();

    let saves_root = PathBuf::from("/testing/saves");

    let unset_config = UnsetConfig {
        field: ConfigField::SavesRoot,
    };

    let config_cmd = ConfigCmd::Unset(unset_config);

    let cmd = Command::Config {
        command: config_cmd,
    };

    let run_config = ConfigBuilder::new()
        .set_saves_root(saves_root)
        .set_config_file(file.path().to_owned())
        .set_command(cmd)
        .build();

    let builder = ConfigBuilder::from(run_config.clone()).unset_saves_root();

    run_config
        .command
        .run(&run_config)
        .expect("failed to run command");

    toml::assert_file_contains_deserializable(file.path(), &strip_non_config_file(builder));
}
*/
