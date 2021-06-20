//! See [`OperatingSystem`].

use serde::{Deserialize, Serialize};
use std::convert::{Infallible, TryInto};
use std::fmt;
use std::fmt::Formatter;

/// A conditional structure that checks against the operating system `hoard` was compiled for.
///
/// This has the effect of "detecting" the operating system at compile time instead of runtime.
/// The downside is that running `hoard` in [Wine](https://www.winehq.org/) will detect the system
/// as Windows, while running in the Windows Subsystem for Linux or FreeBSD's Linuxulator will
/// detect the system as Linux.
///
/// For possible values to check against, see [`std::env::consts::OS`].
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Hash)]
#[serde(transparent)]
pub struct OperatingSystem(pub String);

impl TryInto<bool> for OperatingSystem {
    type Error = Infallible;

    fn try_into(self) -> Result<bool, Self::Error> {
        let OperatingSystem(expected) = self;
        tracing::trace!(
            os = std::env::consts::OS,
            %expected,
            "checking if current operating system matches expected",
        );
        Ok(expected == std::env::consts::OS)
    }
}

impl fmt::Display for OperatingSystem {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let OperatingSystem(os) = self;
        write!(f, "OPERATING SYSTEM == {}", os)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correct_os() {
        let os = OperatingSystem(std::env::consts::OS.to_owned());
        let is_os: bool = os.try_into().expect("failed to check operating system");
        assert!(is_os);
    }

    #[test]
    #[cfg(not(target_os = "windows"))]
    fn test_incorrect_os() {
        let os = OperatingSystem(String::from("windows"));
        let is_os: bool = os.try_into().expect("failed to check operating system");
        assert!(!is_os);
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn test_incorrect_os() {
        let os = OperatingSystem(String::from("linux"));
        let is_os: bool = os.try_into().expect("failed to check operating system");
        assert!(!is_os);
    }
}
