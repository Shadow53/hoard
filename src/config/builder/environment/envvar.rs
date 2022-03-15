//! See [`EnvVariable`].

use serde::{Deserialize, Serialize};
use std::convert::{Infallible, TryInto};
use std::fmt;
use std::fmt::Formatter;

/// A conditional structure that checks if the given environment variable exists and optionally if
/// it is set to a specific value.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Hash)]
pub struct EnvVariable {
    /// The variable to check.
    pub var: String,
    /// The expected value to check against. If `None`, this matches any value.
    pub expected: Option<String>,
}

impl TryInto<bool> for EnvVariable {
    type Error = Infallible;

    fn try_into(self) -> Result<bool, Self::Error> {
        let EnvVariable { var, expected } = self;
        tracing::trace!(%var, "checking if environment variable exists");
        let result = match std::env::var_os(&var) {
            None => false,
            Some(val) => match expected {
                None => true,
                Some(expected) => {
                    tracing::trace!(%var, %expected, "checking if variable matches expected value");
                    val == expected.as_str()
                }
            },
        };
        Ok(result)
    }
}

// For use in displaying in boolean strings
impl fmt::Display for EnvVariable {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.expected {
            None => write!(f, "ENV ${{{}}} IS SET", self.var),
            Some(expected) => write!(f, "ENV ${{{}}} == \"{}\"", self.var, expected),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_env_no_value() {
        let env = EnvVariable {
            var: "TESTING_VAR".to_string(),
            expected: None,
        };
        assert_eq!("ENV ${TESTING_VAR} IS SET", env.to_string());
    }

    #[test]
    fn test_display_env_with_value() {
        let env = EnvVariable {
            var: "TESTING_VAR".to_string(),
            expected: Some("testing value".to_string()),
        };
        assert_eq!("ENV ${TESTING_VAR} == \"testing value\"", env.to_string());
    }

    #[test]
    fn test_env_variable_is_set() {
        let var = String::from("HOARD_ENV_IS_SET");
        std::env::set_var(&var, "true");
        let is_set: bool = EnvVariable {
            var,
            expected: None,
        }
        .try_into()
        .expect("failed to check environment variable");
        assert!(is_set);
    }

    #[test]
    fn test_env_variable_is_set_to_value() {
        let var = String::from("HOARD_ENV_IS_SET_TO");
        let value = String::from("set to this");
        std::env::set_var(&var, &value);
        let is_set: bool = EnvVariable {
            var,
            expected: Some(value),
        }
        .try_into()
        .expect("failed to check environment variable");
        assert!(is_set);
    }

    #[test]
    fn test_env_variable_is_not_set() {
        let var = String::from("HOARD_ENV_NOT_SET");
        assert!(std::env::var_os(&var).is_none(), "env var {} should not be set", var);
        let is_set: bool = EnvVariable {
            var,
            expected: None,
        }
        .try_into()
        .expect("failed to check environment variable");
        assert!(!is_set);
    }

    #[test]
    fn test_env_variable_is_not_set_to_value() {
        let var= String::from("HOARD_ENV_WRONG_VALUE");
        std::env::set_var(&var, "unexpected value");
        let is_set: bool = EnvVariable {
            var,
            expected: Some(String::from("wrong value")),
        }
        .try_into()
        .expect("failed to check environment variable");
        assert!(!is_set);
    }
}
