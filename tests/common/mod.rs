#![allow(unused)]

use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use rand::RngCore;
use tempfile::NamedTempFile;

pub mod base;
pub mod file;
pub mod test_subscriber;
pub mod tester;
pub mod toml;

pub async fn create_random_file<const SIZE: usize>() -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("failed to create temporary file");
    create_file_with_random_data::<SIZE>(file.path()).await;
    file
}

pub async fn create_file_with_random_data<const SIZE: usize>(path: &Path) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .await
            .expect("failed to ensure parent directories");
    }

    let mut content = [0; SIZE];
    rand::thread_rng().fill_bytes(&mut content);
    fs::write(path, content)
        .await
        .expect("failed to write random data to file");
}

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub enum UuidLocation {
    Local,
    Remote,
}
