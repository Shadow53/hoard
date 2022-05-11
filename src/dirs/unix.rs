use std::path::PathBuf;

use super::{path_from_env, PROJECT};
#[cfg(target_os = "macos")]
use super::{COMPANY, TLD};

#[tracing::instrument(level = "trace")]
fn xdg_config_dir() -> Option<PathBuf> {
    path_from_env("XDG_CONFIG_HOME").map(|path| path.join(PROJECT))
}

#[tracing::instrument(level = "trace")]
fn xdg_data_dir() -> Option<PathBuf> {
    path_from_env("XDG_DATA_HOME").map(|path| path.join(PROJECT))
}

#[must_use]
#[tracing::instrument(level = "trace")]
pub(super) fn home_dir() -> PathBuf {
    path_from_env("HOME").expect("could not determine user home directory")
}

#[cfg(target_os = "macos")]
#[tracing::instrument(level = "trace")]
fn mac_config_dir() -> PathBuf {
    tracing::trace!("using macos-specific config/data directory");
    home_dir()
        .join("Library")
        .join("Application Support")
        .join(format!("{}.{}.{}", TLD, COMPANY, PROJECT))
}

#[cfg(target_os = "macos")]
#[must_use]
#[tracing::instrument(level = "trace")]
pub(super) fn config_dir() -> PathBuf {
    xdg_config_dir().unwrap_or_else(mac_config_dir)
}

#[cfg(target_os = "macos")]
#[must_use]
#[tracing::instrument(level = "trace")]
pub(super) fn data_dir() -> PathBuf {
    xdg_data_dir().unwrap_or_else(mac_config_dir)
}

#[cfg(not(target_os = "macos"))]
#[must_use]
#[tracing::instrument(level = "trace")]
pub(super) fn config_dir() -> PathBuf {
    xdg_config_dir().unwrap_or_else(|| {
        tracing::trace!("using fallback config directory");
        home_dir().join(".config").join(PROJECT)
    })
}

#[cfg(not(target_os = "macos"))]
#[must_use]
#[tracing::instrument(level = "trace")]
pub(super) fn data_dir() -> PathBuf {
    xdg_data_dir().unwrap_or_else(|| {
        tracing::trace!("using fallback data directory");
        home_dir().join(".local").join("share").join(PROJECT)
    })
}
