//! Newtypes used to enforce invariants throughout this library.
//!
//! - Names (`*Name`) must contain only alphanumeric characters, dash (`-`), or underscore (`_`).
//! - [`EnvironmentString`] has its own requirements.

use thiserror::Error;

pub use environment_name::EnvironmentName;
pub use environment_string::EnvironmentString;
pub use hoard_name::HoardName;
pub use non_empty_pile_name::NonEmptyPileName;
pub use pile_name::PileName;

mod environment_name;
mod environment_string;
mod hoard_name;
mod non_empty_pile_name;
mod pile_name;

/// Errors that may occur while creating an instance of one of this newtypes.
#[derive(Debug, Error, PartialEq)]
pub enum Error {
    /// The given string contains disallowed characters.
    #[error("invalid name \"{0}\": must contain only alphanumeric characters, '-', '_', or '.'")]
    DisallowedCharacters(String),
    /// The given string is a disallowed name.
    #[error("name \"{0}\" is not allowed")]
    DisallowedName(String),
    /// The given string was empty, which is not allowed.
    #[error("name cannot be empty (null, None, or the empty string)")]
    EmptyName,
}

const DISALLOWED_NAMES: [&str; 2] = ["", "config"];

#[tracing::instrument(level = "trace")]
fn validate_name(name: String) -> Result<String, Error> {
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return crate::create_log_error(Error::DisallowedCharacters(name));
    }

    if DISALLOWED_NAMES
        .iter()
        .any(|disallowed| &name == disallowed) {
        return crate::create_log_error(Error::DisallowedName(name));
    }

    Ok(name)
}
