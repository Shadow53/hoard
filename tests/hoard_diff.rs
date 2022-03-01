mod common;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use common::test_helper::Tester;
use hoard::command::Command;
use paste::paste;

const DIFF_TOML: &str = r#"
exclusivity = [
    ["first", "second"],
    ["unix", "windows"]
]

[envs]
[envs.windows]
    os = ["windows"]
[[envs.windows.env]]
    var = "HOMEPATH"
[envs.unix]
    os = ["linux", "macos"]
[[envs.unix.env]]
    var = "HOME"


[hoards]
[hoards.anon_txt]
    "unix"    = "${HOME}/anon.txt"
    "windows" = "${USERPROFILE}/anon.txt"

[hoards.anon_bin]
    "unix"    = "${HOME}/anon.bin"
    "windows" = "${USERPROFILE}/anon.bin"

[hoards.named]
[hoards.named.text]
    "unix"    = "${HOME}/named.txt"
    "windows" = "${USERPROFILE}/named.txt"
[hoards.named.binary]
    "unix"    = "${HOME}/named.bin"
    "windows" = "${USERPROFILE}/named.bin"

[hoards.anon_dir]
    config = { ignore = ["*ignore*"] }
    "unix"    = "${HOME}/testdir"
    "windows" = "${USERPROFILE}/testdir"
"#;


fn no_op(_tester: &Tester, _path: &Path, _content: Option<Content>, _is_text: bool, _hoard: &str) {}

fn setup_modify(tester: &Tester, path: &Path, content: Option<Content>, is_text: bool, hoard: &str) {
    modify_file(path, content, is_text, hoard);
    tester.use_local_uuid();
    tester.expect_command(Command::Backup { hoards: vec![hoard.to_string()] });
    tester.use_remote_uuid();
    tester.expect_command(Command::Restore { hoards: vec![hoard.to_string()] });
}

fn setup_permissions(tester: &Tester, path: &Path, content: Option<Content>, is_text: bool, hoard: &str) {
    modify_file(path, Some(Content::Data(DEFAULT_CONTENT.clone())), is_text, hoard);
    setup_modify(tester, path, content, is_text, hoard);
}

fn setup_recreate(tester: &Tester, path: &Path, content: Option<Content>, is_text: bool, hoard: &str) {
    modify_file(path, content, is_text, hoard);
    tester.use_local_uuid();
    tester.expect_command(Command::Backup { hoards: vec![hoard.to_string()] });
    modify_file(path, None, is_text, hoard);
    tester.expect_command(Command::Restore { hoards: vec![hoard.to_string()] });
}

fn is_writable(octet: u32) -> bool {
    octet & 0o000400 != 0
}

fn modify_file(path: &Path, content: Option<Content>, is_text: bool, hoard: &str) {
    match content {
        None => if path.exists() {
            fs::remove_file(path).expect("removing file should succeed");
            assert!(!path.exists());
        }
        Some(Content::Data((text, binary))) => {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("should be able to create file parents");
            }

            if is_text {
                fs::write(path, text).expect("writing text to file should succeed");
            } else {
                fs::write(path, binary).expect("writing text to file should succeed");
            }

            assert!(path.exists(), "writing to the {} failed to create file", path.display());
        }
        Some(Content::Perms(octet)) => {
            let file = fs::File::open(path)
                .expect("file should exist and be able to be opened");
            let mut permissions = file.metadata()
                .expect("failed to read file metadata")
                .permissions();
            #[cfg(unix)]
            permissions.set_mode(octet);
            #[cfg(windows)]
            permissions.set_readonly(!is_writable(octet));

            file.set_permissions(permissions)
                .expect("failed to set permissions on file");
        }
    }
}

fn assert_diff_contains(
    tester: &Tester,
    hoard: &str,
    content: String,
    is_partial: bool,
    invert: bool,
    is_verbose: bool,
) {
    tester.use_local_uuid();
    tester.expect_command(Command::Diff { hoard: hoard.to_string(), verbose: is_verbose });
    if invert {
        tester.assert_not_has_output(&content);
    } else if is_partial {
        tester.assert_has_output(&content);
    } else {
        let debug_output = tester.extra_logging_output();
        assert_eq!(tester.output(), content, "{}", debug_output);
    }
}

