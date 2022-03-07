use md5::Digest as _;
use serde::{Deserialize, Serialize};
use sha2::Digest as _;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};
use crate::hoard::{HoardPath, SystemPath};

#[derive(Debug, PartialEq, Eq, PartialOrd, Hash, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChecksumType {
    MD5,
    SHA256,
}

impl Default for ChecksumType {
    fn default() -> Self {
        Self::SHA256
    }
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Checksum {
    MD5(String),
    SHA256(String),
}

impl Checksum {
    pub(crate) fn typ(&self) -> ChecksumType {
        match self {
            Self::MD5(_) => ChecksumType::MD5,
            Self::SHA256(_) => ChecksumType::SHA256,
        }
    }
}

impl fmt::Display for Checksum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MD5(md5) => write!(f, "md5({})", md5),
            Self::SHA256(sha256) => write!(f, "sha256({})", sha256),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct HoardFile {
    pile_name: Option<String>,
    hoard_prefix: HoardPath,
    system_prefix: SystemPath,
    hoard_path: HoardPath,
    system_path: SystemPath,
    relative_path: PathBuf,
}

impl HoardFile {
    pub fn new(
        pile_name: Option<String>,
        hoard_prefix: HoardPath,
        system_prefix: SystemPath,
        relative_path: PathBuf,
    ) -> Self {
        let (hoard_path, system_path) = if relative_path.to_str() == Some("") {
            (hoard_prefix.clone(), system_prefix.clone())
        } else {
            let hoard_path = HoardPath::from(hoard_prefix.join(&relative_path));
            let system_path = SystemPath::from(system_prefix.join(&relative_path));
            (hoard_path, system_path)
        };

        Self {
            pile_name,
            hoard_prefix,
            system_prefix,
            hoard_path,
            system_path,
            relative_path,
        }
    }

    pub fn pile_name(&self) -> Option<&str> {
        self.pile_name.as_deref()
    }

    pub fn relative_path(&self) -> &Path {
        &self.relative_path
    }

    pub fn hoard_prefix(&self) -> &Path {
        &self.hoard_prefix
    }

    pub fn system_prefix(&self) -> &Path {
        &self.system_prefix
    }

    pub fn hoard_path(&self) -> &Path {
        &self.hoard_path
    }

    pub fn system_path(&self) -> &Path {
        &self.system_path
    }

    pub fn is_file(&self) -> bool {
        let sys = self.system_path();
        let hoard = self.hoard_path();
        let sys_exists = sys.exists();
        let hoard_exists = hoard.exists();
        (sys.is_file() || !sys_exists)
            && (hoard.is_file() || !hoard_exists)
            && (sys_exists || hoard_exists)
    }

    pub fn is_dir(&self) -> bool {
        let sys = self.system_path();
        let hoard = self.hoard_path();
        let sys_exists = sys.exists();
        let hoard_exists = hoard.exists();
        (sys.is_dir() || !sys_exists)
            && (hoard.is_dir() || !hoard_exists)
            && (sys_exists || hoard_exists)
    }

    fn content(path: &Path) -> io::Result<Option<Vec<u8>>> {
        match fs::read(path) {
            Ok(content) => Ok(Some(content)),
            Err(err) => match err.kind() {
                ErrorKind::NotFound => Ok(None),
                _ => Err(err),
            },
        }
    }

    pub fn system_content(&self) -> io::Result<Option<Vec<u8>>> {
        Self::content(self.system_path())
    }

    pub fn hoard_content(&self) -> io::Result<Option<Vec<u8>>> {
        Self::content(self.hoard_path())
    }

    pub(crate) fn hoard_checksum(&self, typ: ChecksumType) -> io::Result<Option<Checksum>> {
        match typ {
            ChecksumType::MD5 => self.hoard_md5(),
            ChecksumType::SHA256 => self.hoard_sha256(),
        }
    }

    pub fn hoard_md5(&self) -> io::Result<Option<Checksum>> {
        self.hoard_content()
            .map(|content| content.as_deref().map(Self::md5))
    }

    pub fn hoard_sha256(&self) -> io::Result<Option<Checksum>> {
        self.hoard_content()
            .map(|content| content.as_deref().map(Self::sha256))
    }

    pub(crate) fn system_checksum(&self, typ: ChecksumType) -> io::Result<Option<Checksum>> {
        match typ {
            ChecksumType::MD5 => self.system_md5(),
            ChecksumType::SHA256 => self.system_sha256(),
        }
    }

    pub fn system_md5(&self) -> io::Result<Option<Checksum>> {
        self.system_content()
            .map(|content| content.as_deref().map(Self::md5))
    }

    pub fn system_sha256(&self) -> io::Result<Option<Checksum>> {
        self.system_content()
            .map(|content| content.as_deref().map(Self::sha256))
    }

    pub(crate) fn md5(content: &[u8]) -> Checksum {
        let digest = md5::Md5::digest(content);
        let hash = format!("{:x}", digest);
        Checksum::MD5(hash)
    }

    pub(crate) fn sha256(content: &[u8]) -> Checksum {
        let digest = sha2::Sha256::digest(content);
        let hash = format!("{:x}", digest);
        Checksum::SHA256(hash)
    }
}
