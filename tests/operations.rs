use crate::common::base::{DefaultConfigTester, HOARD_ANON_FILE};
use hoard::checkers::history::operation::{Operation, OperationImpl};
use hoard::checksum::Checksum;
use hoard::command::Command;
use hoard::hoard_item::HoardItem;
use hoard::newtypes::PileName;

mod common;

async fn last_op(file: &HoardItem) -> Operation {
    Operation::latest_local(
        &HOARD_ANON_FILE.parse().unwrap(),
        Some((&PileName::anonymous(), file.relative_path())),
    )
    .await
    .expect("finding a recent operation should not fail")
    .expect("a recent operation should exist")
}

async fn last_checksum(file: &HoardItem) -> Checksum {
    last_op(file)
        .await
        .checksum_for(&PileName::anonymous(), file.relative_path())
        .expect("checksum should exist for file")
}

async fn current_checksum(file: &HoardItem) -> Checksum {
    file.system_sha256()
        .await
        .expect("getting checksum should not fail")
        .expect("file should exist to checksum")
}

async fn assert_matching_checksum(file: &HoardItem) {
    assert_eq!(last_checksum(file).await, current_checksum(file).await);
}

async fn assert_not_matching_checksum(file: &HoardItem) {
    assert_ne!(last_checksum(file).await, current_checksum(file).await);
}

#[tokio::test]
async fn test_operations() {
    let mut tester = DefaultConfigTester::new().await;
    tester.use_first_env();
    tester.setup_files().await;

    let file = tester.anon_file();
    let backup = Command::Backup { hoards: Vec::new() };
    // 1 - Command should work because it is the first backup
    tester.use_local_uuid().await;
    tester.expect_command(backup.clone()).await;

    // 2 - Command should work because all files are the same
    tester.use_remote_uuid().await;
    tester.expect_command(backup.clone()).await;

    // 3 - Modify file and back up again. Should succeed because this id has the most recent backup
    assert_matching_checksum(&file).await;
    common::create_file_with_random_data::<2048>(file.system_path()).await;
    assert_not_matching_checksum(&file).await;
    tester.expect_command(backup.clone()).await;
    assert_matching_checksum(&file).await;

    // 4 - Swap UUIDs, change file content, try backup again. Should fail.
    tester.use_local_uuid().await;
    // latest checksum for this id should not match
    assert_not_matching_checksum(&file).await;
    common::create_file_with_random_data::<2048>(file.system_path()).await;

    // TODO: assert error
    let _error_1 = tester
        .run_command(backup.clone())
        .await
        .expect_err("backup should fail because this id does not have latest backup");

    // 5 - should fail when trying again
    let _error_2 = tester
        .run_command(backup.clone())
        .await
        .expect_err("backup should *still* fail because this id does not have latest backup");

    // 6 - should now work because it's forced
    tester.expect_forced_command(backup).await;
    assert_matching_checksum(&file).await;
}
