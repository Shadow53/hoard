use super::Editor;
use registry::{key::Error as RegError, Data, Hive, Security};
use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

const EDITOR_NAME: &str = "editor.ps1";
const EDITOR_ID: &str = "hoard.test.editor";

fn set_reg_key(key: &str, name: &str, val: &Data) {
    let reg = Hive::CurrentUser
        .open(key, Security::AllAccess)
        .or_else(|err| match err {
            RegError::NotFound(_, _) => Hive::CurrentUser.create(key, Security::AllAccess),
            _ => Err(err),
        })
        .expect("opening/creating registry key should not fail");
    reg.set_value(name, val)
        .expect("setting registry value should not fail");
}

fn get_reg_key(key: &str, name: &str) -> Option<Data> {
    match Hive::CurrentUser.open(key, Security::Read) {
        Ok(reg) => reg
            .value(name)
            .map(Some)
            .expect("reading registry key should not fail"),
        Err(err) => match err {
            RegError::NotFound(_, _) => None,
            _ => panic!("failed to open registry item {}: {:?}", key, err),
        },
    }
}

/// Returns previous default editor
fn set_default_editor(file_type: &str, command: &str) {
    let key = format!("Software\\Classes\\.{}", file_type);
    set_reg_key(&key, "", &Data::String(EDITOR_ID.try_into().unwrap()));

    let key = format!("{}\\OpenWithProgIds", key);
    set_reg_key(&key, &EDITOR_ID, &Data::String("".try_into().unwrap()));

    let key = format!("Software\\Classes\\{}\\shell\\open\\command", EDITOR_ID);
    let data = Data::String(command.try_into().unwrap());
    set_reg_key(&key, "", &data);
}

fn remove_default_editor() {
    let key = format!("Software\\Classes\\{}", EDITOR_ID);
    Hive::CurrentUser.delete(key, true).unwrap();
}

pub struct EditorGuard {
    temp_dir: TempDir,
    script_file: PathBuf,
    old_path: OsString,
    modified_registry: bool,
}

impl EditorGuard {
    pub fn script_path(&self) -> &Path {
        &self.script_file
    }
}

impl Drop for EditorGuard {
    fn drop(&mut self) {
        if self.modified_registry {
            remove_default_editor();
        }
        std::env::set_var("PATH", &self.old_path);
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
        modified_registry: false,
    }
}

const SHELL_EDITOR_COMMAND: &str = "Unknown\\shell\\editor\\command";
const SHELL_OPEN_COMMAND: &str = "Unknown\\shell\\Open\\command";
const TXTFILE_OPEN_COMMAND: &str = "txtfile\\shell\\Open\\command";

pub fn set_default_gui_editor(editor: Editor) -> EditorGuard {
    let mut guard = create_script_file(editor);
    let command = format!(
        "powershell.exe -Path {} \"%1\"",
        guard.script_path().display()
    );

    for file_type in ["toml", "yaml", "yml"] {
        set_default_editor(file_type, &command);
    }

    guard.modified_registry = true;
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
