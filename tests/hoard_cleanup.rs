mod common;

use common::base::DefaultConfigTester;
use common::base::{HOARD_ANON_DIR, HOARD_ANON_FILE, HOARD_NAMED};
use common::UuidLocation;
use futures::{StreamExt, TryStreamExt};
use hoard::command::Command;
use once_cell::sync::Lazy;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio_stream::wrappers::ReadDirStream;

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
enum Direction {
    Backup,
    Restore,
}

const STRATEGY: [(UuidLocation, Direction, &str); 29] = [
    // AnonDir operations
    (UuidLocation::Local, Direction::Backup, HOARD_ANON_DIR),
    (UuidLocation::Remote, Direction::Restore, HOARD_ANON_DIR),
    (UuidLocation::Local, Direction::Backup, HOARD_ANON_DIR),
    (UuidLocation::Remote, Direction::Restore, HOARD_ANON_DIR),
    (UuidLocation::Remote, Direction::Backup, HOARD_ANON_DIR),
    (UuidLocation::Local, Direction::Restore, HOARD_ANON_DIR),
    (UuidLocation::Local, Direction::Backup, HOARD_ANON_DIR),
    (UuidLocation::Remote, Direction::Restore, HOARD_ANON_DIR),
    (UuidLocation::Remote, Direction::Backup, HOARD_ANON_DIR),
    (UuidLocation::Local, Direction::Restore, HOARD_ANON_DIR),
    (UuidLocation::Remote, Direction::Restore, HOARD_ANON_DIR),
    // AnonFile operations
    (UuidLocation::Local, Direction::Backup, HOARD_ANON_FILE),
    (UuidLocation::Local, Direction::Backup, HOARD_ANON_FILE),
    (UuidLocation::Local, Direction::Restore, HOARD_ANON_FILE),
    (UuidLocation::Remote, Direction::Restore, HOARD_ANON_FILE),
    (UuidLocation::Local, Direction::Backup, HOARD_ANON_FILE),
    (UuidLocation::Local, Direction::Restore, HOARD_ANON_FILE),
    (UuidLocation::Local, Direction::Backup, HOARD_ANON_FILE),
    (UuidLocation::Remote, Direction::Restore, HOARD_ANON_FILE),
    // Named operations
    (UuidLocation::Remote, Direction::Backup, HOARD_NAMED),
    (UuidLocation::Local, Direction::Restore, HOARD_NAMED),
    (UuidLocation::Remote, Direction::Backup, HOARD_NAMED),
    (UuidLocation::Local, Direction::Restore, HOARD_NAMED),
    (UuidLocation::Local, Direction::Backup, HOARD_NAMED),
    (UuidLocation::Local, Direction::Restore, HOARD_NAMED),
    (UuidLocation::Remote, Direction::Restore, HOARD_NAMED),
    (UuidLocation::Remote, Direction::Backup, HOARD_NAMED),
    (UuidLocation::Local, Direction::Restore, HOARD_NAMED),
    (UuidLocation::Remote, Direction::Backup, HOARD_NAMED),
];

static RETAINED: Lazy<HashMap<UuidLocation, HashMap<&'static str, Vec<usize>>>> = Lazy::new(|| {
    maplit::hashmap! {
        UuidLocation::Local => maplit::hashmap! {
            HOARD_ANON_FILE => vec![5],
            HOARD_ANON_DIR => vec![3, 4],
            HOARD_NAMED => vec![2, 4],
        },
        UuidLocation::Remote => maplit::hashmap! {
            HOARD_ANON_FILE => vec![1],
            HOARD_ANON_DIR => vec![4, 5],
            HOARD_NAMED => vec![4],
        }
    }
});

// In the end, these should be the final results:
// bkup = backups
// rstr = restore
// ALL CAPS indicates the most recent operation type
//
// +-------------------------------------------+
// |         | anon_dir | anon_file |  named   |
// +---------+----------+-----------+----------+
// | system1 | bkup x 3 | BKUP x 4  | bkup x 1 |
// |         | RSTR x 2 | rstr x 2  | RSTR x 4 |
// +---------+----------+-----------+----------+
// | system2 | bkup x 2 | bkup x 0  | BKUP x 4 |
// |         | RSTR x 4 | RSTR x 2  | rstr x 1 |
// +---------+----------+-----------+----------+

