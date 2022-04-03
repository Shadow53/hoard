use digest::generic_array::{ArrayLength, GenericArray};
use digest::typenum::Unsigned;
use hex::FromHex;
use serde::{de::Error as _, Deserialize, Deserializer, Serialize, Serializer};
use std::cmp::Ordering;
use std::fmt;
use std::fmt::LowerHex;
use std::hash::{Hash, Hasher};
use std::ops::Add;
use std::str::FromStr;
use thiserror::Error;

/// Digest definition for MD5.
pub type MD5 = Digest<md5::Md5>;
/// Digest definition for SHA256.
pub type SHA256 = Digest<sha2::Sha256>;

fn checksum_to_string<H>(data: H) -> String
where
    H: LowerHex,
{
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
        GenericArray::from_exact_iter(v).ok_or_else(|| Error::InvalidLength {
            received_len,
            expected_len: Self::OutputSize::to_usize(),
        })
    }
    fn digest_to_array<D: AsRef<[u8]>>(data: D) -> GenericArray<u8, Self::OutputSize>;
    fn digest_to_string<D: AsRef<[u8]>>(data: D) -> String
    where
        <Self::OutputSize as Add<Self::OutputSize>>::Output: ArrayLength<u8>,
    {
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
    InvalidLength {
        received_len: usize,
        expected_len: usize,
    },
}

#[derive(Debug)]
#[repr(transparent)]
pub struct Digest<T>(GenericArray<u8, T::OutputSize>)
where
    T: Digestable,
    <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>;

// The following stdlib derives did not work, so do manually
impl<T> PartialEq for Digest<T>
where
    T: Digestable,
    <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>,
{
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<T> Eq for Digest<T>
where
    T: Digestable,
    <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>,
{
}

impl<T> Ord for Digest<T>
where
    T: Digestable,
    <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<T> PartialOrd for Digest<T>
where
    T: Digestable,
    <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<T> Hash for Digest<T>
where
    T: Digestable,
    <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>,
{
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl<T> Clone for Digest<T>
where
    T: Digestable,
    <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>,
{
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Serialize for Digest<T>
where
    T: Digestable,
    <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de, T> Deserialize<'de> for Digest<T>
where
    T: Digestable,
    <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(D::Error::custom)
    }
}

impl<T> fmt::Display for Digest<T>
where
    T: Digestable,
    <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", checksum_to_string(&self.0))
    }
}

impl<T> FromStr for Digest<T>
where
    T: Digestable,
    <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>,
{
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        T::digest_from_str(s).map(Digest)
    }
}

