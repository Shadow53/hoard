mod common;

use common::tester::Tester;
use futures::TryStreamExt;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use time::Duration;
use tokio::fs;
use tokio_stream::wrappers::ReadDirStream;

use hoard::checkers::history::operation::util::TIME_FORMAT;
use hoard::checkers::history::operation::v1::{Hoard as HoardV1, OperationV1, Pile as PileV1};
use hoard::checkers::history::operation::v2::OperationV2;
use hoard::checkers::history::operation::OperationImpl;
use hoard::checksum::Checksum;
use hoard::command::Command;
use hoard::newtypes::HoardName;
use hoard::paths::RelativePath;

fn anon_file_operations() -> Vec<OperationV1> {
    let third_timestamp = time::OffsetDateTime::now_utc();
    let second_timestamp = third_timestamp - Duration::hours(2);
    let first_timestamp = second_timestamp - Duration::hours(2);
    let hoard_name: HoardName = "anon_file".parse().unwrap();
    vec![
        OperationV1 {
            timestamp: first_timestamp,
            is_backup: true,
            hoard_name: hoard_name.clone(),
            hoard: HoardV1::Anonymous(PileV1::from(
                maplit::hashmap! { RelativePath::none() => Checksum::MD5("d3369a026ace494f56ead54d502a00dd".parse().unwrap()) },
            )),
        },
        OperationV1 {
            timestamp: second_timestamp,
            is_backup: false,
            hoard_name: hoard_name.clone(),
            hoard: HoardV1::Anonymous(PileV1::from(
                maplit::hashmap! { RelativePath::none() => Checksum::MD5("d3369a026ace494f56ead54d502a00dd".parse().unwrap()) },
            )),
        },
        OperationV1 {
            timestamp: third_timestamp,
            is_backup: true,
            hoard_name,
            hoard: HoardV1::Anonymous(PileV1::from(HashMap::new())),
        },
    ]
}

fn anon_dir_operations() -> Vec<OperationV1> {
    let third_timestamp = time::OffsetDateTime::now_utc();
    let second_timestamp = third_timestamp - Duration::hours(2);
    let first_timestamp = second_timestamp - Duration::hours(2);
    let hoard_name: HoardName = "anon_dir".parse().unwrap();
    vec![
        OperationV1 {
            timestamp: first_timestamp,
            is_backup: true,
            hoard_name: hoard_name.clone(),
            hoard: HoardV1::Anonymous(PileV1::from(maplit::hashmap! {
                RelativePath::try_from(PathBuf::from("file_1")).unwrap() => Checksum::MD5("ba9d332813a722b273a95fa13dd88d94".parse().unwrap()),
                RelativePath::try_from(PathBuf::from("file_2")).unwrap() => Checksum::MD5("92ed3b5f07b44bc4f70d0b24d5e1867c".parse().unwrap()),
            })),
        },
        OperationV1 {
            timestamp: second_timestamp,
            is_backup: true,
            hoard_name: hoard_name.clone(),
            hoard: HoardV1::Anonymous(PileV1::from(maplit::hashmap! {
                RelativePath::try_from(PathBuf::from("file_1")).unwrap() => Checksum::MD5("1cfab2a192005a9a8bdc69106b4627e2".parse().unwrap()),
                RelativePath::try_from(PathBuf::from("file_2")).unwrap() => Checksum::MD5("92ed3b5f07b44bc4f70d0b24d5e1867c".parse().unwrap()),
                RelativePath::try_from(PathBuf::from("file_3")).unwrap() => Checksum::MD5("797b373a9c4ec0d6de0a31a90b5bee8e".parse().unwrap()),
            })),
        },
        OperationV1 {
            timestamp: third_timestamp,
            is_backup: true,
            hoard_name,
            hoard: HoardV1::Anonymous(PileV1::from(maplit::hashmap! {
                RelativePath::try_from(PathBuf::from("file_1")).unwrap() => Checksum::MD5("1cfab2a192005a9a8bdc69106b4627e2".parse().unwrap()),
                RelativePath::try_from(PathBuf::from("file_3")).unwrap() => Checksum::MD5("1deb21ef3bb87be4ad71d73fff6bb8ec".parse().unwrap()),
            })),
        },
    ]
}

