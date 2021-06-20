//! See [`ExeExists`].

use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::fmt;
use std::fmt::Formatter;

/// A conditional structure that tests if the given executable exists in the `$PATH` environment
/// variable.
///
/// See the documentation for [`which`] for more information on how detection works.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Hash)]
#[serde(transparent)]
#[allow(clippy::module_name_repetitions)]
pub struct ExeExists(pub String);

impl TryInto<bool> for ExeExists {
    type Error = which::Error;

    fn try_into(self) -> Result<bool, Self::Error> {
        let ExeExists(exe) = self;
        tracing::trace!(%exe, "checking if exe exists in $PATH");
        match which::which(exe) {
            Ok(_) => Ok(true),
            Err(err) => match err {
                // AFAICT, this error is the "exe not found" one.
                which::Error::CannotFindBinaryPath => Ok(false),
                err => Err(err),
            },
        }
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
        for exe in &EXE_NAMES {
            let exists: bool = ExeExists((*exe).to_string())
                .try_into()
                .expect("failed to check if exe exists");

            assert!(exists)
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
