mod common;

use common::tester::Tester;
use hoard::checkers::history::operation::{Operation, OperationImpl};
use hoard::command::Command;
use hoard::hoard_item::Checksum;
use hoard::paths::RelativePath;
use md5::{Digest as _, Md5};
use sha2::{Digest as _, Sha256};
use std::fs;

const CONFIG: &str = r#"
exclusivity = [[ "unix", "windows" ]]

[envs]
[envs.unix]
    os = ["linux", "macos"]
    env = [{ var = "HOME" }]
[envs.windows]
    os = ["windows"]
    env = [{ var = "USERPROFILE" }]

[hoards]
[hoards.md5]
    config = { hash_algorithm = "md5" }
    "unix" = "${HOME}/testing.txt"
    "windows" = "${USERPROFILE}/testing.txt"
[hoards.sha256]
    config = { hash_algorithm = "sha256" }
    "unix" = "${HOME}/testing.txt"
    "windows" = "${USERPROFILE}/testing.txt"
[hoards.default]
    "unix" = "${HOME}/testing.txt"
    "windows" = "${USERPROFILE}/testing.txt"
"#;

#[test]
fn test_operation_checksums() {
    let tester = Tester::new(CONFIG);
    let file_path = tester.home_dir().join("testing.txt");
    // Relative path for a file is ""
    let rel_file = RelativePath::none();
    common::create_file_with_random_data::<2048>(&file_path);

    tester.expect_command(Command::Backup { hoards: Vec::new() });

    let data = fs::read(&file_path).expect("reading data from test file should succeed");
    let md5 = Checksum::MD5(format!("{:x}", Md5::digest(&data)));
    let sha256 = Checksum::SHA256(format!("{:x}", Sha256::digest(&data)));

    let md5_op = Operation::latest_local("md5", Some((None, &rel_file)))
        .expect("should not fail to load operation for md5 hoard")
        .expect("operation should exist")
        .checksum_for(None, &rel_file)
        .expect("checksum should exist for file");
    let sha256_op = Operation::latest_local("sha256", Some((None, &rel_file)))
        .expect("should not fail to load operation for sha256 hoard")
        .expect("operation should exist")
        .checksum_for(None, &rel_file)
        .expect("checksum should exist for file");
    let default_op = Operation::latest_local("default", Some((None, &rel_file)))
        .expect("should not fail to load operation for default hoard")
        .expect("operation should exist")
        .checksum_for(None, &rel_file)
        .expect("checksum should exist for file");

    assert_eq!(md5_op, md5);
    assert_eq!(sha256_op, sha256);
    assert_eq!(default_op, sha256);
}
