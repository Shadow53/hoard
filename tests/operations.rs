use crate::common::base::{DefaultConfigTester, HOARD_ANON_FILE};
use hoard::checkers::history::operation::{Operation, OperationImpl};
use hoard::command::Command;
use hoard::hoard_file::{Checksum, HoardFile};

mod common;

fn last_op(file: &HoardFile) -> Operation {
    Operation::latest_local(HOARD_ANON_FILE, Some((None, file.relative_path())))
        .expect("finding a recent operation should not fail")
        .expect("a recent operation should exist")
}

fn last_checksum(file: &HoardFile) -> Checksum {
    last_op(file)
        .checksum_for(None, file.relative_path())
        .expect("checksum should exist for file")
}

fn current_checksum(file: &HoardFile) -> Checksum {
    file.system_sha256()
        .expect("getting checksum should not fail")
        .expect("file should exist to checksum")
}

fn assert_matching_checksum(file: &HoardFile) {
    assert_eq!(last_checksum(file), current_checksum(file));
}

fn assert_not_matching_checksum(file: &HoardFile) {
    assert_ne!(last_checksum(file), current_checksum(file));
}

#[test]
#[serial_test::serial]
fn test_operations() {
    let mut tester = DefaultConfigTester::new();
    tester.use_first_env();
    tester.setup_files();

    let file = tester.anon_file();
    let backup = Command::Backup { hoards: Vec::new() };
    // 1 - Command should work because it is the first backup
    tester.use_local_uuid();
    tester.expect_command(backup.clone());

    // 2 - Command should work because all files are the same
    tester.use_remote_uuid();
    tester.expect_command(backup.clone());

    // 3 - Modify file and back up again. Should succeed because this id has the most recent backup
    assert_matching_checksum(&file);
    common::create_file_with_random_data::<2048>(file.system_path());
    assert_not_matching_checksum(&file);
    tester.expect_command(backup.clone());
    assert_matching_checksum(&file);

    // 4 - Swap UUIDs, change file content, try backup again. Should fail.
    tester.use_local_uuid();
    // latest checksum for this id should not match
    assert_not_matching_checksum(&file);
    common::create_file_with_random_data::<2048>(file.system_path());

    // TODO: assert error
    let _error_1 = tester
        .run_command(backup.clone())
        .expect_err("backup should fail because this id does not have latest backup");

    // 5 - should fail when trying again
    let _error_2 = tester
        .run_command(backup.clone())
        .expect_err("backup should *still* fail because this id does not have latest backup");

    // 6 - should now work because it's forced
    tester.expect_forced_command(backup);
    assert_matching_checksum(&file);
}
