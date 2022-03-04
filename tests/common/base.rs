use std::fs;
use rand::RngCore;

use super::tester::Tester;

pub fn use_first_env() {
    std::env::set_var("USE_ENV", "1");
}

pub fn use_second_env() {
    std::env::set_var("USE_ENV", "2");
}

pub fn setup_files(tester: &Tester) {
    // First Env
    let first_paths = [
        tester.home_dir().join("first_anon_file"),
        tester.home_dir().join("first_named_file"),
        tester.home_dir().join("first_anon_dir").join("1"),
        tester.home_dir().join("first_anon_dir").join("2"),
        tester.home_dir().join("first_anon_dir").join("3"),
        tester.home_dir().join("first_named_dir1").join("1"),
        tester.home_dir().join("first_named_dir1").join("2"),
        tester.home_dir().join("first_named_dir1").join("3"),
        tester.home_dir().join("first_named_dir2").join("1"),
        tester.home_dir().join("first_named_dir2").join("2"),
        tester.home_dir().join("first_named_dir2").join("3"),
    ];

    // Second Env
    let second_paths = [
        tester.home_dir().join("second_anon_file"),
        tester.home_dir().join("second_named_file"),
        tester.home_dir().join("second_anon_dir").join("1"),
        tester.home_dir().join("second_anon_dir").join("2"),
        tester.home_dir().join("second_anon_dir").join("3"),
        tester.home_dir().join("second_named_dir1").join("1"),
        tester.home_dir().join("second_named_dir1").join("2"),
        tester.home_dir().join("second_named_dir1").join("3"),
        tester.home_dir().join("second_named_dir2").join("1"),
        tester.home_dir().join("second_named_dir2").join("2"),
        tester.home_dir().join("second_named_dir2").join("3"),
    ];

    for file in first_paths.into_iter().chain(second_paths) {
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent).expect("creating parent dirs should not fail");
        }
        
        let mut content = [0; 2048];
        rand::thread_rng().fill_bytes(&mut content);
        fs::write(&file, &content).expect("writing data to file should not fail");
    }
}

pub const HOARD_ANON_DIR: &str = "anon_dir";
pub const HOARD_ANON_FILE: &str = "anon_file";
pub const HOARD_NAMED: &str = "named";

pub const BASE_CONFIG: &str = r#"
# Using weird table-array syntax to make converting from TOML->YAML for tests easier.
# Using inline {} tables uses a custom TOML type that does not translate correctly
# NOTE: this is only for testing. Inline tables work fine with Hoard

exclusivity = [
    ["first", "second"],
    ["unix", "windows"]
]

[envs]
[envs.first]
[[envs.first.env]]
    var = "USE_ENV"
    expected = "1"
[envs.second]
[[envs.second.env]]
    var = "USE_ENV"
    expected = "2"
[envs.windows]
    os = ["windows"]
[[envs.windows.env]]
    var = "USERPROFILE"
[envs.unix]
    os = ["linux", "macos"]
[[envs.unix.env]]
    var = "HOME"

[config]
    ignore = ["global*"]

[hoards]
[hoards.anon_dir]
    "unix|first"  = "${HOME}/first_anon_dir"
    "unix|second" = "${HOME}/second_anon_dir"
    "windows|first"  = "${USERPROFILE}/first_anon_dir"
    "windows|second" = "${USERPROFILE}/second_anon_dir"
[hoards.anon_file]
    "unix|first"  = "${HOME}/first_anon_file"
    "unix|second" = "${HOME}/second_anon_file"
    "windows|first"  = "${USERPROFILE}/first_anon_file"
    "windows|second" = "${USERPROFILE}/second_anon_file"
[hoards.named]
    [hoards.named.config]
        ignore = ["*hoard*"]
    [hoards.named.file]
        "unix|first"  = "${HOME}/first_named_file"
        "unix|second" = "${HOME}/second_named_file"
        "windows|first"  = "${USERPROFILE}/first_named_file"
        "windows|second" = "${USERPROFILE}/second_named_file"
    [hoards.named.dir1]
        "unix|first"  = "${HOME}/first_named_dir1"
        "unix|second" = "${HOME}/second_named_dir1"
        "windows|first"  = "${USERPROFILE}/first_named_dir1"
        "windows|second" = "${USERPROFILE}/second_named_dir1"
    [hoards.named.dir1.config]
        ignore = ["*pile*", ".hidden"]
    [hoards.named.dir2]
        "unix|first"  = "${HOME}/first_named_dir2"
        "unix|second" = "${HOME}/second_named_dir2"
        "windows|first"  = "${USERPROFILE}/first_named_dir2"
        "windows|second" = "${USERPROFILE}/second_named_dir2"
"#;
