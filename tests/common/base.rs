use std::collections::HashMap;
use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use rand::RngCore;
use sha2::digest::generic_array::GenericArray;
use sha2::{Digest, Sha256};
use sha2::digest::OutputSizeUser;

use super::tester::Tester;

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
    [hoards.named.dir2.config]
        ignore = ["**/.hidden"]
"#;

pub struct DefaultConfigTester(Tester);

impl AsRef<Tester> for DefaultConfigTester {
    fn as_ref(&self) -> &Tester {
        &self.0
    }
}

impl Deref for DefaultConfigTester {
    type Target = Tester;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for DefaultConfigTester {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl DefaultConfigTester {
    pub fn new() -> Self {
        Self(Tester::new(BASE_CONFIG))
    }

    pub fn with_log_level(log_level: tracing::Level) -> Self {
        Self(Tester::with_log_level(BASE_CONFIG, log_level))
    }

    pub fn use_first_env(&mut self) {
        std::env::set_var("USE_ENV", "1");
        self.reset_config(BASE_CONFIG);
    }

    pub fn use_second_env(&mut self) {
        std::env::set_var("USE_ENV", "2");
        self.reset_config(BASE_CONFIG);
    }

    pub fn unset_env(&mut self) {
        std::env::remove_var("USE_ENV");
        self.reset_config(BASE_CONFIG);
    }

    pub fn setup_files(&self) {
        // First Env
        let first_paths = [
            self.home_dir().join("first_anon_file"),
            self.home_dir().join("first_named_file"),
            self.home_dir().join("first_anon_dir").join("1"),
            self.home_dir().join("first_anon_dir").join("2"),
            self.home_dir().join("first_anon_dir").join("3"),
            self.home_dir().join("first_named_dir1").join("1"),
            self.home_dir().join("first_named_dir1").join("2"),
            self.home_dir().join("first_named_dir1").join("3"),
            self.home_dir().join("first_named_dir2").join("1"),
            self.home_dir().join("first_named_dir2").join("2"),
            self.home_dir().join("first_named_dir2").join("3"),
        ];

        // Second Env
        let second_paths = [
            self.home_dir().join("second_anon_file"),
            self.home_dir().join("second_named_file"),
            self.home_dir().join("second_anon_dir").join("1"),
            self.home_dir().join("second_anon_dir").join("2"),
            self.home_dir().join("second_anon_dir").join("3"),
            self.home_dir().join("second_named_dir1").join("1"),
            self.home_dir().join("second_named_dir1").join("2"),
            self.home_dir().join("second_named_dir1").join("3"),
            self.home_dir().join("second_named_dir2").join("1"),
            self.home_dir().join("second_named_dir2").join("2"),
            self.home_dir().join("second_named_dir2").join("3"),
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

    fn hash_file(file: &Path) -> GenericArray<u8, <Sha256 as OutputSizeUser>::OutputSize> {
        let data = fs::read(file).expect("file should always exist");
        Sha256::digest(&data)
    }

    fn file_contents(path: &Path, root: &Path) -> HashMap<PathBuf, GenericArray<u8, <Sha256 as OutputSizeUser>::OutputSize>> {
        if path.is_file() {
            let key = path.strip_prefix(root).expect("path should always have root as prefix").to_path_buf();
            maplit::hashmap! { key => Self::hash_file(path) }
        } else if path.is_dir() {
            let mut map = HashMap::new();
            for entry in fs::read_dir(path).expect("reading dir should not fail") {
                let entry = entry.expect("reading entry should not fail");
                let nested = Self::file_contents(&entry.path(), root);
                map.extend(nested);
            }
            map
        } else {
            panic!("{} does not exist", path.display());
        }
    }

    fn assert_same_tree(left: &Path, right: &Path) {
        let left_content = Self::file_contents(left, left);
        let right_content = Self::file_contents(right, right);
        assert_eq!(left_content, right_content, "{} and {} do not have matching contents", left.display(), right.display());
    }

    pub fn assert_first_tree(&self) {
        let hoards_root = self.data_dir().join("hoards");
        Self::assert_same_tree(
            &self.home_dir().join("first_anon_file"),
            &hoards_root.join("anon_file")
        );
        Self::assert_same_tree(
            &self.home_dir().join("first_anon_dir"),
            &hoards_root.join("anon_dir")
        );
        Self::assert_same_tree(
            &self.home_dir().join("first_named_file"),
            &hoards_root.join("named").join("file")
        );
        Self::assert_same_tree(
            &self.home_dir().join("first_named_dir1"),
            &hoards_root.join("named").join("dir1")
        );
        Self::assert_same_tree(
            &self.home_dir().join("first_named_dir2"),
            &hoards_root.join("named").join("dir2")
        );
    }

    pub fn assert_second_tree(&self) {
        let hoards_root = self.data_dir().join("hoards");
        Self::assert_same_tree(
            &self.home_dir().join("second_anon_file"),
            &hoards_root.join("anon_file")
        );
        Self::assert_same_tree(
            &self.home_dir().join("second_anon_dir"),
            &hoards_root.join("anon_dir")
        );
        Self::assert_same_tree(
            &self.home_dir().join("second_named_file"),
            &hoards_root.join("named").join("file")
        );
        Self::assert_same_tree(
            &self.home_dir().join("second_named_dir1"),
            &hoards_root.join("named").join("dir1")
        );
        Self::assert_same_tree(
            &self.home_dir().join("second_named_dir2"),
            &hoards_root.join("named").join("dir2")
        );
    }
}
