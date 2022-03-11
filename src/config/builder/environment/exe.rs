//! See [`ExeExists`].

use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::fmt::Formatter;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::{fmt, fs};
use thiserror::Error;

/// A conditional structure that tests if the given executable exists in the `$PATH` environment
/// variable.
///
/// See the documentation for [`which`] for more information on how detection works.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Hash)]
#[serde(transparent)]
#[allow(clippy::module_name_repetitions)]
pub struct ExeExists(pub String);

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to read metadata for path: {0}")]
    Metadata(std::io::Error),
    #[error("cannot determine if executable in PATH: PATH is not set")]
    NoPath,
}

#[cfg(windows)]
fn is_executable(file: &Path) -> Result<bool, Error> {
    if file.exists() {
        fs::metadata(file)
            .map(|meta| meta.is_file())
            .map_err(Error::Metadata)
    } else {
        Ok(false)
    }
}

#[cfg(unix)]
fn is_executable(file: &Path) -> Result<bool, Error> {
    if file.exists() {
        fs::metadata(file)
            .map(|meta| {
                // 1 == executable bit in octal (2 == read, 4 == write)
                meta.is_file() && meta.mode() & 0o000111 != 0
            })
            .map_err(Error::Metadata)
    } else {
        Ok(false)
    }
}

fn exe_in_path(name: &str) -> Result<bool, Error> {
    let exe_path = PathBuf::from(name);
    if exe_path.is_absolute() {
        is_executable(&exe_path)
    } else {
        let path = std::env::var_os("PATH").ok_or(Error::NoPath)?;
        println!("PATH = {}", path.to_str().unwrap());
        std::env::split_paths(&path)
            .map(|path| {
                let path = path.join(name);
                is_executable(&path)
            })
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
