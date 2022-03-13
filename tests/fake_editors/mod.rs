#![allow(unused)]

#[cfg(windows)]
mod windows;

#[cfg(unix)]
mod unix;

#[cfg(unix)]
use unix as sys;

#[cfg(windows)]
use self::windows as sys;

pub use sys::EditorGuard;

#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub enum Editor {
    Good,
    Error,
}

impl Editor {
    #[cfg(unix)]
    pub const fn file_content(&self) -> &'static str {
        match self {
            Editor::Good => include_str!("fake-editor.sh"),
            Editor::Error => include_str!("fake-error-editor.sh"),
        }
    }

    #[cfg(windows)]
    pub const fn file_content(&self) -> &'static str {
        match self {
            Editor::Good => include_str!("fake-editor.ps1"),
            Editor::Error => include_str!("fake-error-editor.ps1"),
        }
    }

    #[inline]
    pub fn is_good(&self) -> bool {
        matches!(self, Editor::Good)
    }

    #[inline]
    pub fn is_bad(&self) -> bool {
        matches!(self, Editor::Error)
    }

    pub fn set_as_default_cli_editor(&self) -> sys::EditorGuard {
        sys::set_default_cli_editor(*self)
    }

    pub fn set_as_default_gui_editor(&self) -> sys::EditorGuard {
        std::env::remove_var("EDITOR");
        // xdg-open tries to open the file in a browser if the editor command does not
        // return success. This will cause it to short circuit for testing purposes.
        std::env::set_var("BROWSER", ":");
        sys::set_default_gui_editor(*self)
    }
}