async fn run_operation(
    tester: &DefaultConfigTester,
    location: UuidLocation,
    direction: Direction,
    hoard: &str,
) {
    // Create new file contents.
    let file_path = if hoard == HOARD_NAMED {
        tester.home_dir().join("first_named_file")
    } else if hoard == HOARD_ANON_DIR {
        tester.home_dir().join("first_anon_dir").join("file.txt")
    } else if hoard == HOARD_ANON_FILE {
        tester.home_dir().join("first_anon_file")
    } else {
        panic!("unexpected hoard {}", hoard);
    };

    if let Some(parent) = file_path.parent() {
        fs::create_dir_all(parent)
            .await
            .expect("creating parent directories should succeed");
    }
    fs::write(file_path, uuid::Uuid::new_v4().as_bytes())
        .await
        .expect("failed to write new content to file");

    match location {
        UuidLocation::Local => tester.use_local_uuid().await,
        UuidLocation::Remote => tester.use_remote_uuid().await,
    }

    match direction {
        Direction::Backup => {
            tester
                .expect_command(Command::Backup {
                    hoards: vec![hoard.parse().unwrap()],
                })
                .await
        }
        Direction::Restore => {
            tester
                .expect_command(Command::Restore {
                    hoards: vec![hoard.parse().unwrap()],
                })
                .await
        }
    }
}

async fn files_in_dir(root: &Path) -> Vec<PathBuf> {
    let stream = fs::read_dir(&root)
        .await
        .map(ReadDirStream::new)
        .expect("failed to read from directory");
    stream
        .and_then(|entry| async move { Ok(entry.path()) })
        .try_collect()
        .await
        .unwrap()
}

#[tokio::test]
async fn test_operation_cleanup() {
    let mut tester = DefaultConfigTester::with_log_level(tracing::Level::TRACE).await;
    tester.use_first_env();
    tester.setup_files().await;
    for (location, direction, hoard) in &STRATEGY {
        run_operation(&tester, *location, *direction, hoard).await;
    }

    let data_dir = tester.data_dir();
    let local_uuid = tester.local_uuid().as_hyphenated().to_string();
    let remote_uuid = tester.remote_uuid().as_hyphenated().to_string();
    let expected: HashMap<UuidLocation, HashMap<&'static str, HashSet<PathBuf>>> =
        tokio_stream::iter(RETAINED.iter())
            .map(|(location, retained)| {
                let system_id = match location {
                    UuidLocation::Local => local_uuid.clone(),
                    UuidLocation::Remote => remote_uuid.clone(),
                };
                (*location, system_id, retained.clone())
            })
            .then(|(location, system_id, retained)| async move {
                let retained: HashMap<&'static str, HashSet<PathBuf>> =
                    tokio_stream::iter(retained.iter())
                        .map(|(hoard, indices)| (system_id.clone(), hoard, indices))
                        .then(|(system_id, hoard, indices)| async move {
                            let path = data_dir.join("history").join(system_id).join(hoard);
                            let mut files = files_in_dir(&path).await;
                            files.sort_unstable();

                            let files = files
                                .into_iter()
                                .enumerate()
                                .filter_map(|(i, path)| indices.contains(&i).then(|| path))
                                .collect();
                            (*hoard, files)
                        })
                        .collect()
                        .await;

                (location, retained)
            })
            .collect()
            .await;

    tester.expect_command(Command::Cleanup).await;

    for (location, retained) in RETAINED.iter() {
        for hoard in retained.keys() {
            let system_id = match location {
                UuidLocation::Local => tester.local_uuid().as_hyphenated().to_string(),
                UuidLocation::Remote => tester.remote_uuid().as_hyphenated().to_string(),
            };
            let path = tester
                .data_dir()
                .join("history")
                .join(system_id)
                .join(hoard);
            let files: HashSet<PathBuf> = files_in_dir(&path).await.into_iter().collect();
            let expected_files = expected
                .get(location)
                .expect("location should exist")
                .get(hoard)
                .expect("hoard should exist");
            assert_eq!(
                &files, expected_files,
                "expected {:?} got {:?}",
                expected_files, files
            );
        }
    }

    tester.use_local_uuid().await;
    common::create_file_with_random_data::<2048>(tester.named_file().system_path()).await;
    tester
        .run_command(Command::Backup {
            hoards: vec![HOARD_NAMED.parse().unwrap()],
        })
        .await
        .expect_err("backing up named hoard should fail");

    tester
        .expect_command(Command::Backup {
            hoards: vec![HOARD_ANON_DIR.parse().unwrap()],
        })
        .await;
    tester
        .expect_command(Command::Backup {
            hoards: vec![HOARD_ANON_FILE.parse().unwrap()],
        })
        .await;
}
