use std::fmt;
use std::str::FromStr;

use serde::{de, Deserialize, Deserializer, Serialize};

use super::{Error, NonEmptyPileName};

/// Newtype wrapper for `Option<String>` representing a pile name.
///
/// - `None` indicates an anonymous (unnamed) pile.
/// - `Some(name)` indicates a named pile with name `name`.
///
/// See the [module documentation](super) for what makes an acceptable name.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct PileName(Option<NonEmptyPileName>);

impl FromStr for PileName {
    type Err = Error;

    #[tracing::instrument(level = "trace", name = "parse_pile_name")]
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        NonEmptyPileName::from_str(value).map(Some).map(Self)
    }
}

struct PileNameVisitor;

impl<'de> de::Visitor<'de> for PileNameVisitor {
    type Value = PileName;

    fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "a valid pile name")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.parse().map_err(E::custom)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(self)
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(PileName(None))
    }
}

impl<'de> Deserialize<'de> for PileName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(PileNameVisitor)
    }
}

impl<T> TryFrom<Option<T>> for PileName
where
    T: AsRef<str>,
{
    type Error = Error;

    #[tracing::instrument(level = "trace", name = "pile_name_try_from_option_str", skip_all)]
    fn try_from(value: Option<T>) -> Result<Self, Self::Error> {
        match value {
            None => Ok(Self(None)),
            Some(inner) => inner.as_ref().parse(),
        }
    }
}

impl From<NonEmptyPileName> for PileName {
    fn from(value: NonEmptyPileName) -> Self {
        Self(Some(value))
    }
}

impl TryFrom<PileName> for NonEmptyPileName {
    type Error = Error;

    #[tracing::instrument(level = "trace", name = "non_empty_pile_name_try_from_pile_name")]
    fn try_from(value: PileName) -> Result<Self, Self::Error> {
        Option::<Self>::from(value).ok_or(Error::EmptyName)
    }
}

impl From<PileName> for Option<NonEmptyPileName> {
    fn from(name: PileName) -> Self {
        name.0
    }
}

impl fmt::Display for PileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_ref() {
            None => write!(f, ""),
            Some(name) => write!(f, "{name}"),
        }
    }
}

impl PileName {
    /// Returns the `PileName` for an anonymous pile.
    #[must_use]
    pub fn anonymous() -> Self {
        Self(None)
    }

    /// Returns whether the `PileName` represents an anonymous pile.
    #[must_use]
    pub fn is_anonymous(&self) -> bool {
        self.0.is_none()
    }

    /// Like [`Option::as_ref`] on the inner value.
    #[must_use]
    pub fn as_ref(&self) -> Option<&NonEmptyPileName> {
        self.0.as_ref()
    }

    /// Like [`Option::as_deref`] on the inner value.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        self.0.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use serde_test::{assert_de_tokens, assert_tokens, Token};

    use super::*;

    #[test]
    fn test_from_str() {
        let inputs = vec![
            ("", Err(Error::DisallowedName(String::new()))),
            ("name", Ok(PileName(Some("name".parse().unwrap())))),
            (
                "invalid name",
                Err(Error::DisallowedCharacters(String::from("invalid name"))),
            ),
        ];

        for (s, expected) in inputs {
            let result = s.parse();
            match (expected, result) {
                (Ok(name1), Ok(name2)) => {
                    assert_eq!(name1, name2, "expected {name1} but got {name2}");
                }
                (Err(err1), Err(err2)) => match (&err1, &err2) {
                    (Error::EmptyName, Error::EmptyName) => {}
                    (Error::EmptyName, _) | (_, Error::EmptyName) => {
                        panic!("expected {err1:?}, got {err2:?}");
                    }
                    (
                        Error::DisallowedName(invalid1) | Error::DisallowedCharacters(invalid1),
                        Error::DisallowedName(invalid2) | Error::DisallowedCharacters(invalid2),
                    ) => {
                        assert_eq!(
                            invalid1, invalid2,
                            "expected invalid string to be {invalid1}, was {invalid2}"
                        );
                    }
                },
                (Ok(name), Err(err)) => {
                    panic!("expected successful parse {name:?}, got error {err:?}");
                }
                (Err(err), Ok(name)) => {
                    panic!("expected error {err:?}, got success with {name:?}");
                }
            }
        }
    }

    #[test]
    fn test_serde_some() {
        let name: PileName = "name".parse().unwrap();
        assert_tokens(&name, &[Token::Some, Token::Str("name")]);
    }

    #[test]
    fn test_serde_none() {
        let name = PileName::anonymous();
        assert_tokens(&name, &[Token::None]);
    }

    #[test]
    fn test_serde_empty_str() {
        serde_test::assert_de_tokens_error::<PileName>(
            &[Token::Str("")],
            "name \"\" is not allowed",
        );
    }

    #[test]
    fn test_serde_non_empty_str() {
        let name = PileName::from_str("valid").unwrap();
        assert_de_tokens(&name, &[Token::Str("valid")]);
    }

    #[test]
    fn test_serde_invalid_type() {
        serde_test::assert_de_tokens_error::<PileName>(
            &[Token::U8(5)],
            "invalid type: integer `5`, expected a valid pile name",
        );
    }

    #[test]
    fn test_try_from_option_str() {
        let op = Some("valid");
        let expected = PileName::from_str("valid").unwrap();
        assert_eq!(PileName::try_from(op).unwrap(), expected);
        let none: Option<&str> = None;
        assert_eq!(PileName::try_from(none).unwrap(), PileName::anonymous());
    }

    #[test]
    fn test_from_non_empty_pile_name() {
        let non_empty: NonEmptyPileName = "valid".parse().unwrap();
        let expected: PileName = "valid".parse().unwrap();
        let result = PileName::from(non_empty);
        assert_eq!(result, expected);
    }

    #[test]
    fn test_into_option_non_empty_pile_name() {
        assert_eq!(
            None,
            Option::<NonEmptyPileName>::from(PileName::anonymous())
        );

        let non_empty: NonEmptyPileName = "valid".parse().unwrap();
        let name = PileName::from(non_empty.clone());
        assert_eq!(Some(non_empty), Option::<NonEmptyPileName>::from(name));
    }

    #[test]
    fn test_try_into_non_empty_pile_name() {
        let error = NonEmptyPileName::try_from(PileName::anonymous())
            .expect_err("anonymous pile name is empty");
        assert!(matches!(error, Error::EmptyName));

        let non_empty: NonEmptyPileName = "testing".parse().unwrap();
        assert_eq!(
            non_empty.clone(),
            NonEmptyPileName::try_from(PileName::from(non_empty)).unwrap()
        );
    }

    #[test]
    fn test_to_string() {
        assert_eq!("", PileName::anonymous().to_string());
        let s = "testing";
        let name: PileName = s.parse().unwrap();
        assert_eq!(s, name.to_string());
    }

    #[test]
    fn test_anonymous_constructor() {
        assert_eq!(PileName(None), PileName::anonymous());
    }

    #[test]
    fn test_is_anonymous() {
        assert!(PileName(None).is_anonymous());
        assert!(!PileName(Some("test".parse().unwrap())).is_anonymous());
    }

    #[test]
    fn test_as_ref() {
        let name: PileName = "testing".parse().unwrap();
        assert_eq!(name.0.as_ref(), name.as_ref());
        assert_eq!(None, PileName::anonymous().as_ref());
    }

    #[test]
    fn test_as_str() {
        let name: PileName = "testing".parse().unwrap();
        assert_eq!(Some("testing"), name.as_str());
        assert_eq!(None, PileName::anonymous().as_str());
    }
}
