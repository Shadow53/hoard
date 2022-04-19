mod common;

use common::base::{HOARD_ANON_DIR, HOARD_ANON_FILE, HOARD_NAMED};
use common::tester::Tester;

use common::base::DefaultConfigTester;
use hoard::checkers::history::last_paths::{
    Error as LastPathsError, HoardPaths, LastPaths, PilePaths,
};
use hoard::checkers::Error as CheckerError;
use hoard::command::{BackupRestoreError, Command, Error as CommandError};
use hoard::config::Error as ConfigError;
use hoard::paths::SystemPath;

async fn assert_expected_paths(tester: &Tester, expected: &LastPaths) {
    let current = LastPaths::from_default_file().await.expect("reading last_paths.json should not fail");
    let anon_file = HOARD_ANON_FILE.parse().unwrap();
    let anon_dir = HOARD_ANON_DIR.parse().unwrap();
    let named = HOARD_NAMED.parse().unwrap();
    assert_eq!(
        current.hoard(&anon_file).expect("hoard should exist").piles,
        expected
            .hoard(&anon_file)
            .expect("hoard should exist")
            .piles,
        "Expected: {:#?}\nReceived{:#?}\n{}",
        expected,
        current,
        tester.extra_logging_output().await
    );
    assert_eq!(
        current.hoard(&anon_dir).expect("hoard should exist").piles,
        expected.hoard(&anon_dir).expect("hoard should exist").piles,
        "Expected: {:#?}\nReceived{:#?}\n{}",
        expected,
        current,
        tester.extra_logging_output().await
    );
    assert_eq!(
        current.hoard(&named).expect("hoard should exist").piles,
        expected.hoard(&named).expect("hoard should exist").piles,
        "Expected: {:#?}\nReceived{:#?}\n{}",
        expected,
        current,
        tester.extra_logging_output().await
    );
}

#[tokio::test]
async fn test_last_paths() {
    let mut tester = DefaultConfigTester::with_log_level(tracing::Level::TRACE).await;

    let timestamp = time::OffsetDateTime::now_utc();
    let first_env_paths = LastPaths::from(maplit::hashmap! {
        HOARD_ANON_FILE.parse().unwrap() => HoardPaths {
            timestamp,
            piles: PilePaths::Anonymous(
                Some(SystemPath::try_from(tester.home_dir().join("first_anon_file")).unwrap())
            )
        },
        HOARD_ANON_DIR.parse().unwrap() => HoardPaths {
            timestamp,
            piles: PilePaths::Anonymous(
                Some(SystemPath::try_from(tester.home_dir().join("first_anon_dir")).unwrap())
            )
        },
        HOARD_NAMED.parse().unwrap() => HoardPaths {
            timestamp,
            piles: PilePaths::Named(
                maplit::hashmap! {
                    "file".parse().unwrap() =>
                        SystemPath::try_from(tester.home_dir().join("first_named_file")).unwrap(),
                    "dir1".parse().unwrap() =>
                        SystemPath::try_from(tester.home_dir().join("first_named_dir1")).unwrap(),
                    "dir2".parse().unwrap() =>
                        SystemPath::try_from(tester.home_dir().join("first_named_dir2")).unwrap()
                }
            )
        }
    });

    let second_env_paths = LastPaths::from(maplit::hashmap! {
        HOARD_ANON_FILE.parse().unwrap() => HoardPaths {
            timestamp,
            piles: PilePaths::Anonymous(
                Some(SystemPath::try_from(tester.home_dir().join("second_anon_file")).unwrap())
            )
        },
        HOARD_ANON_DIR.parse().unwrap() => HoardPaths {
            timestamp,
            piles: PilePaths::Anonymous(
                Some(SystemPath::try_from(tester.home_dir().join("second_anon_dir")).unwrap())
            )
        },
        HOARD_NAMED.parse().unwrap() => HoardPaths {
            timestamp,
            piles: PilePaths::Named(
                maplit::hashmap! {
                    "file".parse().unwrap() =>
                        SystemPath::try_from(tester.home_dir().join("second_named_file")).unwrap(),
                    "dir1".parse().unwrap() =>
                        SystemPath::try_from(tester.home_dir().join("second_named_dir1")).unwrap(),
                    "dir2".parse().unwrap() =>
                        SystemPath::try_from(tester.home_dir().join("second_named_dir2")).unwrap()
                }
            )
        }
    });

    let backup = Command::Backup { hoards: Vec::new() };
    tester.setup_files().await;

    tester.use_first_env();

    // Running twice should succeed
    tester.expect_command(backup.clone()).await;
    tester.expect_command(backup.clone()).await;
    assert_expected_paths(&tester, &first_env_paths).await;

    // Switching environments (thus paths) should fail
    tester.use_second_env();

    let error = tester
        .run_command(backup.clone())
        .await
        .expect_err("changing environment should have caused last_paths to fail");
    assert!(matches!(
        error,
        ConfigError::Command(CommandError::Backup(BackupRestoreError::Consistency(
            CheckerError::LastPaths(LastPathsError::HoardPathsMismatch)
        )))
    ));
    assert_expected_paths(&tester, &first_env_paths).await;

    // Mismatched paths should not be saved, so first env should succeed still
    tester.use_first_env();

    tester.expect_command(backup.clone()).await;
    assert_expected_paths(&tester, &first_env_paths).await;

    tester.use_second_env();

    tester.expect_forced_command(backup).await;
    assert_expected_paths(&tester, &second_env_paths).await;
}
