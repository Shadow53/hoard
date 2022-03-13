use std::path::PathBuf;

#[cfg(unix)]
mod unix;
#[cfg(windows)]
mod win;

#[cfg(unix)]
pub use unix::{config_dir, data_dir, home_dir};
#[cfg(windows)]
pub use win::{config_dir, data_dir, home_dir, get_known_folder, set_known_folder, FOLDERID_Profile, FOLDERID_RoamingAppData};

#[cfg(target_os = "macos")]
const TLD: &str = "com";

#[cfg(any(windows, target_os = "macos"))]
const COMPANY: &str = "shadow53";

const PROJECT: &str = "hoard";

#[inline]
fn path_from_env(var: &str) -> Option<PathBuf> {
    std::env::var_os(var).map(PathBuf::from)
}


