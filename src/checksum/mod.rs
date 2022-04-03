#![allow(clippy::module_name_repetitions)]
//! Module for handling checksums.
use serde::{Deserialize, Serialize};
use std::fmt;
mod digest;

pub use self::digest::{MD5, SHA256};

/// The types of checksums supported by Hoard.
#[derive(Debug, PartialEq, Eq, PartialOrd, Hash, Copy, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChecksumType {
    /// MD5 checksum -- provided for backwards compatibility with older versions of Hoard.
    MD5,
    /// SHA256 checksum -- currently the default.
    SHA256,
}

impl Default for ChecksumType {
    fn default() -> Self {
        Self::SHA256
    }
}

/// A file's checksum as a human-readable string.
///
/// If you have a choice of which variant to construct,
/// prefer using [`HoardItem::system_checksum`] or [`HoardItem::hoard_checksum`] with the
/// return value of [`ChecksumType::default()`]
///
/// # TODO
///
/// - Ensure that the contained values can never be invalid for the associated checksum type.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Checksum {
    /// An MD5 checksum -- provided for backwards compatibility with older versions of Hoard.
    MD5(MD5),
    /// A SHA256 checksum -- currently the default.
    SHA256(SHA256),
}

impl Checksum {
    /// Returns the [`ChecksumType`] for this `Checksum`.
    ///
    /// ```
    /// # use hoard::checksum::{Checksum, ChecksumType};
    /// let checksum = Checksum::SHA256("50d858e0985ecc7f60418aaf0cc5ab587f42c2570a884095a9e8ccacd0f6545c".parse().unwrap());
    /// assert_eq!(checksum.typ(), ChecksumType::SHA256);
    /// let checksum = Checksum::MD5("ae2b1fca515949e5d54fb22b8ed95575".parse().unwrap());
    /// assert_eq!(checksum.typ(), ChecksumType::MD5);
    /// ```
    #[must_use]
    pub fn typ(&self) -> ChecksumType {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checksum_display() {
        let shasum = "50d858e0985ecc7f60418aaf0cc5ab587f42c2570a884095a9e8ccacd0f6545c";
        let checksum = Checksum::SHA256(shasum.parse().unwrap());
        assert_eq!(format!("sha256({})", shasum), checksum.to_string());
        let md5sum = "ae2b1fca515949e5d54fb22b8ed95575";
        let checksum = Checksum::MD5(md5sum.parse().unwrap());
        assert_eq!(format!("md5({})", md5sum), checksum.to_string());
    }

    #[test]
    fn test_checksum_type() {
        assert_eq!(ChecksumType::MD5, Checksum::MD5("ae2b1fca515949e5d54fb22b8ed95575".parse().unwrap()).typ());
        assert_eq!(ChecksumType::SHA256, Checksum::SHA256("50d858e0985ecc7f60418aaf0cc5ab587f42c2570a884095a9e8ccacd0f6545c".parse().unwrap()).typ());
    }
}