impl<T> Digest<T>
where
    T: Digestable,
    <<T as Digestable>::OutputSize as Add>::Output: ArrayLength<u8>,
{
    /// Create a digest of the given data.
    pub fn from_data<D>(data: D) -> Self
    where
        D: AsRef<[u8]>,
    {
        Self(T::digest_to_array(data))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DATA: &str = "testing";

    const MD5_STR: &str = "ae2b1fca515949e5d54fb22b8ed95575";
    const MD5_ARR: [u8; 16] = [
        0xae, 0x2b, 0x1f, 0xca, 0x51, 0x59, 0x49, 0xe5, 0xd5, 0x4f, 0xb2, 0x2b, 0x8e, 0xd9, 0x55, 0x75
    ];

    fn get_digest() -> MD5 {
        MD5::from_data(DATA)
    }

    // TODO: Error conditions for digest_from_str()

    mod md5 {
        use super::*;
        use ::md5::Md5;

        #[test]
        fn test_md5_output_size() {
            assert_eq!(<Md5 as Digestable>::OutputSize::to_usize(), MD5_ARR.len());
        }

        #[test]
        fn test_md5_digest_to_array() {
            let result = Md5::digest_to_array(&DATA);
            let expected = GenericArray::<u8, <Md5 as Digestable>::OutputSize>::from_slice(&MD5_ARR);
            assert_eq!(&result, expected);
        }

        #[test]
        fn test_md5_digest_to_string() {
            let result = Md5::digest_to_string(&DATA);
            assert_eq!(&result, MD5_STR);
        }

        #[test]
        fn test_md5_digest_from_str() {
            let expected = Md5::digest_to_array(&DATA);
            let result = Md5::digest_from_str(MD5_STR).unwrap();
            assert_eq!(expected, result);
        }
    }

    mod sha256 {
        use super::*;
        use ::sha2::Sha256;

        const SHA256_STR: &str = "cf80cd8aed482d5d1527d7dc72fceff84e6326592848447d2dc0b0e87dfc9a90";
        const SHA256_ARR: [u8; 32] = [
            0xcf, 0x80, 0xcd, 0x8a, 0xed, 0x48, 0x2d, 0x5d, 0x15, 0x27, 0xd7, 0xdc, 0x72, 0xfc, 0xef, 0xf8,
            0x4e, 0x63, 0x26, 0x59, 0x28, 0x48, 0x44, 0x7d, 0x2d, 0xc0, 0xb0, 0xe8, 0x7d, 0xfc, 0x9a, 0x90,
        ];

        #[test]
        fn test_sha256_output_size() {
            assert_eq!(<Sha256 as Digestable>::OutputSize::to_usize(), SHA256_ARR.len());
        }

        #[test]
        fn test_sha256_digest_to_array() {
            let result = Sha256::digest_to_array(&DATA);
            let expected = GenericArray::<u8, <Sha256 as Digestable>::OutputSize>::from_slice(&SHA256_ARR);
            assert_eq!(&result, expected);
        }

        #[test]
        fn test_sha256_digest_to_string() {
            let result = Sha256::digest_to_string(&DATA);
            assert_eq!(&result, SHA256_STR);
        }

        #[test]
        fn test_sha256_digest_from_str() {
            let expected = Sha256::digest_to_array(&DATA);
            let result = Sha256::digest_from_str(SHA256_STR).unwrap();
            assert_eq!(expected, result);
        }
    }

    mod digest {
        use super::*;
        use ::md5::Md5;
        use serde_test::{assert_tokens, Token};

        #[test]
        fn test_from_str() {
            let result = MD5::from_str(MD5_STR).unwrap();
            let expected = Digest(GenericArray::<u8, <Md5 as Digestable>::OutputSize>::from(MD5_ARR));
            assert_eq!(expected, result);
            let error = MD5::from_str("bad1").expect_err("\"bad1\" is an invalid MD5 checksum");
            if let Error::InvalidLength { received_len, expected_len } = error {
                // Each two characters is one byte
                assert_eq!(received_len, 2);
                assert_eq!(expected_len, <Md5 as Digestable>::OutputSize::to_usize());
            } else {
                panic!("expected InvalidLength error, got {:?}", error);
            }
        }

        #[test]
        fn test_eq() {
            // Last character differs between these strings
            let first = MD5::from_str("ae2b1fca515949e5d54fb22b8ed95575").unwrap();
            let second = MD5::from_str("ae2b1fca515949e5d54fb22b8ed95576").unwrap();

            assert_eq!(first, first);
            assert_eq!(second, second);
            assert_ne!(first, second);
        }

        #[test]
        fn test_ord() {
            let first = MD5::from_str("ae2b1fca515949e5d54fb22b8ed95575").unwrap();
            let second = MD5::from_str("ae2b1fca515949e5d54fb22b8ed95576").unwrap();

            assert!(matches!(first.partial_cmp(&second), Some(Ordering::Less)));
            assert!(matches!(first.cmp(&second), Ordering::Less));
            assert!(matches!(second.partial_cmp(&first), Some(Ordering::Greater)));
            assert!(matches!(second.cmp(&first), Ordering::Greater));
            assert!(matches!(first.partial_cmp(&first), Some(Ordering::Equal)));
            assert!(matches!(first.cmp(&first), Ordering::Equal));
        }

        #[test]
        fn test_clone() {
            let digest = get_digest();
            assert_eq!(digest, digest.clone());
        }

        #[test]
        fn test_serde() {
            let digest = get_digest();
            assert_tokens(&digest, &[
                Token::Str(MD5_STR)
            ]);
        }

        #[test]
        fn test_display() {
            assert_eq!(MD5_STR, &format!("{}", get_digest()));
        }

        #[test]
        fn test_from_data() {
            assert_eq!(
                MD5::from_data(DATA),
                MD5::from_str(MD5_STR).unwrap(),
            );
        }
    }
}
