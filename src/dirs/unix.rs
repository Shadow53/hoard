use std::path::PathBuf;
use super::{path_from_env, PROJECT};

#[cfg(target_os = "macos")]
use super::{TLD, COMPANY};

fn xdg_config_dir() -> Option<PathBuf> {
    path_from_env("XDG_CONFIG_HOME").map(|path| path.join(PROJECT))
}

fn xdg_data_dir() -> Option<PathBuf> {
    path_from_env("XDG_DATA_HOME").map(|path| path.join(PROJECT))
}

#[must_use]
pub fn home_dir() -> PathBuf {
    path_from_env("HOME")
        .expect("could not determine user home directory")
}

#[cfg(target_os = "macos")]
fn mac_config_dir() -> PathBuf {
    home_dir().join("Library").join("Application Support").join(format!("{}.{}.{}", TLD, COMPANY, PROJECT))
}

#[cfg(target_os = "macos")]
#[must_use]
pub fn config_dir() -> PathBuf {
    xdg_config_dir()
        .unwrap_or_else(mac_config_dir)
}

#[cfg(target_os = "macos")]
#[must_use]
pub fn data_dir() -> PathBuf {
    xdg_data_dir()
        .unwrap_or_else(mac_config_dir)
}

#[cfg(not(target_os = "macos"))]
#[must_use]
pub fn config_dir() -> PathBuf {
    xdg_config_dir()
        .unwrap_or_else(|| {
            home_dir().join(".config").join(PROJECT)
        })
}

#[cfg(not(target_os = "macos"))]
#[must_use]
pub fn data_dir() -> PathBuf {
    xdg_data_dir()
        .unwrap_or_else(|| {
            home_dir().join(".local").join("share").join(PROJECT)
        })
}
