use std::{fmt, ops::Deref, str::FromStr};

use serde::{de::Error as _, Deserialize, Deserializer, Serialize};

use super::{validate_name, Error};

/// Newtype wrapper for `String` representing an [environment](https://hoard.rs/config/envs.html).
///
/// See the [module documentation](super) for what makes an acceptable name.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct EnvironmentName(String);

impl FromStr for EnvironmentName {
    type Err = Error;
    #[tracing::instrument(level = "trace", name = "parse_environment_name")]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_name(s.to_string()).map(Self)
    }
}

impl Deref for EnvironmentName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.as_str()
    }
}

impl fmt::Display for EnvironmentName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.deref().fmt(f)
    }
}

impl<'de> Deserialize<'de> for EnvironmentName {
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
            ("", Err(Error::DisallowedName(String::new()))),
            (
                "invalid name",
                Err(Error::DisallowedCharacters(String::from("invalid name"))),
            ),
            ("valid", Ok(EnvironmentName(String::from("valid")))),
        ];

        for (s, expected) in inputs {
            let result = s.parse::<EnvironmentName>();
            match (result, expected) {
                (Ok(result), Err(expected)) => {
                    panic!("expected error {expected:?} but got success {result:?}")
                }
                (Err(err), Ok(expected)) => {
                    panic!("expected success {expected:?} but got error {err:?}")
                }
                (Ok(result), Ok(expected)) => {
                    assert_eq!(result, expected, "expected {expected:?} but got {result:?}");
                }
                (Err(err), Err(expected)) => {
                    assert_eq!(err, expected, "expected error {expected:?} but got {err:?}");
                }
            }
        }
    }

    #[test]
    #[allow(clippy::explicit_deref_methods)]
    fn test_deref() {
        let s = "testing";
        let name: EnvironmentName = s.parse().unwrap();
        assert_eq!(s, name.deref());
    }

    #[test]
    fn test_to_string() {
        let s = "test_name";
        let name: EnvironmentName = s.parse().unwrap();
        assert_eq!(s, name.to_string());
    }

    #[test]
    fn test_serde() {
        let name: EnvironmentName = "testing".parse().unwrap();
        assert_tokens(&name, &[Token::Str("testing")]);
    }
}
