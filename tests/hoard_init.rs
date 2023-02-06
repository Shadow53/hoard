mod common;

use common::tester::Tester;
use hoard::command::Command;
use tokio::fs;

#[tokio::test]
async fn test_hoard_init() {
    let tester = Tester::new("").await;

    fs::remove_dir(tester.config_dir())
        .await
        .expect("should have deleted hoard config dir");
    fs::remove_dir(tester.data_dir())
        .await
        .expect("should have deleted hoard data dir");

    assert!(
        !tester.config_dir().exists(),
        "hoard config directory should not exist"
    );
    assert!(
        !tester.data_dir().exists(),
        "hoard data directory should not exist"
    );

    tester
        .run_command(Command::Init)
        .await
        .expect("initialization should succeed");

    assert!(
        tester.config_dir().exists(),
        "hoard config directory should exist"
    );
    assert!(
        tester.data_dir().exists(),
        "hoard data directory should exist"
    );

    let config_file = tester.config_dir().join("config.toml");

    assert!(config_file.exists(), "config file should have been created");
}
