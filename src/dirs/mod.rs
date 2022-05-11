//! Functions to determine special folders for Hoard to work with on different platforms.
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
#[cfg(windows)]
pub use windows::Win32::UI::Shell::{FOLDERID_Profile, FOLDERID_RoamingAppData};

#[cfg(unix)]
use unix as sys;
#[cfg(windows)]
use win as sys;
#[cfg(windows)]
pub use win::{get_known_folder, set_known_folder};

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod win;

/// The TLD portion of the application identifier.
pub const TLD: &str = "com";
/// The Company portion of the application identifier.
pub const COMPANY: &str = "shadow53";
/// The Project Name portion of the application identifier.
pub const PROJECT: &str = "hoard";
/// The environment variable that takes precendence over data dir detection.
pub const DATA_DIR_ENV: &str = "HOARD_DATA_DIR";
/// The environment variable that takes precendence over config dir detection.
pub const CONFIG_DIR_ENV: &str = "HOARD_CONFIG_DIR";

static EMPTY_SPAN: Lazy<tracing::Span> = Lazy::new(|| tracing::trace_span!("get_dir_path"));

#[inline]
#[tracing::instrument(level = "trace")]
fn path_from_env(var: &str) -> Option<PathBuf> {
    match std::env::var_os(var).map(PathBuf::from) {
        None => {
            tracing::trace!("could not find path in env var {}", var);
            None
        }
        Some(path) => {
            tracing::trace!("found {} = {}", var, path.display());
            Some(path)
        }
    }
}

/// Returns the current user's home directory.
///
/// - Windows: The "known folder" `FOLDERID_Profile`, fallback to `%USERPROFILE%`.
/// - macOS/Linux/BSD: The value of `$HOME`.
#[must_use]
#[inline]
pub fn home_dir() -> PathBuf {
    let _span = tracing::trace_span!(parent: &*EMPTY_SPAN, "home_dir").entered();
    sys::home_dir()
}

/// Returns Hoard's configuration directory for the current user.
///
/// Returns the contents of `HOARD_CONFIG_DIR`, if set, otherwise:
///
/// - Windows: `{appdata}/shadow53/hoard/config` where `{appdata}` is the "known folder"
///   `FOLDERID_RoamingAppData` or the value of `%APPDATA%`.
/// - macOS: `${XDG_CONFIG_HOME}/hoard`, if `XDG_CONFIG_HOME` is set, otherwise
///   `$HOME/Library/Application Support/com.shadow53.hoard`.
/// - Linux/BSD: `${XFG_CONFIG_HOME}/hoard`, if `XDG_CONFIG_HOME` is set, otherwise `$HOME/.config/hoard`.
#[must_use]
#[inline]
pub fn config_dir() -> PathBuf {
    let _span = tracing::trace_span!(parent: &*EMPTY_SPAN, "config_dir").entered();
    path_from_env(CONFIG_DIR_ENV).unwrap_or_else(sys::config_dir)
}

/// Returns Hoard's data directory for the current user.
///
/// - Windows: `{appdata}/shadow53/hoard/data` where `{appdata}` is the "known folder"
///   `FOLDERID_RoamingAppData` or the value of `%APPDATA%`.
/// - macOS: `${XDG_DATA_HOME}/hoard`, if `XDG_DATA_HOME` is set, otherwise
///   `$HOME/Library/Application Support/com.shadow53.hoard`.
/// - Linux/BSD: `${XFG_DATA_HOME}/hoard`, if `XDG_DATA_HOME` is set, otherwise `$HOME/.local/share/hoard`.
#[must_use]
#[inline]
pub fn data_dir() -> PathBuf {
    let _span = tracing::trace_span!(parent: &*EMPTY_SPAN, "data_dir").entered();
    path_from_env(DATA_DIR_ENV).unwrap_or_else(sys::data_dir)
}

/// Set the environment variable that overrides Hoard's config directory.
///
/// See [`CONFIG_DIR_ENV`].
#[tracing::instrument(level = "trace")]
pub fn set_config_dir(path: &Path) {
    std::env::set_var(CONFIG_DIR_ENV, path);
}

/// Set the environment variable that overrides Hoard's data directory.
///
/// See [`DATA_DIR_ENV`].
#[tracing::instrument(level = "trace")]
pub fn set_data_dir(path: &Path) {
    std::env::set_var(DATA_DIR_ENV, path);
}

#[cfg(test)]
mod tests {
    use std::env;

    use super::*;

    #[test]
    fn test_env_config_dir() {
        env::remove_var(CONFIG_DIR_ENV);
        let original = config_dir();
        let new_path = PathBuf::from("/env/config/dir");
        assert_ne!(original, new_path);
        set_config_dir(&new_path);
        assert_eq!(new_path.as_os_str(), env::var_os(CONFIG_DIR_ENV).unwrap());
        assert_eq!(new_path, config_dir());
    }

    #[test]
    fn test_env_data_dir() {
        env::remove_var(DATA_DIR_ENV);
        let original = data_dir();
        let new_path = PathBuf::from("/env/data/dir");
        assert_ne!(original, new_path);
        set_data_dir(&new_path);
        assert_eq!(new_path.as_os_str(), env::var_os(DATA_DIR_ENV).unwrap());
        assert_eq!(new_path, data_dir());
    }
}
