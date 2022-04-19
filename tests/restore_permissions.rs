mod common;

use common::tester::Tester;
use hoard::command::Command;
#[cfg(unix)]
use std::fs::Permissions;
use tokio::fs;

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

#[tokio::test]
async fn test_default_permissions() {
    let tester = Tester::with_log_level(CONFIG, tracing::Level::DEBUG).await;
    let root = tester.home_dir().join("defaultdir");
    let file = root.join("file");
    let ignored = root.join("subdir").join("ignore");
    let hoards = vec!["default_dir".parse().unwrap()];

    fs::create_dir_all(ignored.parent().unwrap()).await.unwrap();
    fs::write(&file, "test content").await.unwrap();
    fs::write(&ignored, "ignore me!").await.unwrap();
    tester
        .expect_command(Command::Backup {
            hoards: hoards.clone(),
        })
        .await;

    #[cfg(unix)]
    {
        // Set permissions to something other than the expected value.
        fs::set_permissions(tester.home_dir(), Permissions::from_mode(0o755))
            .await
            .unwrap();
        fs::set_permissions(&root, Permissions::from_mode(0o755))
            .await
            .unwrap();
        fs::set_permissions(&ignored.parent().unwrap(), Permissions::from_mode(0o755))
            .await
            .unwrap();
        fs::set_permissions(&file, Permissions::from_mode(0o644))
            .await
            .unwrap();
        fs::set_permissions(&ignored, Permissions::from_mode(0o644))
            .await
            .unwrap();
    }

    tester.expect_command(Command::Restore { hoards }).await;

    let file_perms = fs::metadata(&file).await.unwrap().permissions();
    let dir_perms = fs::metadata(&root).await.unwrap().permissions();

    assert_eq!(
        (false, false),
        (file_perms.readonly(), dir_perms.readonly())
    );

    #[cfg(unix)]
    {
        let home_perms = fs::metadata(tester.home_dir()).await.unwrap().permissions();
        let ignored_dir_perms = fs::metadata(ignored.parent().unwrap())
            .await
            .unwrap()
            .permissions();
        let ignored_perms = fs::metadata(ignored).await.unwrap().permissions();
        assert_eq!(0o100600, file_perms.mode());
        assert_eq!(0o040700, dir_perms.mode());
        assert_eq!(
            0o040755,
            home_perms.mode(),
            "permissions should not be set on the parent of pile root"
        );
        assert_eq!(
            0o040755,
            ignored_dir_perms.mode(),
            "permissions should not be set on the parent of ignored file"
        );
        assert_eq!(
            0o100644,
            ignored_perms.mode(),
            "permissions should not be set on ignored file"
        );
    }
}

#[tokio::test]
async fn test_anon_txt_configured_perms() {
    let tester = Tester::new(CONFIG).await;
    let file = tester.home_dir().join("anon.txt");
    let hoards = vec!["anon_txt".parse().unwrap()];

    fs::write(&file, "content").await.unwrap();

    tester
        .expect_command(Command::Backup {
            hoards: hoards.clone(),
        })
        .await;

    fs::remove_file(&file).await.unwrap();

    tester.expect_command(Command::Restore { hoards }).await;

    assert!(file.exists());
    let perms = fs::metadata(file).await.unwrap().permissions();

    #[cfg(unix)]
    assert_eq!(0o100444, perms.mode());
    assert!(perms.readonly());
}

#[tokio::test]
async fn test_anon_bin_configured_perms() {
    let tester = Tester::new(CONFIG).await;
    let file = tester.home_dir().join("anon.bin");
    let hoards = vec!["anon_bin".parse().unwrap()];

    fs::write(&file, [0xFF, 0xFF, 0xFF, 0xDE]).await.unwrap();

    tester
        .expect_command(Command::Backup {
            hoards: hoards.clone(),
        })
        .await;

    tester.expect_command(Command::Restore { hoards }).await;

    let perms = fs::metadata(file).await.unwrap().permissions();

    #[cfg(unix)]
    assert_eq!(0o100755, perms.mode());
    assert!(!perms.readonly());
}

