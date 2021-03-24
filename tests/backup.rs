use hoard::backup::Direction;
use hoard::{Command, Config, Game, GameType, Games};
use rand::RngCore;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::{tempdir, NamedTempFile, TempDir};

mod common;

struct TestBackups {
    // Root of all backups
    backup: TempDir,
    // The source folder to backup from/restore to
    dest: TempDir,
    // The games file for the `Config`
    games_file: NamedTempFile,
    // The `Games` data for the `Config`
    games: Games,
    // The `Config` to use to run the program
    config: Config,
}

impl TestBackups {
    const NESTED_DIR_NAME: &'static str = "nested";
    const OTHER_FILE_NAME: &'static str = "other.txt";
    const TOP_FILE_NAME: &'static str = "top_file.txt";
    const NESTED_FILE_NAME: &'static str = "nested_file.txt";
    const GAME_NAME: &'static str = "test_game";
    const GAME_TYPE: GameType = GameType::Native;

    fn new(direction: Direction) -> Self {
        let backup = tempdir().expect("failed to create backup dir");
        let dest = tempdir().expect("failed to create dest dir");
        let mut games_file = NamedTempFile::new().expect("failed to create games file");

        let nested_dest = dest.path().join(Self::NESTED_DIR_NAME);
        let nested_src = backup.path().join(Self::NESTED_DIR_NAME);

        fs::create_dir(nested_src).expect("failed to create nested src dir");
        fs::create_dir(nested_dest).expect("failed to create nested dest dir");

        let games = {
            let mut games = Games::new();
            let game = {
                let mut game = Game::new();
                game.insert(Self::GAME_TYPE, dest.path().to_owned());
                game
            };
            games.insert(Self::GAME_NAME.to_owned(), game);
            games
        };

        let command = match direction {
            Direction::Backup => Command::Backup,
            Direction::Restore => Command::Restore,
        };

        let games_str = toml::to_string_pretty(&games).expect("failed to convert games to toml");

        games_file
            .write_all(games_str.as_bytes())
            .expect("failed to write games bytes to file");
        games_file.flush().expect("failed to flush games file");

        let config = Config::builder()
            .set_saves_root(backup.path().to_owned())
            .set_games_file(games_file.path().to_owned())
            .set_command(command)
            .build();

        TestBackups {
            backup,
            dest,
            games,
            games_file,
            config,
        }
    }

    fn populate_in(path: &Path, include_untouched: bool) {
        let top_level = fs::File::create(path.join(Self::TOP_FILE_NAME))
            .expect("failed to create top-level file");
        let nested = fs::File::create(
            path.join(Self::NESTED_DIR_NAME)
                .join(Self::NESTED_FILE_NAME),
        )
        .expect("failed to create nested file");

        let files = if include_untouched {
            let untouched =
                fs::File::create(path.join(Self::NESTED_DIR_NAME).join(Self::OTHER_FILE_NAME))
                    .expect("failed to create file that shouldn't be touched");
            vec![top_level, nested, untouched]
        } else {
            vec![top_level, nested]
        };

        for mut file in files {
            let mut buf = [0u8; 256];
            rand::thread_rng()
                .try_fill_bytes(&mut buf)
                .expect("failed to write to buffer");
            file.write_all(&buf).expect("failed to write to file");
            file.sync_all()
                .expect("failed to sync file data to filesystem");
        }
    }

    fn populate_backup(&self, include_untouched: bool) {
        Self::populate_in(self.backup.path(), include_untouched)
    }

    fn populate_dest(&self, include_untouched: bool) {
        Self::populate_in(self.dest.path(), include_untouched)
    }

    fn assert_same_files(&self) {
        let paths = vec![
            (
                self.backup.path().join(Self::TOP_FILE_NAME),
                self.dest.path().join(Self::TOP_FILE_NAME),
            ),
            (
                self.backup
                    .path()
                    .join(Self::NESTED_DIR_NAME)
                    .join(Self::NESTED_FILE_NAME),
                self.dest
                    .path()
                    .join(Self::NESTED_DIR_NAME)
                    .join(Self::NESTED_FILE_NAME),
            ),
        ];

        for (path1, path2) in paths {
            let mut file1 = fs::File::open(&path1).expect("failed to open file");
            let mut file2 = fs::File::open(&path2).expect("failed to open file");

            common::file::assert_eq_files(&mut file1, &mut file2);
        }
    }

    fn assert_untouched_file(&self) {
        let paths = match &self.config.command {
            Command::Backup => vec![(
                self.backup
                    .path()
                    .join(Self::NESTED_DIR_NAME)
                    .join(Self::OTHER_FILE_NAME),
                self.dest
                    .path()
                    .join(Self::NESTED_DIR_NAME)
                    .join(Self::OTHER_FILE_NAME),
            )],
            Command::Restore => vec![(
                self.dest
                    .path()
                    .join(Self::NESTED_DIR_NAME)
                    .join(Self::OTHER_FILE_NAME),
                self.backup
                    .path()
                    .join(Self::NESTED_DIR_NAME)
                    .join(Self::OTHER_FILE_NAME),
            )],
            _ => panic!("expected Command::Backup, Command::Restore"),
        };

        for (exists, no_exists) in paths {
            assert!(
                exists.exists(),
                "{} does not exist but should",
                exists.to_string_lossy()
            );
            assert!(
                !no_exists.exists(),
                "{} exists but should not",
                no_exists.to_string_lossy()
            );
        }
    }

    fn run(&self) -> Result<(), hoard::Error> {
        self.config.command.run(&self.config)
    }
}

#[test]
fn test_backup_empty_root() {
    let test_backups = TestBackups::new(Direction::Backup);
    test_backups.populate_backup(false);
    test_backups.run().expect("command failed");
}

#[test]
fn test_restore_empty_dest() {
    let test_backups = TestBackups::new(Direction::Restore);
    test_backups.populate_dest(false);
    test_backups.run().expect("command failed");
}

#[test]
fn test_backup_nonempty_dest() {
    let test_backups = TestBackups::new(Direction::Backup);
    test_backups.populate_backup(false);
    test_backups.populate_dest(false);
    test_backups.run().expect("command failed");
}

#[test]
fn test_restore_nonempty_dest() {
    let test_backups = TestBackups::new(Direction::Restore);
    test_backups.populate_backup(false);
    test_backups.populate_dest(false);
    test_backups.run().expect("command failed");
}

#[test]
fn test_backup_src_not_deleted() {
    let test_backups = TestBackups::new(Direction::Backup);
    test_backups.populate_backup(true);
    test_backups.populate_dest(false);
    test_backups.run().expect("command failed");
}

#[test]
fn test_restore_dest_not_deleted() {
    let test_backups = TestBackups::new(Direction::Restore);
    test_backups.populate_backup(false);
    test_backups.populate_dest(true);
    test_backups.run().expect("command failed");
}
