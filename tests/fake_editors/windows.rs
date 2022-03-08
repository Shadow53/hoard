use super::Editor;
use registry::{Data, Error as RegError, Hive, Security};
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const EDITOR_NAME: &str = "editor.ps1";

fn set_reg_key(key: &str, val: &Data) {
    let reg = Hive::ClassesRoot::create(key, Security::Write | Security::SetValue)
        .expect("opening/creating registry key should not fail");
    reg.set_value(val)
        .expect("setting registry value should not fail");
}

fn get_reg_key(key: &str) -> Option<Data> {
    match Hive::ClassesRoot::open(key, Security::Read) {
        Ok(reg) => reg
            .value()
            .map(Some)
            .expect("reading registry key should not fail"),
        Err(err) => match err {
            RegError::NotFound(_, _) => Ok(None),
            _ => panic!("failed to open registry item {}: {:?}", key, err),
        },
    }
}

pub struct EditorGuard {
    temp_dir: TempDir,
    script_file: PathBuf,
    old_path: OsString,
    old_shell_editor_command: Option<Data>,
    old_shell_open_command: Option<Data>,
    old_txtfile_open_command: Option<Data>,
}

impl EditorGuard {
    pub fn script_path(&self) -> &Path {
        &self.script_file
    }
}

impl Drop for EditorGuard {
    fn drop(&mut self) {
        if let Some(value) = self.old_shell_editor_command {
            set_reg_key(SHELL_EDITOR_COMMAND, &value);
        }

        if let Some(value) = self.old_shell_open_command {
            set_reg_key(SHELL_OPEN_COMMAND, &value);
        }

        if let Some(value) = self.old_txtfile_open_command {
            set_reg_key(TXTFILE_OPEN_COMMAND, &value);
        }

        std::env::set_var("PATH", self.old_path);
    }
}

fn create_script_file(editor: Editor) -> EditorGuard {
    let temp_dir = tempfile::tempdir().expect("creating tempdir should succeed");
    let script_file = temp_dir.path().join(EDITOR_NAME);
    let mut script =
        fs::File::create(&script_file).expect("creating script file should not succeed");
    script
        .write_all(editor.file_content().as_bytes())
        .expect("writing to script file should succeed");

    let old_path = std::env::var_os("PATH").expect("windows systems should always have PATH set");

    EditorGuard {
        temp_dir,
        script_file,
        old_path,
        old_shell_open_command: Option::<Data>::None,
        old_shell_editor_command: Option::<Data>::None,
        old_txtfile_open_command: Option::<Data>::None,
    }
}

const SHELL_EDITOR_COMMAND: &str = "Unknown\\shell\\editor\\command";
const SHELL_OPEN_COMMAND: &str = "Unknown\\shell\\Open\\command";
const TXTFILE_OPEN_COMMAND: &str = "txtfile\\shell\\Open\\command";

pub fn set_default_gui_editor(editor: Editor) -> EditorGuard {
    let mut guard = create_script_file(editor);
    let reg_value = Data::String(
        format!(
            "powershell.exe -Path {} \"%1\"",
            guard.script_path().display()
        )
        .try_into()
        .expect("converting ASCII string to U16 string should not fail"),
    );

    guard.old_shell_editor_command = get_reg_key(SHELL_EDITOR_COMMAND);
    guard.old_shell_open_command = get_reg_key(SHELL_OPEN_COMMAND);
    guard.old_txtfile_open_command = get_reg_key(TXTFILE_OPEN_COMMAND);

    set_reg_key(SHELL_EDITOR_COMMAND, &reg_value);
    set_reg_key(SHELL_OPEN_COMMAND, &reg_value);
    set_reg_key(TXTFILE_OPEN_COMMAND, &reg_value);

    guard
}

pub fn set_default_cli_editor(editor: Editor) -> EditorGuard {
    let mut guard = create_script_file(editor);
    std::env::set_var("EDITOR", EDITOR_NAME);
    let mut path: OsString = guard.temp_dir.path().into();
    path.push(";");
    path.push(&guard.old_path);
    std::env::set_var("PATH", path);
    guard
}
