use hoard::hoard::{HoardPath, SystemPath};
use hoard::hoard_item::HoardItem;
use nix::libc::write;
use rand::RngCore;
use sha2::digest::generic_array::GenericArray;
use sha2::digest::OutputSizeUser;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};

use super::tester::Tester;

pub const HOARD_ANON_DIR: &str = "anon_dir";
pub const HOARD_ANON_FILE: &str = "anon_file";
pub const HOARD_NAMED: &str = "named";
pub const HOARD_NAMED_FILE: &str = "file";
pub const HOARD_NAMED_DIR1: &str = "dir1";
pub const HOARD_NAMED_DIR2: &str = "dir2";
pub const DIR_FILE_1: &str = "1";
pub const DIR_FILE_2: &str = "2";
pub const DIR_FILE_3: &str = "3";

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

pub struct DefaultConfigTester {
    tester: Tester,
    is_first_env: Option<bool>,
}

impl AsRef<Tester> for DefaultConfigTester {
    fn as_ref(&self) -> &Tester {
        &self.tester
    }
}

impl Deref for DefaultConfigTester {
    type Target = Tester;
    fn deref(&self) -> &Self::Target {
        &self.tester
    }
}

impl DerefMut for DefaultConfigTester {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tester
    }
}

impl DefaultConfigTester {
    pub fn new() -> Self {
        Self::with_log_level(tracing::Level::INFO)
    }

    pub fn with_log_level(log_level: tracing::Level) -> Self {
        Self {
            tester: Tester::with_log_level(BASE_CONFIG, log_level),
            is_first_env: None,
        }
    }

    pub fn use_first_env(&mut self) {
        std::env::set_var("USE_ENV", "1");
        self.is_first_env = Some(true);
        self.reset_config(BASE_CONFIG);
    }

    pub fn use_second_env(&mut self) {
        std::env::set_var("USE_ENV", "2");
        self.is_first_env = Some(false);
        self.reset_config(BASE_CONFIG);
    }

    pub fn unset_env(&mut self) {
        std::env::remove_var("USE_ENV");
        self.is_first_env = None;
        self.reset_config(BASE_CONFIG);
    }

    fn is_first_env(&self) -> bool {
        self.is_first_env.expect("USE_ENV must be set")
    }

