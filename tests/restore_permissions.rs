mod common;

use std::fs;
use std::fs::Permissions;
use common::tester::Tester;
use hoard::command::Command;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

const CONFIG: &str = r#"
exclusivity = [
    ["first", "second"],
    ["unix", "windows"]
]

[envs]
[envs.windows]
    os = ["windows"]
[envs.unix]
    os = ["linux", "macos"]


[hoards]
[hoards.anon_txt]
    "unix"    = "${HOME}/anon.txt"
    "windows" = "${HOARD_TMP}/anon.txt"
[hoards.anon_txt.config]
    file_permissions = 0o444

[hoards.anon_bin]
    "unix"    = "${HOME}/anon.bin"
    "windows" = "${HOARD_TMP}/anon.bin"
[hoards.anon_bin.config]
    file_permissions = 0o755

[hoards.readonly_dir]
    "unix"    = "${HOME}/readonly"
    "windows" = "${HOARD_TMP}/readonly"
[hoards.readonly_dir.config]
    folder_permissions = 0o555

[hoards.anon_dir]
    "unix"    = "${HOME}/testdir"
    "windows" = "${HOARD_TMP}/testdir"
[hoards.anon_dir.config.file_permissions]
    is_readable = true
    is_writable = false
    is_executable = false
    others_can_read = true
    others_can_write = false
    others_can_execute = true
[hoards.anon_dir.config.folder_permissions]
    is_readable = true
    is_writable = true
    is_executable = true
    others_can_read = true
    others_can_write = false
    others_can_execute = true

[hoards.default_dir]
    config = { ignore = ["**/ignore"] }
    "unix"    = "${HOME}/defaultdir"
    "windows" = "${HOARD_TMP}/defaultdir"
"#;

#[test]
fn test_default_permissions() {
    let tester = Tester::with_log_level(CONFIG, tracing::Level::DEBUG);
    let root = tester.home_dir().join("defaultdir");
    let file = root.join("file");
    let ignored = root.join("subdir").join("ignore");
    let hoards = vec!["default_dir".parse().unwrap()];

    fs::create_dir_all(ignored.parent().unwrap()).unwrap();
    fs::write(&file, "test content").unwrap();
    fs::write(&ignored, "ignore me!").unwrap();
    tester.expect_command(Command::Backup { hoards: hoards.clone() });

    #[cfg(unix)]
    {
        // Set permissions to something other than the expected value.
        fs::set_permissions(tester.home_dir(), Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&root, Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&ignored.parent().unwrap(), Permissions::from_mode(0o755)).unwrap();
        fs::set_permissions(&file, Permissions::from_mode(0o644)).unwrap();
        fs::set_permissions(&ignored, Permissions::from_mode(0o644)).unwrap();
    }

    tester.expect_command(Command::Restore { hoards: hoards.clone() });

    let file_perms = fs::metadata(&file).unwrap().permissions();
    let dir_perms = fs::metadata(&root).unwrap().permissions();

    assert_eq!((false, false), (file_perms.readonly(), dir_perms.readonly()));

    #[cfg(unix)]
    {
        let home_perms = fs::metadata(tester.home_dir()).unwrap().permissions();
        let ignored_dir_perms = fs::metadata(ignored.parent().unwrap()).unwrap().permissions();
        let ignored_perms = fs::metadata(ignored).unwrap().permissions();
        assert_eq!(0o100600, file_perms.mode());
        assert_eq!(0o040700, dir_perms.mode());
        assert_eq!(0o040755, home_perms.mode(), "permissions should not be set on the parent of pile root");
        assert_eq!(0o040755, ignored_dir_perms.mode(), "permissions should not be set on the parent of ignored file");
        assert_eq!(0o100644, ignored_perms.mode(), "permissions should not be set on ignored file");
    }
}

#[test]
fn test_anon_txt_configured_perms() {
    let tester = Tester::new(CONFIG);
    let file = tester.home_dir().join("anon.txt");
    let hoards = vec!["anon_txt".parse().unwrap()];

    fs::write(&file, "content").unwrap();

    tester.expect_command(Command::Backup { hoards: hoards.clone() });

    fs::remove_file(&file).unwrap();

    tester.expect_command(Command::Restore { hoards: hoards.clone() });

    assert!(file.exists());
    let perms = fs::metadata(file).unwrap().permissions();

    #[cfg(unix)]
    assert_eq!(0o100444, perms.mode());
    assert_eq!(true, perms.readonly());
}

#[test]
fn test_anon_bin_configured_perms() {
    let tester = Tester::new(CONFIG);
    let file = tester.home_dir().join("anon.bin");
    let hoards = vec!["anon_bin".parse().unwrap()];

    fs::write(&file, [0xFF, 0xFF, 0xFF, 0xDE]).unwrap();

    tester.expect_command(Command::Backup { hoards: hoards.clone() });

    tester.expect_command(Command::Restore { hoards: hoards.clone() });

    let perms = fs::metadata(file).unwrap().permissions();

    #[cfg(unix)]
    assert_eq!(0o100755, perms.mode());
    assert_eq!(false, perms.readonly());
}

