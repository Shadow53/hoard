//! Functions to determine special folders for Hoard to work with on different platforms.
use std::path::PathBuf;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod win;

#[cfg(windows)]
pub use win::{get_known_folder, set_known_folder};
#[cfg(windows)]
pub use windows::Win32::UI::Shell::{FOLDERID_Profile, FOLDERID_RoamingAppData};

/// The TLD portion of the application identifier.
pub const TLD: &str = "com";
/// The Company portion of the application identifier.
pub const COMPANY: &str = "shadow53";
/// The Project Name portion of the application identifier.
pub const PROJECT: &str = "hoard";

#[inline]
fn path_from_env(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}

/// Returns the current user's home directory.
///
/// - Windows: The "known folder" `FOLDERID_Profile`, fallback to `%USERPROFILE%`.
/// - macOS/Linux/BSD: The value of `$HOME`.
#[must_use]
#[inline]
pub fn home_dir() -> PathBuf {
    #[cfg(unix)]
    {
        unix::home_dir()
    }
    #[cfg(windows)]
    {
        win::home_dir()
    }
}

/// Returns Hoard's configuration directory for the current user.
///
/// - Windows: `{appdata}/config` where `{appdata}` is the "known folder" `FOLDERID_RoamingAppData` or
///   the value of `%APPDATA%`.
/// - macOS: `${XDG_CONFIG_HOME}/hoard`, if `XDG_CONFIG_HOME` is set, otherwise
///   `$HOME/Library/Application Support/com.shadow53.hoard`.
/// - Linux/BSD: `${XFG_CONFIG_HOME}/hoard`, if `XDG_CONFIG_HOME` is set, otherwise `$HOME/.config/hoard`.
#[must_use]
#[inline]
pub fn config_dir() -> PathBuf {
    #[cfg(unix)]
    {
        unix::config_dir()
    }
    #[cfg(windows)]
    {
        win::config_dir()
    }
}

/// Returns Hoard's data directory for the current user.
///
/// - Windows: `{appdata}/data` where `{appdata}` is the "known folder" `FOLDERID_RoamingAppData` or
///   the value of `%APPDATA%`.
/// - macOS: `${XDG_DATA_HOME}/hoard`, if `XDG_DATA_HOME` is set, otherwise
///   `$HOME/Library/Application Support/com.shadow53.hoard`.
/// - Linux/BSD: `${XFG_DATA_HOME}/hoard`, if `XDG_DATA_HOME` is set, otherwise `$HOME/.local/share/hoard`.
#[must_use]
#[inline]
pub fn data_dir() -> PathBuf {
    #[cfg(unix)]
    {
        unix::data_dir()
    }
    #[cfg(windows)]
    {
        win::data_dir()
    }
}
