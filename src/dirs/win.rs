use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

use windows::core::{Result as WinResult, GUID, PCWSTR, PWSTR};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::UI::Shell::{FOLDERID_Profile, FOLDERID_RoamingAppData};
use windows::Win32::UI::Shell::{SHGetKnownFolderPath, SHSetKnownFolderPath, KF_FLAG_CREATE};

use super::{path_from_env, COMPANY, PROJECT};

#[allow(unsafe_code)]
fn pwstr_len(pwstr: PWSTR) -> usize {
    unsafe {
        // Not entirely sure if this is correct, but it should be:
        // - The string is always returned from another Windows API
        // - `as_wide` converts into a slice of u16 without '\0'
        // - AFAICT, there are no multi-u16 characters
        pwstr.as_wide().len()
    }
}

/// Get a Windows "Known Folder" by id.
///
/// All ids can be found under [`windows::Win32::UI::Shell`] as `FOLDERID_{Name}`.
///
/// This crate uses and re-exports [`FOLDERID_Profile`] and [`FOLDERID_RoamingAppData`].
///
/// # Errors
///
/// This function will error if [`SHGetKnownFolderPath`] does. See the
/// [official Microsoft docs](https://docs.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shgetknownfolderpath#return-value)
/// for more.
#[allow(unsafe_code)]
pub fn get_known_folder(folder_id: GUID) -> WinResult<PathBuf> {
    unsafe {
        SHGetKnownFolderPath(&folder_id, KF_FLAG_CREATE, HANDLE(std::ptr::null_mut())).map(
            |pwstr| {
                let slice = std::slice::from_raw_parts(pwstr.0, pwstr_len(pwstr));
                PathBuf::from(OsString::from_wide(slice))
            },
        )
    }
}

/// Set a Windows "Known Folder" by id.
///
/// All ids can be found under [`windows::Win32::UI::Shell`] as `FOLDERID_{Name}`.
///
/// This crate uses and re-exports [`FOLDERID_Profile`] and [`FOLDERID_RoamingAppData`].
///
/// # Errors
///
/// This function will error if [`SHSetKnownFolderPath`] does. See the
/// [official Microsoft docs](https://docs.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shsetknownfolderpath#return-value)
/// for more.
#[allow(unsafe_code)]
pub fn set_known_folder(folder_id: GUID, new_path: &Path) -> WinResult<()> {
    unsafe {
        let new_path: Vec<u16> = new_path.as_os_str().encode_wide().chain([0]).collect();
        let new_path = PCWSTR(new_path.as_ptr());
        SHSetKnownFolderPath(&folder_id, 0, HANDLE(std::ptr::null_mut()), new_path)
    }
}

macro_rules! get_and_log_known_folder {
    ($id: ident) => {{
        tracing::trace!("attempting to get known folder {}", std::stringify!($id));
        get_known_folder($id)
    }};
}

#[must_use]
#[tracing::instrument(level = "trace")]
pub(super) fn home_dir() -> PathBuf {
    get_and_log_known_folder!(FOLDERID_Profile)
        .ok()
        .or_else(|| path_from_env("USERPROFILE"))
        .expect("could not determine user home directory")
}

#[inline]
#[tracing::instrument(level = "trace")]
fn appdata() -> PathBuf {
    get_and_log_known_folder!(FOLDERID_RoamingAppData)
        .ok()
        .or_else(|| path_from_env("APPDATA"))
        .unwrap_or_else(|| home_dir().join("AppData").join("Roaming"))
        .join(COMPANY)
        .join(PROJECT)
}

#[must_use]
#[tracing::instrument(level = "trace")]
pub(super) fn config_dir() -> PathBuf {
    appdata().join("config")
}

#[must_use]
#[tracing::instrument(level = "trace")]
pub(super) fn data_dir() -> PathBuf {
    appdata().join("data")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_known_folder_works_correctly() {
        let known_home = get_known_folder(FOLDERID_Profile).unwrap();
        let env_home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap();
        assert_eq!(known_home, env_home);
    }
}