#[test]
fn test_anon_dir_configured_perms() {
    let tester = Tester::with_log_level(CONFIG, tracing::Level::DEBUG);
    let root = tester.home_dir().join("testdir");
    let file1 = root.join("file");
    let sub_dir = root.join("subdir");
    let file2 = sub_dir.join("file");
    let hoards = vec!["anon_dir".parse().unwrap()];

    fs::create_dir_all(&sub_dir).unwrap();
    fs::write(&file1, "content 1").unwrap();
    fs::write(&file2, "content 2").unwrap();

    tester.expect_command(Command::Backup { hoards: hoards.clone() });

    fs::remove_dir_all(&sub_dir).unwrap();
    fs::remove_file(&file1).unwrap();

    tester.expect_command(Command::Restore { hoards: hoards.clone() });
    println!("{}", tester.output());

    assert!(root.exists());
    assert!(file1.exists());
    assert!(sub_dir.exists());
    assert!(file2.exists());

    let root_perms = fs::metadata(root).unwrap().permissions();
    let file1_perms = fs::metadata(file1).unwrap().permissions();
    let sub_dir_perms = fs::metadata(sub_dir).unwrap().permissions();
    let file2_perms = fs::metadata(file2).unwrap().permissions();

    assert_eq!(root_perms, sub_dir_perms);
    assert_eq!(file1_perms, file2_perms);
    #[cfg(unix)]
    assert_eq!((0o100455, 0o040755), (file1_perms.mode(), root_perms.mode()));
    assert_eq!((true, false), (file1_perms.readonly(), root_perms.readonly()));
}

#[test]
fn test_readonly_dir() {
    let tester = Tester::new(CONFIG);
    let root = tester.home_dir().join("readonly");
    let sub_dir = root.join("subdir");
    let file = sub_dir.join("file");
    let hoards = vec!["readonly_dir".parse().unwrap()];

    fs::create_dir_all(&sub_dir).unwrap();
    fs::write(&file, "content").unwrap();

    tester.expect_command(Command::Backup { hoards: hoards.clone() });
    
    for path in [&file, &sub_dir, &root] {
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(false);
        fs::set_permissions(&path, perms.clone()).unwrap();
        if path.is_file() {
            fs::remove_file(&path).unwrap();
        } else if path.is_dir() {
            perms.set_readonly(true);
            fs::set_permissions(&path, perms).unwrap();
        }
    }

    tester.expect_command(Command::Restore { hoards: hoards.clone() });

    assert!(root.exists());
    assert!(sub_dir.exists());
    assert!(file.exists());

    let root_perms = fs::metadata(root).unwrap().permissions();
    let sub_dir_perms = fs::metadata(sub_dir).unwrap().permissions();
    let file_perms = fs::metadata(file).unwrap().permissions();

    assert_eq!(root_perms, sub_dir_perms);
    #[cfg(unix)]
    assert_eq!((0o100600, 0o040555), (file_perms.mode(), root_perms.mode()));
    assert_eq!((false, true), (file_perms.readonly(), root_perms.readonly()));
}

#[test]
fn test_hoard_file_permissions() {
    let tester = Tester::with_log_level(CONFIG, tracing::Level::DEBUG);
    let root = tester.home_dir().join("testdir");
    let file1 = root.join("file");
    let sub_dir = root.join("subdir");
    let file2 = sub_dir.join("file");
    let hoards = vec!["anon_dir".parse().unwrap()];

    fs::create_dir_all(&sub_dir).unwrap();
    fs::write(&file1, "content 1").unwrap();
    fs::write(&file2, "content 2").unwrap();

    tester.expect_command(Command::Backup { hoards: hoards.clone() });

    let hoard_root = tester.data_dir().join("hoards").join("anon_dir");
    let hoard_file1 = hoard_root.join("file");
    let hoard_subdir = hoard_root.join("subdir");
    let hoard_file2 = hoard_subdir.join("file");

    assert!(hoard_root.exists());
    assert!(hoard_file1.exists());
    assert!(hoard_subdir.exists());
    assert!(hoard_file2.exists());

    let root_perms = fs::metadata(hoard_root).unwrap().permissions();
    let file1_perms = fs::metadata(hoard_file1).unwrap().permissions();
    let sub_dir_perms = fs::metadata(hoard_subdir).unwrap().permissions();
    let file2_perms = fs::metadata(hoard_file2).unwrap().permissions();

    assert_eq!(root_perms, sub_dir_perms);
    assert_eq!(file1_perms, file2_perms);
    #[cfg(unix)]
    assert_eq!((0o100600, 0o040700), (file1_perms.mode(), root_perms.mode()));
    assert_eq!((false, false), (file1_perms.readonly(), root_perms.readonly()));
}