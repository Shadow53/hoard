//! See [`ExeExists`].

use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::fmt::Formatter;
use std::path::{Path, PathBuf};
use std::{fmt, fs};
use thiserror::Error;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

/// A conditional structure that tests if the given executable exists in the `$PATH` environment
/// variable.
///
/// # Absolute Paths
///
/// If the string provided is an absolute path, that exact path will be tested that:
///
/// - it exists
/// - it is a file
/// - the file is executable (on non-Windows systems)
///
/// # File Names
///
/// On non-Windows, "nix" systems, all `$PATH` paths will be searched for an executable file with
/// the exact name given, including case.
///
/// On Windows, all `%PATH%` paths will be searched for an executable with the exact name OR with
/// that given name and one of the common Windows executable file extensions. As an example,
/// `ExeExists("cmd")` will check `%PATH%` for:
///
/// - `cmd`
/// - `CMD`
/// - `cmd.exe`
/// - `CMD.EXE`
/// - And so on, for extensions `.exe`, `.com`, `.bat`, and `.cmd`
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Hash)]
#[serde(transparent)]
#[allow(clippy::module_name_repetitions)]
pub struct ExeExists(pub String);

/// Errors that may occur while checking for an executable's existence.
#[derive(Debug, Error)]
pub enum Error {
    /// Found a possible candidate in `$PATH` but could not read its metadata.
    #[error("failed to read metadata for {path}: {error}")]
    Metadata {
        /// The path that caused failure.
        path: PathBuf,
        /// The error that occurred.
        #[source]
        error: std::io::Error,
    },

    /// A file name was provided, but `$PATH` was not set.
    #[error("cannot determine if executable in PATH: PATH is not set")]
    NoPath,
}

#[cfg(windows)]
fn is_executable(dir: Option<&Path>, file: &str) -> Result<bool, Error> {
    const EXTS: [&str; 5] = ["", ".exe", ".com", ".bat", ".cmd"];

    for ext in EXTS {
        let file_name = format!("{}{}", file, ext);
        let files = [
            file_name.clone(),
            file_name.to_uppercase(),
            file_name.to_lowercase(),
        ];

        for file in files {
            let file = dir.map_or_else(|| PathBuf::from(&file), |dir| dir.join(&file));
            if file.exists() {
                let is_file = fs::metadata(&file)
                    .map(|meta| meta.is_file())
                    .map_err(|error| Error::Metadata { path: file, error })?;

                if is_file {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}

#[cfg(unix)]
fn is_executable(dir: Option<&Path>, file: &str) -> Result<bool, Error> {
    let file = dir.map_or_else(|| PathBuf::from(&file), |dir| dir.join(&file));
    if file.exists() {
        fs::metadata(&file)
            .map(|meta| {
                // 1 == executable bit in octal (2 == read, 4 == write)
                meta.is_file() && meta.mode() & 0o000_111 != 0
            })
            .map_err(|error| Error::Metadata { path: file, error })
    } else {
        Ok(false)
    }
}

fn exe_in_path(name: &str) -> Result<bool, Error> {
    let exe_path = PathBuf::from(name);
    if exe_path.is_absolute() {
        is_executable(None, name)
    } else {
        let path = std::env::var_os("PATH").ok_or(Error::NoPath)?;
        std::env::split_paths(&path)
            .map(|path| is_executable(Some(&path), name))
            .find(|result| matches!(result, Ok(true) | Err(_)))
            .unwrap_or(Ok(false))
    }
}

impl TryInto<bool> for ExeExists {
    type Error = Error;

    fn try_into(self) -> Result<bool, Self::Error> {
        let ExeExists(exe) = self;
        tracing::trace!(%exe, "checking if exe exists in $PATH");
        exe_in_path(&exe)
    }
}

impl fmt::Display for ExeExists {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let ExeExists(exe) = self;
        write!(f, "EXE {} EXISTS", exe)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "windows")]
    const EXE_NAMES: [&str; 5] = [
        "CMD",
        "CMD.EXE",
        "cmd",
        "cmd.exe",
        r"C:\Windows\System32\cmd.exe",
    ];

    #[cfg(not(target_os = "windows"))]
    const EXE_NAMES: [&str; 5] = ["less", "vi", "sh", "true", "/bin/sh"];

    #[test]
    fn test_exe_exists() {
        println!("{}", std::env::var("PATH").unwrap());
        for exe in &EXE_NAMES {
            let exists: bool = ExeExists((*exe).to_string())
                .try_into()
                .expect("failed to check if exe exists");

            assert!(exists, "exe {} should exist", exe);
        }
    }

    #[test]
    fn test_exe_does_not_exist() {
        let exists: bool = ExeExists(String::from("HoardTestNotExist"))
            .try_into()
            .expect("failed to check if exe exists");
        assert!(!exists);
    }
}
