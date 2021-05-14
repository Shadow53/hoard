use std::io::Write;

use rand::RngCore;
use tempfile::NamedTempFile;

pub mod file;
pub mod toml;

pub fn create_random_file(size: usize) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("failed to create temporary file");
    let mut content = [0; 1024];
    rand::thread_rng().fill_bytes(&mut content);
    file.write_all(&content)
        .expect("failed to write random data to temp file");
    file
}