fn check_created_file(
    tester: &Tester,
    file: &File,
    hoard: &str,
    location: &str,
    is_partial: bool,
    hoard_content: Option<Content>,
    system_content: Option<Content>
) {
    let summary = format!("{}: created {}\n", file.path.display(), location);
    let full_diff = get_full_diff(file, hoard_content, system_content);
    assert_diff_contains(tester, hoard, summary.clone(), is_partial, file.ignored, false);
    assert_diff_contains(tester, hoard, format!("{}{}", summary, full_diff), is_partial, file.ignored, true);
}

fn get_full_diff(file: &File, hoard_content: Option<Content>, system_content: Option<Content>) -> String {
    let hoard_content = match hoard_content {
        None => return String::new(),
        Some(Content::Data((hoard_content, _))) => hoard_content,
        Some(_) => panic!("expected text, not permissions"),
    };

    let system_content = match system_content {
        None => return String::new(),
        Some(Content::Data((system_content, _))) => system_content,
        Some(_) => panic!("expected text, not permissions"),
    };

    if file.is_text && file.hoard_path.is_some() {
        format!(r#"--- {}
+++ {}
@@ -1 +1 @@
-{}
\ No newline at end of file
+{}
\ No newline at end of file

"#,
            file.hoard_path.as_ref().unwrap().display(),
            file.path.display(),
            hoard_content,
            system_content
        )
    } else { String::new() }
}

fn check_modified_file(
    tester: &Tester,
    file: &File,
    hoard: &str,
    location: &str,
    is_partial: bool,
    hoard_content: Option<Content>,
    system_content: Option<Content>
) {
    let file_type = if file.is_text { "text" } else { "binary" };
    let summary = format!("{}: {} file changed {}\n", file.path.display(), file_type, location);
    let full_diff = get_full_diff(file, hoard_content, system_content);
    assert_diff_contains(tester, hoard, summary.clone(), is_partial, file.ignored, false);
    assert_diff_contains(tester, hoard, format!("{}{}", summary, full_diff), is_partial, file.ignored, true);
}

#[cfg(unix)]
fn check_modified_perms(
    tester: &Tester,
    file: &File,
    hoard: &str,
    _location: &str,
    is_partial: bool,
    hoard_content: Option<Content>,
    system_content: Option<Content>
) {
    let hoard_perms = match hoard_content.expect("expected permissions") {
        Content::Data(_) => panic!("expected permissions, not data"),
        Content::Perms(perms) => perms,
    };

    let system_perms = match system_content.expect("expected permissions") {
        Content::Data(_) => panic!("expected permissions, not data"),
        Content::Perms(perms) => perms,
    };

    #[cfg(unix)]
    let (hoard_perms, system_perms) = (format!("{:o}", hoard_perms), format!("{:o}",  system_perms));

    #[cfg(windows)]
    let hoard_perms = if is_writable(hoard_perms) { "writable" } else { "readonly" };

    #[cfg(windows)]
    let system_perms = if is_writable(system_perms) { "writable" } else { "readonly" };

    for verbose in [true, false] {
        assert_diff_contains(
            tester,
            hoard,
            format!("{}: permissions changed: hoard ({}), system ({})\n", file.path.display(), hoard_perms, system_perms),
            is_partial,
            file.ignored,
            verbose
        );
    }
}

fn check_deleted_file(
    tester: &Tester,
    file: &File,
    hoard: &str,
    location: &str,
    is_partial: bool,
    hoard_content: Option<Content>,
    system_content: Option<Content>
) {
    let system_deleted = system_content.is_none();
    let hoard_deleted = hoard_content.is_none();
    
    if system_deleted {
        assert!(!file.path.exists() || file.ignored, "{} was not deleted", file.path.display());
    }
    
    if hoard_deleted {
        assert!(!file.hoard_path.as_ref().unwrap().exists() || file.ignored, "{} was not deleted", file.hoard_path.as_ref().unwrap().display());
    }

    assert_diff_contains(
        tester,
        hoard,
        format!("{}: deleted {}\n", file.path.display(), location),
        is_partial,
        file.ignored,
        false,
    );

    assert_diff_contains(
        tester,
        hoard,
        format!("{}: deleted {}\n", file.path.display(), location),
        is_partial,
        file.ignored,
        true,
    );
}

fn check_recreated_file(
    tester: &Tester,
    file: &File,
    hoard: &str,
    location: &str,
    is_partial: bool,
    hoard_content: Option<Content>,
    system_content: Option<Content>
) {
    assert_diff_contains(tester, hoard, format!("{}: recreated {}\n", file.path.display(), location), is_partial, file.ignored, false);
    assert_diff_contains(tester, hoard, format!("{}: recreated {}\n", file.path.display(), location), is_partial, file.ignored, true);
}

