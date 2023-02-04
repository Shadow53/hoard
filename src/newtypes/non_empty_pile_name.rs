use std::{fmt, ops::Deref, str::FromStr};

use serde::{Deserialize, Serialize};

use super::{validate_name, Error};

/// Like [`PileName`](super::PileName), but not allowed to be empty ("anonymous")
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(try_from = "String")]
#[serde(into = "String")]
pub struct NonEmptyPileName(String);

impl Deref for NonEmptyPileName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for NonEmptyPileName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for NonEmptyPileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for NonEmptyPileName {
    type Err = Error;

    #[tracing::instrument(level = "trace", name = "parse_non_empty_pile_name")]
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validate_name(value.to_string()).map(Self)
    }
}

impl TryFrom<String> for NonEmptyPileName {
    type Error = Error;

    #[tracing::instrument(level = "trace", name = "non_empty_pile_name_try_from_string")]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        validate_name(value).map(Self)
    }
}

impl From<NonEmptyPileName> for String {
    fn from(name: NonEmptyPileName) -> Self {
        name.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::explicit_deref_methods)]
    fn test_deref() {
        let name = NonEmptyPileName(String::from("testing"));
        assert_eq!("testing", name.deref());
    }

    #[test]
    fn test_as_ref() {
        let name = NonEmptyPileName(String::from("testing"));
        assert_eq!("testing", name.as_ref());
    }

    #[test]
    fn test_to_string() {
        let s = String::from("testing");
        let name = NonEmptyPileName(s.clone());
        assert_eq!(s, name.to_string());
    }

    #[test]
    fn test_from_str_and_try_from_string() {
        let inputs = vec![
            (String::new(), Err(Error::DisallowedName(String::new()))),
            (
                String::from("testing"),
                Ok(NonEmptyPileName(String::from("testing"))),
            ),
            (
                String::from("config"),
                Err(Error::DisallowedName(String::from("config"))),
            ),
        ];

        for (s, expected) in inputs {
            let from_str = s.parse::<NonEmptyPileName>();
            let try_from = NonEmptyPileName::try_from(s);
            match (&from_str, &try_from, &expected) {
                (Ok(_), Err(_), _) | (Err(_), Ok(_), _) => {
                    panic!(
                        "from_str ({from_str:?}) and try_from_string ({try_from:?}) returned different results"
                    )
                }
                (Ok(result), Ok(_), Err(err)) => {
                    panic!("conversion succeeded ({result}) but expected to fail with {err:?}");
                }
                (Err(err), Err(_), Ok(result)) => {
                    panic!(
                        "conversion failed with {err:?} but expected to succeed with {result:?}"
                    );
                }
                (Ok(from_str), Ok(try_from), Ok(expected)) => {
                    assert_eq!(
                        from_str, try_from,
                        "from_str and try_from_string returned different results"
                    );
                    assert_eq!(from_str, expected, "expected {expected} but got {from_str}");
                }
                (Err(from_str), Err(try_from), Err(expected)) => {
                    assert_eq!(
                        from_str, try_from,
                        "from_str and try_from_string returned different errors"
                    );
                    assert_eq!(
                        from_str, expected,
                        "expected error {expected:?} but got {from_str:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_into_string() {
        let s = String::from("testing");
        let name = NonEmptyPileName(s.clone());
        assert_eq!(s, name.to_string());
    }
}