fn named_operations() -> Vec<OperationV1> {
    let third_timestamp = time::OffsetDateTime::now_utc();
    let second_timestamp = third_timestamp - Duration::hours(2);
    let first_timestamp = second_timestamp - Duration::hours(2);
    let hoard_name: HoardName = "named".parse().unwrap();
    vec![
        OperationV1 {
            timestamp: first_timestamp,
            is_backup: true,
            hoard_name: hoard_name.clone(),
            hoard: HoardV1::Named(maplit::hashmap! {
                "single_file".parse().unwrap() => PileV1::from(maplit::hashmap! { RelativePath::none() => Checksum::MD5("d3369a026ace494f56ead54d502a00dd".parse().unwrap()) }),
                "dir".parse().unwrap() => PileV1::from(maplit::hashmap! {
                    RelativePath::try_from(PathBuf::from("file_1")).unwrap() => Checksum::MD5("ba9d332813a722b273a95fa13dd88d94".parse().unwrap()),
                    RelativePath::try_from(PathBuf::from("file_2")).unwrap() => Checksum::MD5("92ed3b5f07b44bc4f70d0b24d5e1867c".parse().unwrap()),
                })
            }),
        },
        OperationV1 {
            timestamp: second_timestamp,
            is_backup: true,
            hoard_name: hoard_name.clone(),
            hoard: HoardV1::Named(maplit::hashmap! {
                "single_file".parse().unwrap() => PileV1::from(maplit::hashmap! { RelativePath::none() => Checksum::MD5("d3369a026ace494f56ead54d502a00dd".parse().unwrap()) }),
                "dir".parse().unwrap() => PileV1::from(maplit::hashmap! {
                    RelativePath::try_from(PathBuf::from("file_1")).unwrap() => Checksum::MD5("1cfab2a192005a9a8bdc69106b4627e2".parse().unwrap()),
                    RelativePath::try_from(PathBuf::from("file_2")).unwrap() => Checksum::MD5("92ed3b5f07b44bc4f70d0b24d5e1867c".parse().unwrap()),
                    RelativePath::try_from(PathBuf::from("file_3")).unwrap() => Checksum::MD5("797b373a9c4ec0d6de0a31a90b5bee8e".parse().unwrap()),
                })
            }),
        },
        OperationV1 {
            timestamp: third_timestamp,
            is_backup: true,
            hoard_name,
            hoard: HoardV1::Named(maplit::hashmap! {
                "single_file".parse().unwrap() => PileV1::from(HashMap::new()),
                "dir".parse().unwrap() => PileV1::from(maplit::hashmap! {
                    RelativePath::try_from(PathBuf::from("file_1")).unwrap() => Checksum::MD5("1cfab2a192005a9a8bdc69106b4627e2".parse().unwrap()),
                    RelativePath::try_from(PathBuf::from("file_3")).unwrap() => Checksum::MD5("1deb21ef3bb87be4ad71d73fff6bb8ec".parse().unwrap()),
                })
            }),
        },
    ]
}

// Unit tests cover the actual conversion, so this assumes it works correctly.
fn convert_vec(v1: &[OperationV1]) -> Vec<OperationV2> {
    let mut mapping = HashMap::new();
    let mut file_set = HashSet::new();
    let mut new_ops = Vec::new();

    for op_v1 in v1 {
        let new_op = OperationV2::from_v1(&mut mapping, &mut file_set, op_v1.clone());
        new_ops.push(new_op);
    }

    new_ops
}

async fn write_to_files(tester: &Tester, ops: &[OperationV1]) {
    for op in ops {
        let path = tester
            .data_dir()
            .join("history")
            .join(
                tester
                    .get_uuid()
                    .await
                    .expect("getting uuid should succeed"),
            )
            .join(op.hoard_name().as_ref())
            .join(format!(
                "{}.log",
                op.timestamp()
                    .format(&TIME_FORMAT)
                    .expect("formatting timestamp should succeed")
            ));

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .await
                .expect("creating parent dirs should succeed");
        }

        let content = serde_json::to_vec(&op).expect("writing a V1 operation file should succeed");
        fs::write(path, &content)
            .await
            .expect("writing file content should succeed");
    }
}

async fn read_from_files(tester: &Tester, hoard: &str) -> Vec<OperationV2> {
    let path = tester
        .data_dir()
        .join("history")
        .join(
            tester
                .get_uuid()
                .await
                .expect("getting uuid should succeed"),
        )
        .join(hoard);
    let mut list: Vec<_> = fs::read_dir(&path)
        .await
        .map_or_else(
            |_| {
                panic!(
                    "reading history directory {} should not fail",
                    path.display()
                )
            },
            ReadDirStream::new,
        )
        .try_filter_map(|entry| async move {
            if entry.file_name() != "last_paths.json" {
                let content = fs::read(entry.path())
                    .await
                    .expect("reading from file should succeed");
                serde_json::from_slice::<OperationV2>(&content)
                    .map(Some)
                    .map(Ok)
                    .unwrap()
            } else {
                Ok(None)
            }
        })
        .try_collect()
        .await
        .unwrap();
    list.sort_unstable_by_key(|left| left.timestamp());
    list
}

#[tokio::test]
async fn test_hoard_upgrade() {
    let tester = Tester::new("").await;
    tester.use_local_uuid().await;

    let v1_anon_file = anon_file_operations();
    let v1_anon_dir = anon_dir_operations();
    let v1_named = named_operations();

    let v2_anon_file = convert_vec(&v1_anon_file);
    let v2_anon_dir = convert_vec(&v1_anon_dir);
    let v2_named = convert_vec(&v1_named);

    write_to_files(&tester, &v1_anon_file).await;
    write_to_files(&tester, &v1_anon_dir).await;
    write_to_files(&tester, &v1_named).await;

    tester.expect_command(Command::Upgrade).await;
    println!("{}", tester.extra_logging_output().await);

    let converted_anon_file = read_from_files(&tester, "anon_file").await;
    let converted_anon_dir = read_from_files(&tester, "anon_dir").await;
    let converted_named = read_from_files(&tester, "named").await;

    println!("{:#?}\n{:#?}", v2_anon_file, converted_anon_file);

    assert_eq!(v2_anon_file, converted_anon_file);
    assert_eq!(v2_anon_dir, converted_anon_dir);
    assert_eq!(v2_named, converted_named);
}
