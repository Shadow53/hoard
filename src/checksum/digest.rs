use std::cmp::Ordering;
use std::fmt;
use std::fmt::LowerHex;
use std::hash::{Hash, Hasher};
use std::ops::Add;
use std::str::FromStr;
use digest::generic_array::{ArrayLength, GenericArray};
use digest::typenum::Unsigned;
use hex::FromHex;
use serde::{Serialize, Deserialize, Serializer, Deserializer, de::Error as _};
use thiserror::Error;

/// Digest definition for MD5.
pub type MD5 = Digest<md5::Md5>;
/// Digest definition for SHA256.
pub type SHA256 = Digest<sha2::Sha256>;

fn checksum_to_string<H>(data: H) -> String where H: LowerHex {
    format!("{:x}", data)
}

mod sealed {
    pub trait Sealed {}
    impl Sealed for md5::Md5 {}
    impl Sealed for sha2::Sha256 {}
}

pub trait Digestable: sealed::Sealed {
    type OutputSize: ArrayLength<u8> + Add<Self::OutputSize>;
    fn digest_from_str(s: &str) -> Result<GenericArray<u8, Self::OutputSize>, Error> {
        let v = Vec::from_hex(s).map_err(|_| Error::InvalidDigest(s.to_string()))?;
        let received_len = v.len();
        GenericArray::from_exact_iter(v).ok_or_else(|| {
            Error::InvalidLength {
                received_len,
                expected_len: Self::OutputSize::to_usize(),
            }
        })
    }
    fn digest_to_array<D: AsRef<[u8]>>(data: D) -> GenericArray<u8, Self::OutputSize>;
    fn digest_to_string<D: AsRef<[u8]>>(data: D) -> String where <Self::OutputSize as Add<Self::OutputSize>>::Output: ArrayLength<u8> {
        checksum_to_string(Self::digest_to_array(data))
    }
}

impl Digestable for md5::Md5 {
    type OutputSize = <md5::Md5 as md5::digest::OutputSizeUser>::OutputSize;
    fn digest_to_array<D: AsRef<[u8]>>(data: D) -> GenericArray<u8, Self::OutputSize> {
        <md5::Md5 as md5::Digest>::digest(data.as_ref())
    }
}

impl Digestable for sha2::Sha256 {
    type OutputSize = <sha2::Sha256 as sha2::digest::OutputSizeUser>::OutputSize;
    fn digest_to_array<D: AsRef<[u8]>>(data: D) -> GenericArray<u8, Self::OutputSize> {
        <sha2::Sha256 as sha2::Digest>::digest(data.as_ref())
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("invalid checksum string: {0}")]
    InvalidDigest(String),
    #[error("expected checksum of length {expected_len}, got {received_len}")]
    InvalidLength { received_len: usize, expected_len: usize },
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Digest<T>(GenericArray<u8, T::OutputSize>) where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>;

// The following stdlib derives did not work, so do manually
impl<T> PartialEq for Digest<T> where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for Digest<T> where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {}

impl<T> Ord for Digest<T> where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T> PartialOrd for Digest<T> where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<T> Hash for Digest<T> where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T> Clone for Digest<T> where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Serialize for Digest<T> where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        self.to_string().serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for Digest<T> where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(D::Error::custom)
    }
}

impl<T> fmt::Display for Digest<T> where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", checksum_to_string(&self.0))
    }
}

impl<T> From<Digest<T>> for String where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {
    fn from(d: Digest<T>) -> Self {
        d.to_string()
    }
}

impl<T> FromStr for Digest<T> where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        T::digest_from_str(s).map(Digest)
    }
}

impl<T> TryFrom<String> for Digest<T> where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {
    type Error = Error;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

impl<T> Digest<T> where T: Digestable, <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8> {
    /// Create a digest of the given data.
    pub fn from_data<D>(data: D) -> Self where D: AsRef<[u8]> {
        Self(T::digest_to_array(data))
    }
}