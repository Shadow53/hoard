mod common;

use std::fs;
use std::io::Write;

use common::tester::Tester;
use hoard::config::builder::{Builder, Error as BuilderError};
use hoard::command::{Command};


#[test]
#[serial_test::serial]
fn test_invalid_uuid() {
    let tester = Tester::new(common::base::BASE_CONFIG);
    let uuid_path = tester.config_dir().join("uuid");
    let bad_content = "INVALID UUID";
    {
        let mut file = fs::File::create(&uuid_path).expect("failed to create uuid file");
        file.write_all(bad_content.as_bytes()).expect("failed to write to uuid file");
    }

    tester.run_command(Command::Backup { hoards: Vec::new() })
        .expect("backup should handle bad uuid file");

    tester.assert_has_output("failed to parse uuid in file");

    let content = fs::read_to_string(&uuid_path).expect("failed to read uuid file");
    assert_ne!(content, bad_content);
}

#[test]
#[serial_test::serial]
fn test_invalid_config_extensions() {
    let tester = Tester::new(common::base::BASE_CONFIG);
    let expected_output = "configuration file must have file extension \"";

    let path = tester.config_dir().join("config_file");
    { fs::File::create(&path).expect("failed to create config_file"); }
    let error = Builder::from_file(&path).expect_err("config file without file extension should fail");
    assert!(matches!(error, BuilderError::InvalidExtension(bad_path) if path == bad_path));

    tester.assert_has_output(expected_output);
    tester.clear_output();

    let path = tester.config_dir().join("config_file.txt");
    { fs::File::create(&path).expect("failed to create config_file.txt"); }
    let error = Builder::from_file(&path).expect_err("config file with bad file extension should fail");
    assert!(matches!(error, BuilderError::InvalidExtension(bad_path) if path == bad_path));

    tester.assert_has_output(expected_output);
}

#[test]
#[serial_test::serial]
fn test_missing_config_dir() {
    let tester = Tester::new(common::base::BASE_CONFIG);
    fs::remove_dir(tester.config_dir()).expect("failed to delete config dir");
    tester.run_command(Command::Backup { hoards: Vec::new() }).expect("running backup without config dir should not fail");
    tester.assert_not_has_output("error while saving uuid to file");
    tester.assert_not_has_output("No such file or directory");
}
