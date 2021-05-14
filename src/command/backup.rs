use log::{debug, info};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to copy {src} to {dest}: {error}")]
    CopyFile {
        src: PathBuf,
        dest: PathBuf,
        error: io::Error,
    },
    #[error("failed to create {path}: {error}")]
    CreateDir { path: PathBuf, error: io::Error },
    #[error("cannot read directory {path}: {error}")]
    ReadDir { path: PathBuf, error: io::Error },
}

pub type Result<T> = std::result::Result<T, Error>;

pub enum Direction {
    Backup,
    Restore,
}

//// Recursively copies files from `source` to `dest`. All required folders in `dest`
//// will be created, if necessary.
//fn copy_path(source: &Path, dest: &Path) -> Result<()> {
//    if source.is_dir() {
//        debug!("{} is a directory", source.to_string_lossy());
//        let dir_contents = fs::read_dir(source).map_err(|err| Error::ReadDir {
//            path: source.to_owned(),
//            error: err,
//        })?;
//
//        for item in dir_contents {
//            let item = item.map_err(|err| Error::ReadDir {
//                path: source.to_owned(),
//                error: err,
//            })?;
//
//            let dest: PathBuf = [dest.as_os_str(), &item.file_name()].iter().collect();
//            copy_path(&item.path(), &dest)?;
//        }
//    } else if source.is_file() {
//        debug!("{} is a file", source.to_string_lossy());
//        if let Some(parent) = dest.parent() {
//            debug!("ensuring parent directories");
//            fs::create_dir_all(parent).map_err(|err| Error::CreateDir {
//                path: dest.to_owned(),
//                error: err,
//            })?;
//        }
//
//        debug!(
//            "Copying {} to {}",
//            source.to_string_lossy(),
//            dest.to_string_lossy()
//        );
//        fs::copy(source.to_owned(), dest).map_err(|err| Error::CopyFile {
//            src: source.to_owned(),
//            dest: dest.to_owned(),
//            error: err,
//        })?;
//    }
//
//    Ok(())
//}

//// Synchronizes all files for the given `game` between the backup `root` folder and the
//// configured source directory.
//fn copy_game(root: &Path, name: &str, game: &Game, dir: Direction) -> Result<()> {
//    for (typ, path) in game.iter() {
//        let backup: PathBuf = [&root.to_string_lossy(), name, typ.to_string().as_str()]
//            .iter()
//            .collect();
//        let (source, dest) = {
//            match dir {
//                Direction::Backup => {
//                    info!("Backing up files for game {} on platform {}", name, typ);
//                    (path, &backup)
//                }
//                Direction::Restore => {
//                    info!("Restoring files for game {} on platform {}", name, typ);
//                    (&backup, path)
//                }
//            }
//        };
//
//        copy_path(source, dest)?;
//    }
//
//    Ok(())
//}

//// Back up the saves for the given `games` to the given `root`.
//pub fn backup(root: &Path, games: &Games) -> Result<()> {
//    for (name, game) in games {
//        info!("Backing up {}", name);
//        copy_game(root, name, game, Direction::Backup)?;
//    }
//
//    Ok(())
//}

//// Restore the saves for the given `games` from the given `root`.
//pub fn restore(root: &Path, games: &Games) -> Result<()> {
//    for (name, game) in games {
//        info!("Restoring {}", name);
//        copy_game(root, name, game, Direction::Restore)?;
//    }
//
//    Ok(())
//}
