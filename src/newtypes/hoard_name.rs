use std::{fmt, ops::Deref, str::FromStr};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use super::{validate_name, Error};

/// Newtype wrapper for `String` representing a hoard name.
///
/// See the [module documentation](super) for what makes an acceptable name.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct HoardName(String);

impl FromStr for HoardName {
    type Err = Error;
    #[tracing::instrument(level = "trace", name = "parse_hoard_name")]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_name(s.to_string()).map(Self)
    }
}

impl AsRef<str> for HoardName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for HoardName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl fmt::Display for HoardName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deref().fmt(f)
    }
}

impl<'de> Deserialize<'de> for HoardName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let inner = String::deserialize(deserializer)?;
        inner.parse().map_err(D::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use serde_test::{assert_tokens, Token};

    use super::*;

    #[test]
    fn test_from_str() {
        let inputs = [
            (
                String::new(),
                Err(Error::DisallowedName(String::new())),
            ),
            (
                String::from("config"),
                Err(Error::DisallowedName(String::from("config"))),
            ),
            (
                String::from("bad name"),
                Err(Error::DisallowedCharacters(String::from("bad name"))),
            ),
            (String::from("valid"), Ok(HoardName(String::from("valid")))),
        ];

        for (s, expected) in inputs {
            let result = s.parse::<HoardName>();
            match (&result, &expected) {
                (Ok(name), Err(err)) => panic!("expected error {err:?}, got success {name:?}"),
                (Err(err), Ok(name)) => panic!("expected success {name:?}, got error {err:?}"),
                (Ok(name), Ok(expected)) => {
                    assert_eq!(name, expected, "expected {expected} but got {name}");
                }
                (Err(err), Err(expected)) => {
                    assert_eq!(err, expected, "expected {expected:?} but got {err:?}");
                }
            }
        }
    }

    #[test]
    #[allow(clippy::explicit_deref_methods)]
    fn test_as_ref_and_deref() {
        let s = "testing";
        let name: HoardName = s.parse().unwrap();
        assert_eq!(s, name.as_ref());
        assert_eq!(s, name.deref());
    }

    #[test]
    fn test_to_string() {
        let s = "testing";
        let name: HoardName = s.parse().unwrap();
        assert_eq!(s, name.to_string());
    }

    #[test]
    fn test_serde() {
        let name: HoardName = "testing".parse().unwrap();
        assert_tokens(&name, &[Token::Str("testing")]);
    }
}