    fn file_prefix(&self) -> &'static str {
        if self.is_first_env() {
            "first_"
        } else {
            "second_"
        }
    }

    pub fn anon_file(&self) -> HoardItem {
        let system_path = SystemPath::from(self.home_dir().join(format!(
            "{}{}",
            self.file_prefix(),
            HOARD_ANON_FILE
        )));
        let hoard_path = HoardPath::from(self.data_dir().join("hoards").join(HOARD_ANON_FILE));
        HoardItem::new(None, hoard_path, system_path, PathBuf::new())
    }

    pub fn anon_dir(&self) -> HoardItem {
        let system_path = SystemPath::from(self.home_dir().join(format!(
            "{}{}",
            self.file_prefix(),
            HOARD_ANON_DIR
        )));
        let hoard_path = HoardPath::from(self.data_dir().join("hoards").join(HOARD_ANON_DIR));
        HoardItem::new(None, hoard_path, system_path, PathBuf::new())
    }

    pub fn named_file(&self) -> HoardItem {
        let system_path = SystemPath::from(self.home_dir().join(format!(
            "{}named_{}",
            self.file_prefix(),
            HOARD_NAMED_FILE
        )));
        let hoard_path = HoardPath::from(
            self.data_dir()
                .join("hoards")
                .join(HOARD_NAMED)
                .join(HOARD_NAMED_FILE),
        );
        HoardItem::new(
            Some(String::from(HOARD_NAMED_FILE)),
            hoard_path,
            system_path,
            PathBuf::new(),
        )
    }

    pub fn named_dir1(&self) -> HoardItem {
        let system_path = SystemPath::from(self.home_dir().join(format!(
            "{}named_{}",
            self.file_prefix(),
            HOARD_NAMED_DIR1
        )));
        let hoard_path = HoardPath::from(
            self.data_dir()
                .join("hoards")
                .join(HOARD_NAMED)
                .join(HOARD_NAMED_DIR1),
        );
        HoardItem::new(
            Some(String::from(HOARD_NAMED_DIR1)),
            hoard_path,
            system_path,
            PathBuf::new(),
        )
    }

    pub fn named_dir2(&self) -> HoardItem {
        let system_path = SystemPath::from(self.home_dir().join(format!(
            "{}named_{}",
            self.file_prefix(),
            HOARD_NAMED_DIR2
        )));
        let hoard_path = HoardPath::from(
            self.data_dir()
                .join("hoards")
                .join(HOARD_NAMED)
                .join(HOARD_NAMED_DIR2),
        );
        HoardItem::new(
            Some(String::from(HOARD_NAMED_DIR2)),
            hoard_path,
            system_path,
            PathBuf::new(),
        )
    }

    fn file_in_hoard_dir(dir: &HoardItem, file: PathBuf) -> HoardItem {
        HoardItem::new(
            dir.pile_name().map(ToString::to_string),
            HoardPath::from(dir.hoard_prefix().to_path_buf()),
            SystemPath::from(dir.system_prefix().to_path_buf()),
            file,
        )
    }

    fn env_files_inner(&self) -> Vec<HoardItem> {
        let anon_dir = self.anon_dir();
        let named_dir1 = self.named_dir1();
        let named_dir2 = self.named_dir2();
        vec![
            self.anon_file(),
            Self::file_in_hoard_dir(&anon_dir, PathBuf::from(DIR_FILE_1)),
            Self::file_in_hoard_dir(&anon_dir, PathBuf::from(DIR_FILE_2)),
            Self::file_in_hoard_dir(&anon_dir, PathBuf::from(DIR_FILE_3)),
            self.named_file(),
            Self::file_in_hoard_dir(&named_dir1, PathBuf::from(DIR_FILE_1)),
            Self::file_in_hoard_dir(&named_dir1, PathBuf::from(DIR_FILE_2)),
            Self::file_in_hoard_dir(&named_dir1, PathBuf::from(DIR_FILE_3)),
            Self::file_in_hoard_dir(&named_dir2, PathBuf::from(DIR_FILE_1)),
            Self::file_in_hoard_dir(&named_dir2, PathBuf::from(DIR_FILE_2)),
            Self::file_in_hoard_dir(&named_dir2, PathBuf::from(DIR_FILE_3)),
        ]
    }

    pub fn first_env_files(&mut self) -> Vec<HoardItem> {
        let old_env = self.is_first_env;
        self.is_first_env = Some(true);
        let result = self.env_files_inner();
        self.is_first_env = old_env;
        result
    }

    pub fn second_env_files(&mut self) -> Vec<HoardItem> {
        let old_env = self.is_first_env;
        self.is_first_env = Some(false);
        let result = self.env_files_inner();
        self.is_first_env = old_env;
        result
    }

    pub fn setup_files(&mut self) {
        // First Env
        let first_paths = self.first_env_files();

        // Second Env
        let second_paths = self.second_env_files();

        let paths = first_paths
            .into_iter()
            .chain(second_paths)
            .map(|file| file.system_path().to_path_buf());

        for file in paths {
            if let Some(parent) = file.parent() {
                fs::create_dir_all(parent).expect("creating parent dirs should not fail");
            }

            super::create_file_with_random_data::<2048>(&file);
        }
    }

    fn hash_file(file: &Path) -> GenericArray<u8, <Sha256 as OutputSizeUser>::OutputSize> {
        let data = fs::read(file).expect("file should always exist");
        Sha256::digest(&data)
    }

    fn file_contents(
        path: &Path,
        root: &Path,
    ) -> HashMap<PathBuf, GenericArray<u8, <Sha256 as OutputSizeUser>::OutputSize>> {
        if path.is_file() {
            let key = path
                .strip_prefix(root)
                .expect("path should always have root as prefix")
                .to_path_buf();
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
        assert_eq!(
            left_content,
            right_content,
            "{} and {} do not have matching contents",
            left.display(),
            right.display()
        );
    }

    pub fn assert_first_tree(&self) {
        let hoards_root = self.data_dir().join("hoards");
        Self::assert_same_tree(
            &self.home_dir().join("first_anon_file"),
            &hoards_root.join("anon_file"),
        );
        Self::assert_same_tree(
            &self.home_dir().join("first_anon_dir"),
            &hoards_root.join("anon_dir"),
        );
        Self::assert_same_tree(
            &self.home_dir().join("first_named_file"),
            &hoards_root.join("named").join("file"),
        );
        Self::assert_same_tree(
            &self.home_dir().join("first_named_dir1"),
            &hoards_root.join("named").join("dir1"),
        );
        Self::assert_same_tree(
            &self.home_dir().join("first_named_dir2"),
            &hoards_root.join("named").join("dir2"),
        );
    }

    pub fn assert_second_tree(&self) {
        let hoards_root = self.data_dir().join("hoards");
        Self::assert_same_tree(
            &self.home_dir().join("second_anon_file"),
            &hoards_root.join("anon_file"),
        );
        Self::assert_same_tree(
            &self.home_dir().join("second_anon_dir"),
            &hoards_root.join("anon_dir"),
        );
        Self::assert_same_tree(
            &self.home_dir().join("second_named_file"),
            &hoards_root.join("named").join("file"),
        );
        Self::assert_same_tree(
            &self.home_dir().join("second_named_dir1"),
            &hoards_root.join("named").join("dir1"),
        );
        Self::assert_same_tree(
            &self.home_dir().join("second_named_dir2"),
            &hoards_root.join("named").join("dir2"),
        );
    }
}
