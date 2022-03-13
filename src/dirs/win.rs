use std::ffi::OsString;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use super::{path_from_env, COMPANY, PROJECT};
use windows::core::{GUID, PCWSTR, PWSTR, Result as WinResult};
use windows::Win32::Foundation::HANDLE;
use windows::Win32::UI::Shell::{SHGetKnownFolderPath, SHSetKnownFolderPath, KF_FLAG_CREATE};
// Prefer KnownFolderID but fall back to environment variables otherwise
// TODO: Convert KnownFolderId to FOLDERID_* GUID?

pub use windows::Win32::UI::Shell::{FOLDERID_Profile, FOLDERID_RoamingAppData};

#[allow(unsafe_code)]
fn pwstr_len(pwstr: PWSTR) -> usize {
    unsafe {
        let mut size = 0_usize;
        let mut i = 0_isize;
        loop {
            if pwstr.0.offset(i).is_null() {
                return size;
            }
            i = i.checked_add(1)
                .expect("raw Windows string was not null-terminated");
            size = size.checked_add(1)
                .expect("raw Windows string was not null-terminated");
        }
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
        let flag = KF_FLAG_CREATE.0.try_into()
            .expect("flag value should always be a non-negative integer");
        SHGetKnownFolderPath(&folder_id, flag, HANDLE(0))
            .map(|pwstr| {
                let slice = std::slice::from_raw_parts(pwstr.0, pwstr_len(pwstr));
                PathBuf::from(OsString::from_wide(slice))
            })
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
        SHSetKnownFolderPath(&folder_id, 0, HANDLE(0), new_path)
    }
}

#[must_use]
pub fn home_dir() -> PathBuf {
    get_known_folder(FOLDERID_Profile).ok()
        .or_else(|| {
            path_from_env("USERPROFILE")
        })
        .expect("could not determine user home directory")
}

#[inline]
fn appdata() -> PathBuf {
    get_known_folder(FOLDERID_Profile).ok()
        .or_else(|| {
            path_from_env("APPDATA")
        })
        .unwrap_or_else(|| {
            home_dir().join("AppData")
        })
        .join(COMPANY).join(PROJECT)
}

#[must_use]
pub fn config_dir() -> PathBuf {
    appdata().join("config")
}

#[must_use]
pub fn data_dir() -> PathBuf {
    appdata().join("data")
}