struct File {
    path: PathBuf,
    hoard_path: Option<PathBuf>,
    is_text: bool,
    ignored: bool,
}

enum Content {
    Data((&'static str, [u8; 5])),
    Perms(u32),
}

const DEFAULT_CONTENT: (&str, [u8; 5]) = ("This is a text file", [0x12, 0xFB, 0x3D, 0x00, 0x3A]);
const CHANGED_CONTENT_A: (&str, [u8; 5]) = ("This is different text content", [0x12, 0xFB, 0x45, 0x00, 0x3A]);
const CHANGED_CONTENT_B: (&str, [u8; 5]) = ("This is yet other text content", [0x12, 0xFB, 0x91, 0x00, 0x3A]);

macro_rules! test_diff_type {
    ($({
        name: $name:ident,
        tester: $tester:ident,
        hoard_files: $files:expr,
        contents: {
            default: $default_content:expr,
            changed_a: $changed_content_a:expr,
            changed_b: $changed_content_b:expr
        },
        setup: $setup_fn:ident,
        modify: $modify_fn:ident,
        check: $check_fn:ident
    }),*) => {
        $(paste! {
            #[test]
            #[serial_test::serial]
            fn [<test_ $name _local>]() {
                const LOCATION: &str = "locally";

                for do_backup in [true, false] {
                    let $tester = Tester::new(DIFF_TOML);
                    let hoard_files = $files;
                    for (hoard, files) in &hoard_files {
                        for file in files {
                            $setup_fn(&$tester, &file.path, $default_content, file.is_text, &hoard);
                        }

                        if do_backup {
                            $tester.use_remote_uuid();
                            $tester.expect_command(Command::Backup { hoards: vec![hoard.clone()] });
                            $tester.use_local_uuid();
                            $tester.expect_command(Command::Restore { hoards: vec![hoard.clone()] });
                        }

                        $tester.use_local_uuid();
                        for file in files {
                            $modify_fn(&file.path, $changed_content_a, file.is_text, &hoard);
                            $check_fn(&$tester, &file, &hoard, LOCATION, files.len() > 1, $default_content, $changed_content_a);
                        }
                    }
                }
            }

            #[test]
            #[serial_test::serial]
            fn [<test_ $name _remote>]() {
                const LOCATION: &str = "remotely";
                let $tester = Tester::new(DIFF_TOML);

                let hoard_files = $files;
                for (hoard, files) in hoard_files {
                    for file in &files {
                        $setup_fn(&$tester, &file.path, $default_content, file.is_text, &hoard);
                    }
                    for file in &files {
                        $modify_fn(&file.path, $changed_content_a, file.is_text, &hoard);
                    }
                    $tester.use_remote_uuid();
                    $tester.expect_command(Command::Backup { hoards: vec![hoard.clone()] });
                    for file in &files {
                        $modify_fn(&file.path, $default_content, file.is_text, &hoard);
                    }
                    $tester.use_local_uuid();
                    for file in &files {
                        $check_fn(&$tester, &file, &hoard, LOCATION, files.len() > 1, $changed_content_a, $default_content);
                    }
                }
            }

            #[test]
            #[serial_test::serial]
            fn [<test_ $name _mixed>]() {
                const LOCATION: &str = "locally and remotely";
                let $tester = Tester::new(DIFF_TOML);

                let hoard_files = $files;
                for (hoard, files) in hoard_files {
                    for file in &files {
                        $setup_fn(&$tester, &file.path, $default_content, file.is_text, &hoard);
                    }
                    for file in &files {
                        $modify_fn(&file.path, $changed_content_a, file.is_text, &hoard);
                    }
                    $tester.use_remote_uuid();
                    $tester.expect_command(Command::Backup { hoards: vec![hoard.clone()] });
                    $tester.use_local_uuid();
                    for file in &files {
                        $modify_fn(&file.path, $changed_content_b, file.is_text, &hoard);
                        $check_fn(&$tester, &file, &hoard, LOCATION, files.len() > 1, $changed_content_a, $changed_content_b);
                    }
                }
            }

            #[test]
            #[serial_test::serial]
            fn [<test_ $name _unexpected>]() {
                const LOCATION: &str = "out-of-band";
                let $tester = Tester::new(DIFF_TOML);

                let hoard_files = $files;
                for (hoard, files) in hoard_files {
                    for file in &files {
                        $setup_fn(&$tester, &file.path, $default_content, file.is_text, &hoard);
                    }
                    for file in &files {
                        if let Some(hoard_path) = file.hoard_path.as_ref() {
                            $modify_fn(hoard_path, $changed_content_a, file.is_text, &hoard);
                        }
                        $check_fn(&$tester, &file, &hoard, LOCATION, files.len() > 1, $changed_content_a, $default_content);
                    }
                }
            }

            #[test]
            #[serial_test::serial]
            fn [<test_ $name _unchanged>]() {
                let $tester = Tester::new(DIFF_TOML);

                let hoard_files = $files;
                for (hoard, files) in hoard_files {
                    for file in &files {
                        $setup_fn(&$tester, &file.path, $default_content, file.is_text, &hoard);
                    }
                    $tester.use_local_uuid();
                    $tester.expect_command(Command::Backup { hoards: vec![hoard.clone()] });
                    $tester.expect_command(Command::Diff { hoard: hoard.clone(), verbose: false });
                    assert_eq!($tester.output(), "")
                }
            }
        })*
    }
}

macro_rules! test_diffs {
    ($tester:ident, $files:expr) => {
        test_diff_type! {
            {
                name: create,
                tester: $tester,
                hoard_files: $files,
                contents: {
                    default: None,
                    changed_a: Some(Content::Data(DEFAULT_CONTENT.clone())),
                    changed_b: Some(Content::Data(CHANGED_CONTENT_A.clone()))
                },
                setup: no_op,
                modify: modify_file,
                check: check_created_file
            },
            {
                name: modify,
                tester: $tester,
                hoard_files: $files,
                contents: {
                    default: Some(Content::Data(DEFAULT_CONTENT.clone())),
                    changed_a: Some(Content::Data(CHANGED_CONTENT_A.clone())),
                    changed_b: Some(Content::Data(CHANGED_CONTENT_B.clone()))
                },
                setup: setup_modify,
                modify: modify_file,
                check: check_modified_file
            },
            {
                name: permissions,
                tester: $tester,
                hoard_files: $files,
                contents: {
                    default: Some(Content::Perms(0o100644)),
                    changed_a: Some(Content::Perms(0o100444)),
                    changed_b: Some(Content::Perms(0o100755))
                },
                setup: setup_permissions,
                modify: modify_file,
                check: check_modified_perms
            }
            //{
            //    name: deleted,
            //    tester: $tester,
            //    hoard_files: $files,
            //    contents: {
            //        default: Some(Content::Data(DEFAULT_CONTENT.clone())),
            //        changed_a: None,
            //        changed_b: None
            //    },
            //    setup: setup_modify,
            //    modify: modify_file,
            //    check: check_deleted_file
            //},
            //{
            //    name: recreate,
            //    tester: $tester,
            //    hoard_files: $files,
            //    contents: {
            //        default: None,
            //        changed_a: Some(Content::Data(DEFAULT_CONTENT.clone())),
            //        changed_b: Some(Content::Data(CHANGED_CONTENT_A.clone()))
            //    },
            //    setup: setup_recreate,
            //    modify: modify_file,
            //    check: check_created_file
            //}
        }
    }
}

test_diffs! {
    tester,
    maplit::btreemap! {
        String::from("anon_dir") => vec![
            File {
                path: tester.home_dir().join("testdir").join("test.txt"),
                hoard_path: Some(tester.data_dir().join("hoards").join("anon_dir").join("test.txt")),
                ignored: false,
                is_text: true,
            },
            File {
                path: tester.home_dir().join("testdir").join("test.bin"),
                hoard_path: Some(tester.data_dir().join("hoards").join("anon_dir").join("test.bin")),
                ignored: false,
                is_text: true,
            },
            File {
                path: tester.home_dir().join("testdir").join("ignore.txt"),
                hoard_path: None,
                is_text: true,
                ignored: true,
            },
        ],
        String::from("anon_txt") => vec![
            File {
                path: tester.home_dir().join("anon.txt"),
                hoard_path: Some(tester.data_dir().join("hoards").join("anon_txt")),
                ignored: false,
                is_text: true,
            },
        ],
        String::from("named") => vec![
            File {
                path: tester.home_dir().join("named.txt"),
                hoard_path: Some(tester.data_dir().join("hoards").join("named").join("text")),
                ignored: false,
                is_text: true,
            },
            File {
                path: tester.home_dir().join("named.bin"),
                hoard_path: Some(tester.data_dir().join("hoards").join("named").join("binary")),
                ignored: false,
                is_text: true,
            },
        ],
    }
}
