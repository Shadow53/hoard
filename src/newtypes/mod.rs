//! Newtypes used to enforce invariants throughout this library.
//!
//! - Names (`*Name`) must contain only alphanumeric characters, dash (`-`), or underscore (`_`).
//! - [`EnvironmentString`] has its own requirements.

use thiserror::Error;

mod environment_name;
mod environment_string;
mod hoard_name;
mod non_empty_pile_name;
mod pile_name;

pub use environment_name::EnvironmentName;
pub use environment_string::EnvironmentString;
pub use hoard_name::HoardName;
pub use non_empty_pile_name::NonEmptyPileName;
pub use pile_name::PileName;

/// Errors that may occur while creating an instance of one of this newtypes.
#[derive(Debug, Error, PartialEq)]
pub enum Error {
    /// The given string is not a valid name (alphanumeric).
    #[error("invalid name: \"{0}\": must contain only alphanumeric characters")]
    InvalidName(String),
    /// The given string was empty, which is not allowed.
    #[error("name cannot be empty (null, None, or the empty string)")]
    EmptyName,
}

const DISALLOWED_NAMES: [&str; 2] = ["", "config"];

fn validate_name(name: String) -> Result<String, Error> {
    if name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        && DISALLOWED_NAMES
            .iter()
            .all(|disallowed| &name != disallowed)
    {
        Ok(name)
    } else {
        Err(Error::InvalidName(name))
    }
}
