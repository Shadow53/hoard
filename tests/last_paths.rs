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

fn assert_expected_paths(tester: &Tester, expected: &LastPaths) {
    let current = LastPaths::from_default_file().expect("reading last_paths.json should not fail");
    assert_eq!(
        current
            .hoard("anon_file")
            .expect("hoard should exist")
            .piles,
        expected
            .hoard("anon_file")
            .expect("hoard should exist")
            .piles,
        "Expected: {:#?}\nReceived{:#?}\n{}",
        expected,
        current,
        tester.extra_logging_output()
    );
    assert_eq!(
        current.hoard("anon_dir").expect("hoard should exist").piles,
        expected
            .hoard("anon_dir")
            .expect("hoard should exist")
            .piles,
        "Expected: {:#?}\nReceived{:#?}\n{}",
        expected,
        current,
        tester.extra_logging_output()
    );
    assert_eq!(
        current.hoard("named").expect("hoard should exist").piles,
        expected.hoard("named").expect("hoard should exist").piles,
        "Expected: {:#?}\nReceived{:#?}\n{}",
        expected,
        current,
        tester.extra_logging_output()
    );
}

#[test]
#[serial_test::serial]
fn test_last_paths() {
    let mut tester = DefaultConfigTester::with_log_level(tracing::Level::TRACE);

    let timestamp = time::OffsetDateTime::now_utc();
    let first_env_paths = LastPaths::from(maplit::hashmap! {
        String::from(HOARD_ANON_FILE) => HoardPaths {
            timestamp,
            piles: PilePaths::Anonymous(
                Some(tester.home_dir().join("first_anon_file"))
            )
        },
        String::from(HOARD_ANON_DIR) => HoardPaths {
            timestamp,
            piles: PilePaths::Anonymous(
                Some(tester.home_dir().join("first_anon_dir"))
            )
        },
        String::from(HOARD_NAMED) => HoardPaths {
            timestamp,
            piles: PilePaths::Named(
                maplit::hashmap! {
                    String::from("file") =>
                        tester.home_dir().join("first_named_file"),
                    String::from("dir1") =>
                        tester.home_dir().join("first_named_dir1"),
                    String::from("dir2") =>
                        tester.home_dir().join("first_named_dir2")
                }
            )
        }
    });

    let second_env_paths = LastPaths::from(maplit::hashmap! {
        String::from(HOARD_ANON_FILE) => HoardPaths {
            timestamp,
            piles: PilePaths::Anonymous(
                Some(tester.home_dir().join("second_anon_file"))
            )
        },
        String::from(HOARD_ANON_DIR) => HoardPaths {
            timestamp,
            piles: PilePaths::Anonymous(
                Some(tester.home_dir().join("second_anon_dir"))
            )
        },
        String::from(HOARD_NAMED) => HoardPaths {
            timestamp,
            piles: PilePaths::Named(
                maplit::hashmap! {
                    String::from("file") =>
                        tester.home_dir().join("second_named_file"),
                    String::from("dir1") =>
                        tester.home_dir().join("second_named_dir1"),
                    String::from("dir2") =>
                        tester.home_dir().join("second_named_dir2")
                }
            )
        }
    });

    let backup = Command::Backup { hoards: Vec::new() };
    tester.setup_files();

    tester.use_first_env();

    // Running twice should succeed
    tester.expect_command(backup.clone());
    tester.expect_command(backup.clone());
    assert_expected_paths(&tester, &first_env_paths);

    // Switching environments (thus paths) should fail
    tester.use_second_env();

    let error = tester
        .run_command(backup.clone())
        .expect_err("changing environment should have caused last_paths to fail");
    assert!(matches!(
        error,
        ConfigError::Command(CommandError::Backup(BackupRestoreError::Consistency(
            CheckerError::LastPaths(LastPathsError::HoardPathsMismatch)
        )))
    ));
    assert_expected_paths(&tester, &first_env_paths);

    // Mismatched paths should not be saved, so first env should succeed still
    tester.use_first_env();

    tester.expect_command(backup.clone());
    assert_expected_paths(&tester, &first_env_paths);

    tester.use_second_env();

    tester.expect_forced_command(backup);
    assert_expected_paths(&tester, &second_env_paths);
}
