//! Newtypes used to enforce invariants throughout this library.
//!
//! - Names (`*Name`) must contain only alphanumeric characters, dash (`-`), or underscore (`_`).
//! - [`EnvironmentString`] has its own requirements.

use std::{ops::Deref, str::FromStr, collections::BTreeSet, fmt};
use serde::{Serialize, Deserialize, Deserializer};
use serde::de::Error as _;
use thiserror::Error;

/// Errors that may occur while creating an instance of one of this newtypes.
#[derive(Debug, Error)]
pub enum Error {
    /// The given string is not a valid name (alphanumeric).
    #[error("invalid name: \"{0}\": must contain only alphanumeric characters")]
    InvalidName(String)
}

const DISALLOWED_NAMES: [&str; 2] = ["", "config"];

fn validate_name(name: String) -> Result<String, Error> {
    if name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') && DISALLOWED_NAMES.iter().all(|disallowed| &name != disallowed) {
        Ok(name)
    } else {
        Err(Error::InvalidName(name))
    }
}

// PILE NAME -- START

/// Newtype wrapper for `Option<String>` representing a pile name.
///
/// - `None` indicates an anonymous (unnamed) pile.
/// - `Some(name)` indicates a named pile with name `name`.
///
/// See the [module documentation](self) for what makes an acceptable name.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct PileName(Option<String>);

impl FromStr for PileName {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        validate_name(value.to_string()).map(Some).map(Self)
    }
}

impl<'de> Deserialize<'de> for PileName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let inner = String::deserialize(deserializer)?;
        inner.parse().map_err(D::Error::custom)
    }
}

impl<T> TryFrom<Option<T>> for PileName where T: AsRef<str> {
    type Error = Error;

    fn try_from(value: Option<T>) -> Result<Self, Self::Error> {
        match value {
            None => Ok(Self(None)),
            Some(inner) => inner.as_ref().parse(),
        }
    }
}

impl Deref for PileName {
    type Target = Option<String>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for PileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.as_deref() {
            None => write!(f, ""),
            Some(name) => write!(f, "{}", name),
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
    pub fn is_anonymous(&self) -> bool { self.0.is_none() }
}

// PILE NAME -- END
// HOARD NAME -- START

/// Newtype wrapper for `String` representing a hoard name.
///
/// See the [module documentation](self) for what makes an acceptable name.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[repr(transparent)]
pub struct HoardName(String);

impl FromStr for HoardName {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        validate_name(s.to_string()).map(Self)
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
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let inner = String::deserialize(deserializer)?;
        inner.parse().map_err(D::Error::custom)
    }
}

// HOARD NAME -- END
// ENVIRONMENT NAME -- START

/// Newtype wrapper for `String` representing an [environment](https://hoard.rs/config/envs.html).
///
/// See the [module documentation](self) for what makes an acceptable name.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[repr(transparent)]
pub struct EnvironmentName(String);

impl FromStr for EnvironmentName {
    type Err = Error;
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
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let inner = String::deserialize(deserializer)?;
        inner.parse().map_err(D::Error::custom)
    }
}

// ENVIRONMENT NAME -- END
// ENVIRONMENT STRING -- START

/// Newtype wrapper for `HashSet<EnvironmentName>` representing a list of environments.
///
///
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
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
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let inner = String::deserialize(deserializer)?;
        inner.parse().map_err(D::Error::custom)
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
    /// Returns an [`EnvironmentString`] containing the [`EnvironmentName`]s of both `self` and `other`.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        let new = self.0.union(&other.0).cloned().collect();
        Self(new)
    }

    /// Inserts the given [`EnvironmentName`] into `self`.
    pub fn insert(&mut self, name: EnvironmentName) {
        self.0.insert(name);
    }

    /// Iterates over the [`EnvironmentName`]s in this `EnvironmentString`.
    pub fn iter(&self) -> std::collections::btree_set::Iter<'_, EnvironmentName> {
        self.into_iter()
    }
}

// ENVIRONMENT STRING -- END
