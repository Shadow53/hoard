mod common;

use common::tester::Tester;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use time::Duration;

use hoard::checkers::history::operation::util::TIME_FORMAT;
use hoard::checkers::history::operation::v1::{Hoard as HoardV1, OperationV1, Pile as PileV1};
use hoard::checkers::history::operation::v2::OperationV2;
use hoard::checkers::history::operation::{OperationImpl};
use hoard::checkers::Checker;
use hoard::command::Command;

fn anon_file_operations() -> Vec<OperationV1> {
    let third_timestamp = time::OffsetDateTime::now_utc();
    let second_timestamp = third_timestamp - Duration::hours(2);
    let first_timestamp = second_timestamp - Duration::hours(2);
    let hoard_name = String::from("anon_file");
    vec![
        OperationV1 {
            timestamp: first_timestamp,
            is_backup: true,
            hoard_name: hoard_name.clone(),
            hoard: HoardV1::Anonymous(PileV1::from(
                maplit::hashmap! { PathBuf::new() => String::from("d3369a026ace494f56ead54d502a00dd") },
            )),
        },
        OperationV1 {
            timestamp: second_timestamp,
            is_backup: false,
            hoard_name: hoard_name.clone(),
            hoard: HoardV1::Anonymous(PileV1::from(
                maplit::hashmap! { PathBuf::new() => String::from("d3369a026ace494f56ead54d502a00dd") },
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
    let hoard_name = String::from("anon_dir");
    vec![
        OperationV1 {
            timestamp: first_timestamp,
            is_backup: true,
            hoard_name: hoard_name.clone(),
            hoard: HoardV1::Anonymous(PileV1::from(maplit::hashmap! {
                PathBuf::from("file_1") => String::from("ba9d332813a722b273a95fa13dd88d94"),
                PathBuf::from("file_2") => String::from("92ed3b5f07b44bc4f70d0b24d5e1867c"),
            })),
        },
        OperationV1 {
            timestamp: second_timestamp,
            is_backup: true,
            hoard_name: hoard_name.clone(),
            hoard: HoardV1::Anonymous(PileV1::from(maplit::hashmap! {
                PathBuf::from("file_1") => String::from("1cfab2a192005a9a8bdc69106b4627e2"),
                PathBuf::from("file_2") => String::from("92ed3b5f07b44bc4f70d0b24d5e1867c"),
                PathBuf::from("file_3") => String::from("797b373a9c4ec0d6de0a31a90b5bee8e"),
            })),
        },
        OperationV1 {
            timestamp: third_timestamp,
            is_backup: true,
            hoard_name: hoard_name,
            hoard: HoardV1::Anonymous(PileV1::from(maplit::hashmap! {
                PathBuf::from("file_1") => String::from("1cfab2a192005a9a8bdc69106b4627e2"),
                PathBuf::from("file_3") => String::from("1deb21ef3bb87be4ad71d73fff6bb8ec"),
            })),
        },
    ]
}

fn named_operations() -> Vec<OperationV1> {
    let third_timestamp = time::OffsetDateTime::now_utc();
    let second_timestamp = third_timestamp - Duration::hours(2);
    let first_timestamp = second_timestamp - Duration::hours(2);
    let hoard_name = String::from("named");
    vec![
        OperationV1 {
            timestamp: first_timestamp,
            is_backup: true,
            hoard_name: hoard_name.clone(),
            hoard: HoardV1::Named(maplit::hashmap! {
                String::from("single_file") => PileV1::from(maplit::hashmap! { PathBuf::new() => String::from("d3369a026ace494f56ead54d502a00dd") }),
                String::from("dir") => PileV1::from(maplit::hashmap! {
                    PathBuf::from("file_1") => String::from("ba9d332813a722b273a95fa13dd88d94"),
                    PathBuf::from("file_2") => String::from("92ed3b5f07b44bc4f70d0b24d5e1867c"),
                })
            }),
        },
        OperationV1 {
            timestamp: second_timestamp,
            is_backup: true,
            hoard_name: hoard_name.clone(),
            hoard: HoardV1::Named(maplit::hashmap! {
                String::from("single_file") => PileV1::from(maplit::hashmap! { PathBuf::new() => String::from("d3369a026ace494f56ead54d502a00dd") }),
                String::from("dir") => PileV1::from(maplit::hashmap! {
                    PathBuf::from("file_1") => String::from("1cfab2a192005a9a8bdc69106b4627e2"),
                    PathBuf::from("file_2") => String::from("92ed3b5f07b44bc4f70d0b24d5e1867c"),
                    PathBuf::from("file_3") => String::from("797b373a9c4ec0d6de0a31a90b5bee8e"),
                })
            }),
        },
        OperationV1 {
            timestamp: third_timestamp,
            is_backup: true,
            hoard_name: hoard_name,
            hoard: HoardV1::Named(maplit::hashmap! {
                String::from("single_file") => PileV1::from(HashMap::new()),
                String::from("dir") => PileV1::from(maplit::hashmap! {
                    PathBuf::from("file_1") => String::from("1cfab2a192005a9a8bdc69106b4627e2"),
                    PathBuf::from("file_3") => String::from("1deb21ef3bb87be4ad71d73fff6bb8ec"),
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

fn write_to_files(tester: &Tester, ops: &[OperationV1]) {
    for op in ops {
        let path = tester
            .data_dir()
            .join("history")
            .join(tester.get_uuid().expect("getting uuid should succeed"))
            .join(&op.hoard_name)
            .join(format!(
                "{}.log",
                op.timestamp()
                    .format(&TIME_FORMAT)
                    .expect("formatting timestamp should succeed")
            ));

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("creating parent dirs should succeed");
        }

        let file = fs::File::create(path).expect("creating an operation file should not fail");

        serde_json::to_writer(file, &op).expect("writing a V1 operation file should succeed");
    }
}

fn read_from_files(tester: &Tester, hoard: &str) -> Vec<OperationV2> {
    let path = tester
        .data_dir()
        .join("history")
        .join(tester.get_uuid().expect("getting uuid should succeed"))
        .join(hoard);
    let mut list: Vec<_> = fs::read_dir(&path)
        .unwrap_or_else(|_| panic!("reading history directory {} should not fail",
            path.display()))
        .filter_map(|entry| {
            let entry = entry.expect("reading dir entry should not fail");
            (entry.file_name() != "last_paths.json").then(|| {
                let file = fs::File::open(entry.path()).expect("opening file should succeed");
                serde_json::from_reader::<_, OperationV2>(file)
                    .expect("parsing json should not fail")
            })
        })
        .collect();
    list.sort_unstable_by_key(|left| left.timestamp());
    list
}

#[test]
#[serial_test::serial]
fn test_hoard_upgrade() {
    let tester = Tester::new("");
    tester.use_local_uuid();

    let v1_anon_file = anon_file_operations();
    let v1_anon_dir = anon_dir_operations();
    let v1_named = named_operations();

    let v2_anon_file = convert_vec(&v1_anon_file);
    let v2_anon_dir = convert_vec(&v1_anon_dir);
    let v2_named = convert_vec(&v1_named);

    write_to_files(&tester, &v1_anon_file);
    write_to_files(&tester, &v1_anon_dir);
    write_to_files(&tester, &v1_named);

    tester.expect_command(Command::Upgrade);
    println!("{}", tester.extra_logging_output());

    let converted_anon_file = read_from_files(&tester, "anon_file");
    let converted_anon_dir = read_from_files(&tester, "anon_dir");
    let converted_named = read_from_files(&tester, "named");

    println!("{:#?}\n{:#?}", v2_anon_file, converted_anon_file);

    assert_eq!(v2_anon_file, converted_anon_file);
    assert_eq!(v2_anon_dir, converted_anon_dir);
    assert_eq!(v2_named, converted_named);
}
