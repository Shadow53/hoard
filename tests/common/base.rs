use futures::TryStreamExt;
use hoard::hoard_item::HoardItem;
use hoard::newtypes::PileName;
use hoard::paths::{HoardPath, RelativePath, SystemPath};
use rand::RngCore;
use sha2::digest::generic_array::GenericArray;
use sha2::digest::OutputSizeUser;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio_stream::wrappers::ReadDirStream;

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
    var = "HOARD_TMP"
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
    "windows|first"  = "${HOARD_TMP}/first_anon_dir"
    "windows|second" = "${HOARD_TMP}/second_anon_dir"
[hoards.anon_file]
    "unix|first"  = "${HOME}/first_anon_file"
    "unix|second" = "${HOME}/second_anon_file"
    "windows|first"  = "${HOARD_TMP}/first_anon_file"
    "windows|second" = "${HOARD_TMP}/second_anon_file"
[hoards.named]
    [hoards.named.config]
        ignore = ["*hoard*"]
    [hoards.named.file]
        "unix|first"  = "${HOME}/first_named_file"
        "unix|second" = "${HOME}/second_named_file"
        "windows|first"  = "${HOARD_TMP}/first_named_file"
        "windows|second" = "${HOARD_TMP}/second_named_file"
    [hoards.named.dir1]
        "unix|first"  = "${HOME}/first_named_dir1"
        "unix|second" = "${HOME}/second_named_dir1"
        "windows|first"  = "${HOARD_TMP}/first_named_dir1"
        "windows|second" = "${HOARD_TMP}/second_named_dir1"
    [hoards.named.dir1.config]
        ignore = ["*pile*", ".hidden"]
    [hoards.named.dir2]
        "unix|first"  = "${HOME}/first_named_dir2"
        "unix|second" = "${HOME}/second_named_dir2"
        "windows|first"  = "${HOARD_TMP}/first_named_dir2"
        "windows|second" = "${HOARD_TMP}/second_named_dir2"
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
    pub async fn new() -> Self {
        Self::with_log_level(tracing::Level::INFO).await
    }

    pub async fn with_log_level(log_level: tracing::Level) -> Self {
        Self {
            tester: Tester::with_log_level(BASE_CONFIG, log_level).await,
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
        let system_path = SystemPath::try_from(self.home_dir().join(format!(
            "{}{}",
            self.file_prefix(),
            HOARD_ANON_FILE
        )))
        .unwrap();
        let hoard_path =
            HoardPath::try_from(self.data_dir().join("hoards").join(HOARD_ANON_FILE)).unwrap();
        HoardItem::new(
            PileName::anonymous(),
            hoard_path,
            system_path,
            RelativePath::none(),
        )
    }

    pub fn anon_dir(&self) -> HoardItem {
        let system_path = SystemPath::try_from(self.home_dir().join(format!(
            "{}{}",
            self.file_prefix(),
            HOARD_ANON_DIR
        )))
        .unwrap();
        let hoard_path =
            HoardPath::try_from(self.data_dir().join("hoards").join(HOARD_ANON_DIR)).unwrap();
        HoardItem::new(
            PileName::anonymous(),
            hoard_path,
            system_path,
            RelativePath::none(),
        )
    }

    pub fn named_file(&self) -> HoardItem {
        let system_path = SystemPath::try_from(self.home_dir().join(format!(
            "{}named_{}",
            self.file_prefix(),
            HOARD_NAMED_FILE
        )))
        .unwrap();
        let hoard_path = HoardPath::try_from(
            self.data_dir()
                .join("hoards")
                .join(HOARD_NAMED)
                .join(HOARD_NAMED_FILE),
        )
        .unwrap();
        HoardItem::new(
            HOARD_NAMED_FILE.parse().unwrap(),
            hoard_path,
            system_path,
            RelativePath::none(),
        )
    }

    pub fn named_dir1(&self) -> HoardItem {
        let system_path = SystemPath::try_from(self.home_dir().join(format!(
            "{}named_{}",
            self.file_prefix(),
            HOARD_NAMED_DIR1
        )))
        .unwrap();
        let hoard_path = HoardPath::try_from(
            self.data_dir()
                .join("hoards")
                .join(HOARD_NAMED)
                .join(HOARD_NAMED_DIR1),
        )
        .unwrap();
        HoardItem::new(
            HOARD_NAMED_DIR1.parse().unwrap(),
            hoard_path,
            system_path,
            RelativePath::none(),
        )
    }

    pub fn named_dir2(&self) -> HoardItem {
        let system_path = SystemPath::try_from(self.home_dir().join(format!(
            "{}named_{}",
            self.file_prefix(),
            HOARD_NAMED_DIR2
        )))
        .unwrap();
        let hoard_path = HoardPath::try_from(
            self.data_dir()
                .join("hoards")
                .join(HOARD_NAMED)
                .join(HOARD_NAMED_DIR2),
        )
        .unwrap();
        HoardItem::new(
            HOARD_NAMED_DIR2.parse().unwrap(),
            hoard_path,
            system_path,
            RelativePath::none(),
        )
    }

    fn file_in_hoard_dir(dir: &HoardItem, file: RelativePath) -> HoardItem {
        HoardItem::new(
            dir.pile_name().clone(),
            dir.hoard_prefix().clone(),
            dir.system_prefix().clone(),
            file,
        )
    }

    fn env_files_inner(&self) -> Vec<HoardItem> {
        let anon_dir = self.anon_dir();
        let named_dir1 = self.named_dir1();
        let named_dir2 = self.named_dir2();
        vec![
            self.anon_file(),
            Self::file_in_hoard_dir(
                &anon_dir,
                RelativePath::try_from(PathBuf::from(DIR_FILE_1)).unwrap(),
            ),
            Self::file_in_hoard_dir(
                &anon_dir,
                RelativePath::try_from(PathBuf::from(DIR_FILE_2)).unwrap(),
            ),
            Self::file_in_hoard_dir(
                &anon_dir,
                RelativePath::try_from(PathBuf::from(DIR_FILE_3)).unwrap(),
            ),
            self.named_file(),
            Self::file_in_hoard_dir(
                &named_dir1,
                RelativePath::try_from(PathBuf::from(DIR_FILE_1)).unwrap(),
            ),
            Self::file_in_hoard_dir(
                &named_dir1,
                RelativePath::try_from(PathBuf::from(DIR_FILE_2)).unwrap(),
            ),
            Self::file_in_hoard_dir(
                &named_dir1,
                RelativePath::try_from(PathBuf::from(DIR_FILE_3)).unwrap(),
            ),
            Self::file_in_hoard_dir(
                &named_dir2,
                RelativePath::try_from(PathBuf::from(DIR_FILE_1)).unwrap(),
            ),
            Self::file_in_hoard_dir(
                &named_dir2,
                RelativePath::try_from(PathBuf::from(DIR_FILE_2)).unwrap(),
            ),
            Self::file_in_hoard_dir(
                &named_dir2,
                RelativePath::try_from(PathBuf::from(DIR_FILE_3)).unwrap(),
            ),
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

    pub async fn setup_files(&mut self) {
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
                fs::create_dir_all(parent)
                    .await
                    .expect("creating parent dirs should not fail");
            }

            super::create_file_with_random_data::<2048>(&file).await;
        }
    }

    async fn hash_file(file: &Path) -> GenericArray<u8, <Sha256 as OutputSizeUser>::OutputSize> {
        let data = fs::read(file).await.expect("file should always exist");
        Sha256::digest(&data)
    }

    async fn file_contents(
        path: &Path,
        root: &Path,
    ) -> HashMap<PathBuf, GenericArray<u8, <Sha256 as OutputSizeUser>::OutputSize>> {
        let mut path_stack = vec![path.to_path_buf()];
        let mut contents_map = HashMap::new();

        while let Some(path) = path_stack.pop() {
            if path.is_file() {
                let key = path
                    .strip_prefix(root)
                    .expect("path should always have root as prefix")
                    .to_path_buf();
                contents_map.insert(key, Self::hash_file(&path).await);
            } else if path.is_dir() {
                let mut stream = ReadDirStream::new(fs::read_dir(path).await.unwrap());
                while let Some(entry) = stream.try_next().await.unwrap() {
                    path_stack.push(entry.path());
                }
            } else if !path.exists() {
                panic!("{} does not exist", path.display());
            } else {
                panic!("{} exists but is not a file or directory", path.display());
            }
        }

        contents_map
    }

    async fn assert_same_tree(left: &Path, right: &Path) {
        let left_content = Self::file_contents(left, left).await;
        let right_content = Self::file_contents(right, right).await;
        assert_eq!(
            left_content,
            right_content,
            "{} and {} do not have matching contents",
            left.display(),
            right.display()
        );
    }

    pub async fn assert_first_tree(&self) {
        let hoards_root = self.data_dir().join("hoards");
        Self::assert_same_tree(
            &self.home_dir().join("first_anon_file"),
            &hoards_root.join("anon_file"),
        )
        .await;
        Self::assert_same_tree(
            &self.home_dir().join("first_anon_dir"),
            &hoards_root.join("anon_dir"),
        )
        .await;
        Self::assert_same_tree(
            &self.home_dir().join("first_named_file"),
            &hoards_root.join("named").join("file"),
        )
        .await;
        Self::assert_same_tree(
            &self.home_dir().join("first_named_dir1"),
            &hoards_root.join("named").join("dir1"),
        )
        .await;
        Self::assert_same_tree(
            &self.home_dir().join("first_named_dir2"),
            &hoards_root.join("named").join("dir2"),
        )
        .await;
    }

    pub async fn assert_second_tree(&self) {
        let hoards_root = self.data_dir().join("hoards");
        Self::assert_same_tree(
            &self.home_dir().join("second_anon_file"),
            &hoards_root.join("anon_file"),
        )
        .await;
        Self::assert_same_tree(
            &self.home_dir().join("second_anon_dir"),
            &hoards_root.join("anon_dir"),
        )
        .await;
        Self::assert_same_tree(
            &self.home_dir().join("second_named_file"),
            &hoards_root.join("named").join("file"),
        )
        .await;
        Self::assert_same_tree(
            &self.home_dir().join("second_named_dir1"),
            &hoards_root.join("named").join("dir1"),
        )
        .await;
        Self::assert_same_tree(
            &self.home_dir().join("second_named_dir2"),
            &hoards_root.join("named").join("dir2"),
        )
        .await;
    }
}
