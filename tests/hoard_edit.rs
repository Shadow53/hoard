mod common;
mod fake_editors;

use common::tester::Tester;
use fake_editors::Editor;

use std::fs;
use std::path::Path;
use std::process::Command;

#[cfg(unix)]
use pty_closure::{run_in_pty, Error as PtyError};

use hoard::command::EditError;
use hoard::command::{Command as HoardCommand, Error as CommandError};
use hoard::config::Error as ConfigError;

const WATCHDOG_FILE_NAME: &str = "watchdog.txt";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum InterfaceType {
    CommandLine,
    Graphical,
}

fn error_is_editor_exit(err: &ConfigError) -> bool {
    matches!(
        err,
        ConfigError::Command(CommandError::Edit(EditError::Exit(_)))
    )
}

fn run_hoard_edit(tester: &Tester, interface: InterfaceType, should_fail: bool) {
    let config_path = tester.config_dir().join("config.toml");

    #[cfg(unix)]
    if interface == InterfaceType::CommandLine {
        const GOOD_ERROR: i32 = 1;
        const BAD_ERROR: i32 = 2;
        let result = unsafe {
            run_in_pty(move || {
                tester.run_command(HoardCommand::Edit).map_err(|err| {
                    if should_fail && error_is_editor_exit(&err) {
                        GOOD_ERROR
                    } else {
                        BAD_ERROR
                    }
                })
            })
        };

        if should_fail {
            assert!(
                matches!(result, Err(PtyError::NonZeroExitCode(status)) if status == GOOD_ERROR),
                "expected editor to return failure code"
            );
        } else {
            assert!(
                result.is_ok(),
                "expected editor to exit without error, got {:?}",
                result
            );
            verify_editor_called_on(tester, &config_path);
        }

        return;
    }

    #[cfg(windows)]
    if interface == InterfaceType::CommandLine {}

    let result = tester.run_command(HoardCommand::Edit);
    if should_fail {
        if let Err(err) = result {
            assert!(
                error_is_editor_exit(&err),
                "expected editor to exit with error code, got this instead: {:?}",
                err
            );
        } else {
            panic!("expected editor to exit with error");
        }
    } else {
        assert!(
            result.is_ok(),
            "expected editor to exit without error, got this instead: {:?}",
            result
        );
        verify_editor_called_on(tester, &config_path);
    }
}

fn verify_editor_called_on(tester: &Tester, _file: &Path) {
    let watchdog_path = tester.home_dir().join(WATCHDOG_FILE_NAME);
    assert!(
        watchdog_path.exists(),
        "watchdog file should have been created"
    );
    fs::remove_file(&watchdog_path).expect("deleting the watchdog file should not fail");
}

#[test]
fn verify_watchdog_works() {
    for editor in [Editor::Good, Editor::Error] {
        for interface in [InterfaceType::Graphical, InterfaceType::CommandLine] {
            let tester = Tester::new("");
            let file = tester.home_dir().join("watchdog_test.txt");

            let guard = match interface {
                InterfaceType::Graphical => editor.set_as_default_gui_editor(),
                InterfaceType::CommandLine => editor.set_as_default_cli_editor(),
            };

            #[cfg(unix)]
            let result = Command::new(guard.script_path()).arg(&file).status();

            #[cfg(windows)]
            let result = Command::new("powershell.exe")
                .arg(guard.script_path())
                .arg(&file)
                .status();

            let status =
                result.expect("no I/O errors should have occurred while running the editor");
            if editor.is_good() {
                assert!(status.success(), "expected editor to exit with success");
                verify_editor_called_on(&tester, &file);
            } else {
                assert!(!status.success(), "expected editor to exit with error");
            }
        }
    }
}

// TODO: disabled tests because they are not passing on CI, but do pass when run manually.
// Probably because it expects a graphical environment.

#[test]
#[cfg(not(windows))]
fn test_hoard_edit_good_cli() {
    let tester = Tester::new("");
    let _guard = Editor::Good.set_as_default_cli_editor();
    run_hoard_edit(&tester, InterfaceType::CommandLine, false);
}

#[test]
#[cfg(not(target_os = "macos"))]
#[cfg(not(windows))]
fn test_hoard_edit_good_gui() {
    let tester = Tester::new("");
    let _guard = Editor::Good.set_as_default_gui_editor();
    run_hoard_edit(&tester, InterfaceType::Graphical, false);
}

#[test]
#[cfg(not(windows))]
fn test_hoard_edit_error_cli() {
    let tester = Tester::new("");
    let _guard = Editor::Error.set_as_default_cli_editor();
    run_hoard_edit(&tester, InterfaceType::CommandLine, true);
}

#[test]
#[cfg(not(target_os = "macos"))]
#[cfg(not(windows))]
fn test_hoard_edit_error_gui() {
    let tester = Tester::new("");
    let _guard = Editor::Error.set_as_default_gui_editor();
    run_hoard_edit(&tester, InterfaceType::Graphical, true);
}
