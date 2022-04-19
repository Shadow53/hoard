use super::Editor;
use std::ffi::OsString;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::runtime::Handle;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tempfile::TempDir;

const EDITOR_NAME: &str = "com.shadow53.hoard.test-editor";
const EDITOR_DESKTOP: &str = "com.shadow53.hoard-test-editor.desktop";

pub struct EditorGuard {
    temp_dir: TempDir,
    script_file: PathBuf,
    #[cfg(not(target_os = "macos"))]
    desktop_file: Option<PathBuf>,
    old_path: OsString,
}

impl EditorGuard {
    pub fn script_path(&self) -> &Path {
        &self.script_file
    }
}

impl Drop for EditorGuard {
    fn drop(&mut self) {
        #[cfg(not(target_os = "macos"))]
        if self.desktop_file.is_some() {
            Handle::current().block_on(tokio::spawn(xdg_desktop_menu("uninstall", EDITOR_DESKTOP)));
        }

        std::env::set_var("PATH", &self.old_path);
    }
}

#[cfg(not(target_os = "macos"))]
async fn xdg_desktop_menu(command: &str, file_name: &str) {
    let status = Command::new("xdg-desktop-menu")
        .arg(command)
        .arg(file_name)
        .status()
        .await
        .expect("xdg-desktop-menu command should not error");
    assert_eq!(
        status.code(),
        Some(0),
        "xdg-desktop-menu exited with non-zero status"
    );
}

#[cfg(not(target_os = "macos"))]
async fn set_desktop_file_default(mime_type: &str) {
    let status = Command::new("xdg-mime")
        .arg("default")
        .arg(EDITOR_DESKTOP)
        .arg(mime_type)
        .status()
        .await
        .expect("xdg-mime command should not error");
    assert_eq!(
        status.code(),
        Some(0),
        "xdg-mime exited with non-zero status"
    );
    let output = Command::new("xdg-mime")
        .arg("query")
        .arg("default")
        .arg(mime_type)
        .output()
        .await
        .expect("xdg-mime command should not error");
    let as_bytes = EDITOR_DESKTOP.as_bytes();
    assert!(
        output
            .stdout
            .windows(as_bytes.len())
            .any(|window| window == as_bytes),
        "{} does not seem to be correctly set as GUI default",
        EDITOR_DESKTOP
    );
}

async fn create_script_file(editor: Editor) -> EditorGuard {
    let temp_dir = tempfile::tempdir().expect("creating tempdir should succeed");
    let script_file = temp_dir.path().join(EDITOR_NAME);
    let mut script =
        fs::File::create(&script_file).await.expect("creating script file should not succeed");
    script
        .write_all(editor.file_content().as_bytes())
        .await
        .expect("writing to script file should succeed");
    let mut permissions = script
        .metadata()
        .await
        .expect("reading script file metadata should succeed")
        .permissions();
    // Mark script executable
    permissions.set_mode(permissions.mode() | 0o000111);
    script
        .set_permissions(permissions)
        .await
        .expect("making script executable should succeed");

    let old_path = std::env::var_os("PATH").expect("unixy systems should always have PATH set");

    EditorGuard {
        temp_dir,
        script_file,
        #[cfg(not(target_os = "macos"))]
        desktop_file: Option::<PathBuf>::None,
        old_path,
    }
}

#[cfg(target_os = "macos")]
pub fn set_default_gui_editor(editor: Editor) -> EditorGuard {
    unimplemented!("setting default GUI programs on MacOS is non-trivial");
}

#[cfg(not(target_os = "macos"))]
pub async fn set_default_gui_editor(editor: Editor) -> EditorGuard {
    let mut guard = create_script_file(editor).await;
    let desktop_path = guard.temp_dir.path().join(EDITOR_DESKTOP);
    let content = format!(
        r#"[Desktop Entry]
Type=Application
Name=Fake Editor
GenericName=Editor
Categories=System
MimeType=text/plain;application/x-yaml
Exec={} %f
"#,
        guard.script_path().display()
    );
    fs::write(&desktop_path, content).await.expect("writing to desktop file should succeed");
    xdg_desktop_menu("install", &desktop_path.to_string_lossy());
    // The mime type reported by xdg-mime for TOML files
    set_desktop_file_default("text/plain");
    // The mime type reported by xdg-mime for YAML files
    set_desktop_file_default("application/x-yaml");
    guard.desktop_file = Some(desktop_path);

    // These env vars need to be set for the system to use xdg-open
    std::env::set_var("XDG_CURRENT_DESKTOP", "X-Generic");
    std::env::set_var("DISPLAY", ":0");

    guard
}

pub async fn set_default_cli_editor(editor: Editor) -> EditorGuard {
    let guard = create_script_file(editor).await;
    std::env::set_var("EDITOR", EDITOR_NAME);
    let mut path: OsString = guard.temp_dir.path().into();
    path.push(":");
    path.push(&guard.old_path);
    std::env::set_var("PATH", path);
    guard
}
