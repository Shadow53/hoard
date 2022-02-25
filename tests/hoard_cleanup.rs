mod common;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use common::test_helper::Tester;
use common::{UuidLocation, HOARD_ANON_DIR, HOARD_ANON_FILE, HOARD_NAMED};
use hoard::command::Command;
use once_cell::unsync::Lazy;

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
enum Direction {
    Backup,
    Restore,
}

const STRATEGY: [(UuidLocation, Direction, &'static str); 29] = [
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

const RETAINED: Lazy<HashMap<UuidLocation, HashMap<&'static str, Vec<usize>>>> = Lazy::new(|| {
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

fn run_operation(tester: &Tester, location: UuidLocation, direction: Direction, hoard: &str) {
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
        fs::create_dir_all(parent).expect("creating parent directories should succeed");
    }
    fs::write(file_path, uuid::Uuid::new_v4().as_bytes())
        .expect("failed to write new content to file");

    match location {
        UuidLocation::Local => tester.use_local_uuid(),
        UuidLocation::Remote => tester.use_remote_uuid(),
    }

    match direction {
        Direction::Backup => tester.expect_command(Command::Backup { hoards: vec![hoard.to_string()] }),
        Direction::Restore => tester.expect_command(Command::Restore { hoards: vec![hoard.to_string()] }),
    }
}

fn files_in_dir(root: &Path) -> Vec<PathBuf> {
    fs::read_dir(&root)
        .expect("failed to read from directory")
        .into_iter()
        .map(|entry| entry.expect("failed to read directory entry").path())
        .collect()
}

#[test]
#[serial_test::serial]
fn test_operation_cleanup() {
    let tester = Tester::new(common::BASE_CONFIG);
    for (location, direction, hoard) in &STRATEGY {
        run_operation(&tester, *location, *direction, hoard);
    }

    let expected: HashMap<UuidLocation, HashMap<&'static str, HashSet<PathBuf>>> = RETAINED.iter()
        .map(|(location, retained)| {
            let retained = retained.iter().map(|(hoard, indices)| {
                let system_id = match location {
                    UuidLocation::Local => tester.local_uuid().to_hyphenated().to_string(),
                    UuidLocation::Remote => tester.remote_uuid().to_hyphenated().to_string(),
                };
                let path = tester.data_dir().join("history").join(system_id).join(hoard);
                let mut files = files_in_dir(&path);
                files.sort_unstable();

                let files = files.into_iter()
                    .filter(|path| !path.to_string_lossy().contains("last_paths"))
                    .enumerate()
                    .filter_map(|(i, path)| indices.contains(&i).then(|| path))
                    .collect();
                (*hoard, files)
            }).collect();

            (*location, retained)
        }).collect();

    tester.expect_command(Command::Cleanup);

    for (location, retained) in RETAINED.iter() {
        for (hoard, _) in retained {
            let system_id = match location {
                UuidLocation::Local => tester.local_uuid().to_hyphenated().to_string(),
                UuidLocation::Remote => tester.remote_uuid().to_hyphenated().to_string(),
            };
            let path = tester.data_dir().join("history").join(system_id).join(hoard);
            let files: HashSet<PathBuf> = files_in_dir(&path).into_iter().collect();
            let expected_files = expected
                .get(location)
                .expect("location should exist")
                .get(hoard)
                .expect("hoard should exist");
            assert_eq!(&files, expected_files, "expected {:?} got {:?}", expected_files, files);
        }
    }

    tester.use_local_uuid();
    tester.run_command(Command::Backup { hoards: vec![HOARD_NAMED.to_string()] })
        .expect_err("backing up named hoard should fail");

    tester.expect_command(Command::Backup { hoards: vec![HOARD_ANON_DIR.to_string()] });
    tester.expect_command(Command::Backup { hoards: vec![HOARD_ANON_FILE.to_string()] });
}
