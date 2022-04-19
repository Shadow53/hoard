mod common;

use common::base::DefaultConfigTester;
use hoard::command::Command;
use tokio::fs;
use std::path::PathBuf;

const GLOBAL_FILE: &str = "global_ignore";
const HOARD_FILE: &str = "ignore_for_hoard";
const PILE_FILE: &str = "spilem";
const NESTED_FILE: &str = "nested_dir/.hidden";

fn ignored_files(tester: &DefaultConfigTester) -> Vec<PathBuf> {
    vec![
        tester.home_dir().join("first_anon_dir").join(GLOBAL_FILE),
        tester.home_dir().join("first_named_dir1").join(GLOBAL_FILE),
        tester.home_dir().join("first_named_dir2").join(GLOBAL_FILE),
        tester.home_dir().join("first_named_dir1").join(HOARD_FILE),
        tester.home_dir().join("first_named_dir2").join(HOARD_FILE),
        tester.home_dir().join("first_named_dir1").join(PILE_FILE),
        tester.home_dir().join("first_named_dir2").join(NESTED_FILE),
    ]
}

fn all_extra_files(tester: &DefaultConfigTester) -> Vec<PathBuf> {
    ["first_anon_dir", "first_named_dir1", "first_named_dir2"]
        .into_iter()
        .flat_map(|slug| {
            vec![
                tester.home_dir().join(slug).join("global_ignore"),
                tester.home_dir().join(slug).join("ignore_for_hoard"),
                tester.home_dir().join(slug).join("spilem"),
                tester
                    .home_dir()
                    .join(slug)
                    .join("nested_dir")
                    .join(".hidden"),
            ]
        })
        .collect()
}

#[tokio::test]
async fn test_ignore_filter() {
    let mut tester = DefaultConfigTester::new().await;
    tester.setup_files().await;
    tester.use_first_env();

    for home in all_extra_files(&tester) {
        common::create_file_with_random_data::<2048>(&home).await;
    }

    tester.expect_command(Command::Backup { hoards: Vec::new() }).await;

    // Delete ignored files from home so assertion works
    for home in ignored_files(&tester) {
        fs::remove_file(&home).await.expect("failed to remove ignored file");
    }

    tester.assert_first_tree().await;
}