#[tokio::test]
async fn test_anon_dir_configured_perms() {
    let tester = Tester::with_log_level(CONFIG, tracing::Level::DEBUG).await;
    let root = tester.home_dir().join("testdir");
    let file1 = root.join("file");
    let sub_dir = root.join("subdir");
    let file2 = sub_dir.join("file");
    let hoards = vec!["anon_dir".parse().unwrap()];

    fs::create_dir_all(&sub_dir).await.unwrap();
    fs::write(&file1, "content 1").await.unwrap();
    fs::write(&file2, "content 2").await.unwrap();

    tester
        .expect_command(Command::Backup {
            hoards: hoards.clone(),
        })
        .await;

    fs::remove_dir_all(&sub_dir).await.unwrap();
    fs::remove_file(&file1).await.unwrap();

    tester.expect_command(Command::Restore { hoards }).await;
    println!("{}", tester.output());

    assert!(root.exists());
    assert!(file1.exists());
    assert!(sub_dir.exists());
    assert!(file2.exists());

    let root_perms = fs::metadata(root).await.unwrap().permissions();
    let file1_perms = fs::metadata(file1).await.unwrap().permissions();
    let sub_dir_perms = fs::metadata(sub_dir).await.unwrap().permissions();
    let file2_perms = fs::metadata(file2).await.unwrap().permissions();

    assert_eq!(root_perms, sub_dir_perms);
    assert_eq!(file1_perms, file2_perms);
    #[cfg(unix)]
    assert_eq!(
        (0o100455, 0o040755),
        (file1_perms.mode(), root_perms.mode())
    );
    assert_eq!(
        (true, false),
        (file1_perms.readonly(), root_perms.readonly())
    );
}

#[tokio::test]
async fn test_readonly_dir() {
    let tester = Tester::new(CONFIG).await;
    let root = tester.home_dir().join("readonly");
    let sub_dir = root.join("subdir");
    let file = sub_dir.join("file");
    let hoards = vec!["readonly_dir".parse().unwrap()];

    fs::create_dir_all(&sub_dir).await.unwrap();
    fs::write(&file, "content").await.unwrap();

    tester
        .expect_command(Command::Backup {
            hoards: hoards.clone(),
        })
        .await;

    for path in [&file, &sub_dir, &root] {
        let mut perms = fs::metadata(&path).await.unwrap().permissions();
        perms.set_readonly(false);
        fs::set_permissions(&path, perms.clone()).await.unwrap();
        if path.is_file() {
            fs::remove_file(&path).await.unwrap();
        } else if path.is_dir() {
            perms.set_readonly(true);
            fs::set_permissions(&path, perms).await.unwrap();
        }
    }

    tester.expect_command(Command::Restore { hoards }).await;

    assert!(root.exists());
    assert!(sub_dir.exists());
    assert!(file.exists());

    let root_perms = fs::metadata(root).await.unwrap().permissions();
    let sub_dir_perms = fs::metadata(sub_dir).await.unwrap().permissions();
    let file_perms = fs::metadata(file).await.unwrap().permissions();

    assert_eq!(root_perms, sub_dir_perms);
    #[cfg(unix)]
    assert_eq!((0o100600, 0o040555), (file_perms.mode(), root_perms.mode()));
    assert_eq!(
        (false, true),
        (file_perms.readonly(), root_perms.readonly())
    );
}

#[tokio::test]
async fn test_hoard_file_permissions() {
    let tester = Tester::with_log_level(CONFIG, tracing::Level::DEBUG).await;
    let root = tester.home_dir().join("testdir");
    let file1 = root.join("file");
    let sub_dir = root.join("subdir");
    let file2 = sub_dir.join("file");
    let hoards = vec!["anon_dir".parse().unwrap()];

    fs::create_dir_all(&sub_dir).await.unwrap();
    fs::write(&file1, "content 1").await.unwrap();
    fs::write(&file2, "content 2").await.unwrap();

    tester.expect_command(Command::Backup { hoards }).await;

    let hoard_root = tester.data_dir().join("hoards").join("anon_dir");
    let hoard_file1 = hoard_root.join("file");
    let hoard_subdir = hoard_root.join("subdir");
    let hoard_file2 = hoard_subdir.join("file");

    assert!(hoard_root.exists());
    assert!(hoard_file1.exists());
    assert!(hoard_subdir.exists());
    assert!(hoard_file2.exists());

    let root_perms = fs::metadata(hoard_root).await.unwrap().permissions();
    let file1_perms = fs::metadata(hoard_file1).await.unwrap().permissions();
    let sub_dir_perms = fs::metadata(hoard_subdir).await.unwrap().permissions();
    let file2_perms = fs::metadata(hoard_file2).await.unwrap().permissions();

    assert_eq!(root_perms, sub_dir_perms);
    assert_eq!(file1_perms, file2_perms);
    #[cfg(unix)]
    assert_eq!(
        (0o100600, 0o040700),
        (file1_perms.mode(), root_perms.mode())
    );
    assert_eq!(
        (false, false),
        (file1_perms.readonly(), root_perms.readonly())
    );
}
