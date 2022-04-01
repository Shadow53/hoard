use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Error as _};
use super::{EnvironmentName, Error};

/// Newtype wrapper for `HashSet<EnvironmentName>` representing a list of environments.
///
///
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct EnvironmentString(BTreeSet<EnvironmentName>);

impl FromStr for EnvironmentString {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.split('|')
            .map(EnvironmentName::from_str)
            .collect::<Result<_, _>>()
            .map(Self)
    }
}

impl IntoIterator for EnvironmentString {
    type Item = EnvironmentName;
    type IntoIter = std::collections::btree_set::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a EnvironmentString {
    type Item = &'a EnvironmentName;
    type IntoIter = std::collections::btree_set::Iter<'a, EnvironmentName>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl fmt::Display for EnvironmentString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut iter = self.0.iter().peekable();
        loop {
            match iter.next() {
                None => break,
                Some(name) => write!(f, "{}", name)?,
            }
            if iter.peek().is_some() {
                write!(f, "|")?;
            }
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for EnvironmentString {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
    {
        let inner = String::deserialize(deserializer)?;
        inner.parse().map_err(D::Error::custom)
    }
}

impl Serialize for EnvironmentString {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl From<EnvironmentName> for EnvironmentString {
    fn from(name: EnvironmentName) -> Self {
        Self({
            let mut set = BTreeSet::new();
            set.insert(name);
            set
        })
    }
}

impl EnvironmentString {
    /// Inserts the given [`EnvironmentName`] into `self`.
    pub fn insert(&mut self, name: EnvironmentName) {
        self.0.insert(name);
    }

    /// Iterates over the [`EnvironmentName`]s in this `EnvironmentString`.
    pub fn iter(&self) -> std::collections::btree_set::Iter<'_, EnvironmentName> {
        self.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use serde_test::{assert_tokens, Token};
    use super::*;

    const NAME_1: &str = "3rd";
    const NAME_2: &str = "FIRST";
    const NAME_3: &str = "the_Second";

    fn expected() -> EnvironmentString {
        EnvironmentString(maplit::btreeset! {
            NAME_1.parse().unwrap(),
            NAME_2.parse().unwrap(),
            NAME_3.parse().unwrap(),
        })
    }

    #[test]
    fn test_from_str() {
        let expected = expected();
        let result1 = format!("{}|{}|{}", NAME_1, NAME_2, NAME_3).parse().unwrap();
        // Order and repetition should not matter
        let result2 = format!("{}|{}|{}|{}", NAME_2, NAME_3, NAME_1, NAME_2).parse().unwrap();

        assert_eq!(expected, result1);
        assert_eq!(expected, result2);
    }

    #[test]
    fn test_to_string() {
        let env_str = expected();
        let expected = format!("{}|{}|{}", NAME_1, NAME_2, NAME_3);
        assert_eq!(expected, env_str.to_string());
    }

    #[test]
    fn test_serde() {
        let env_str = EnvironmentString(maplit::btreeset! {
            "first".parse().unwrap(),
            "2nd".parse().unwrap(),
            "LAST".parse().unwrap(),
        });
        assert_tokens(&env_str, &[
            Token::Str("2nd|LAST|first")
        ]);
    }

    #[test]
    fn test_iterators() {
        let env_str = expected();

        let expected = vec![
            NAME_1.parse().unwrap(),
            NAME_2.parse().unwrap(),
            NAME_3.parse().unwrap(),
        ];
        let ref_expected: Vec<_> = expected.iter().collect();

        let ref_iter: Vec<_> = env_str.iter().collect();
        assert_eq!(ref_iter, ref_expected);

        let ref_iter: Vec<_> = (&env_str).into_iter().collect();
        assert_eq!(ref_iter, ref_expected);

        let into_iter: Vec<_> = env_str.into_iter().collect();
        assert_eq!(into_iter, expected);
    }

    #[test]
    fn test_from_name() {
        let name = EnvironmentName::from_str("test").unwrap();
        let expected = EnvironmentString(maplit::btreeset! { name.clone() });
        let result = EnvironmentString::from(name);
        assert_eq!(expected, result);
    }

    #[test]
    fn test_insert() {
        let mut env_str: EnvironmentString = "test".parse().unwrap();
        let other: EnvironmentName = "other".parse().unwrap();
        env_str.insert(other.clone());
        assert!(env_str.0.contains(&other));
    }

    #[test]
    fn test_invalid_strings() {
        let inputs = vec![
            ("", Some("")),
            ("|", Some("")),
            ("valid|", Some("")),
            ("|valid", Some("")),
            ("valid|invalid name", Some("invalid name")),
            ("valid|config", Some("config")),
        ];

        for (s, expected) in inputs {
            let error = EnvironmentString::from_str(s).expect_err("input string should be invalid");
            match (expected, &error) {
                (None, Error::EmptyName) => {},
                (None, Error::InvalidName(_)) => panic!("expected Error::EmptyName, got {:?}", error),
                (Some(s), Error::EmptyName) => panic!("expected Error::InvalidName(\"{}\"), got {:?}", s, error),
                (Some(s1), Error::InvalidName(s2)) => assert_eq!(s1, s2, "expected invalid name to be \"{}\", got \"{}\"", s1, s2),
            }
        }
    }
}
