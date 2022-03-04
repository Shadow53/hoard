use std::io::Write;

use rand::RngCore;
use tempfile::NamedTempFile;

pub mod base;
pub mod file;
pub mod toml;
pub mod test_subscriber;
pub mod tester;

pub fn create_random_file<const SIZE: usize>() -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("failed to create temporary file");
    let mut content = [0; SIZE];
    rand::thread_rng().fill_bytes(&mut content);
    file.write_all(&content)
        .expect("failed to write random data to temp file");
    file
}

#[derive(Copy, Clone, Hash, PartialEq, Eq)]
pub enum UuidLocation {
    Local,
    Remote,
}