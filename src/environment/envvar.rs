use serde::{Deserialize, Serialize};
use std::convert::{Infallible, TryInto};
use std::fmt;
use std::fmt::Formatter;

#[derive(PartialEq, Debug, Serialize, Deserialize)]
pub struct EnvVariable {
    pub var: String,
    pub expected: Option<String>,
}

impl TryInto<bool> for EnvVariable {
    type Error = Infallible;

    fn try_into(self) -> Result<bool, Self::Error> {
        let EnvVariable { var, expected } = self;
        let result = match std::env::var_os(var) {
            None => false,
            Some(val) => match expected {
                None => true,
                Some(expected_val) => val == expected_val.as_str(),
            },
        };
        Ok(result)
    }
}

// For use in displaying in boolean strings
impl fmt::Display for EnvVariable {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match &self.expected {
            None => write!(f, "ENV ${} IS SET", self.var),
            Some(expected) => write!(f, "ENV ${} == \"{}\"", self.var, expected),
        }
    }
}

#[cfg(all(test, feature = "single-threaded-tests"))]
mod tests {
    use super::*;

    #[test]
    fn test_display_env_no_value() {
        let env = EnvVariable {
            var: "TESTING_VAR".to_string(),
            expected: None,
        };
        assert_eq!("ENV $TESTING_VAR IS SET", env.to_string());
    }

    #[test]
    fn test_display_env_with_value() {
        let env = EnvVariable {
            var: "TESTING_VAR".to_string(),
            expected: Some("testing value".to_string()),
        };
        assert_eq!("ENV $TESTING_VAR == \"testing value\"", env.to_string());
    }

    #[test]
    fn test_env_variable_is_set() {
        for (var, _) in std::env::vars() {
            let is_set: bool = EnvVariable {
                var,
                expected: None,
            }
            .try_into()
            .expect("failed to check environment variable");
            assert!(is_set);
        }
    }

    #[test]
    fn test_env_variable_is_set_to_value() {
        for (var, val) in std::env::vars() {
            let is_set: bool = EnvVariable {
                var,
                expected: Some(val),
            }
            .try_into()
            .expect("failed to check environment variable");
            assert!(is_set);
        }
    }

    #[test]
    fn test_env_variable_is_not_set() {
        for (var, val) in std::env::vars() {
            std::env::remove_var(&var);
            let is_set: bool = EnvVariable {
                var: var.clone(),
                expected: None,
            }
            .try_into()
            .expect("failed to check environment variable");
            std::env::set_var(&var, val);
            assert!(!is_set);
        }
    }

    #[test]
    fn test_env_variable_is_not_set_to_value() {
        for (var, val) in std::env::vars() {
            std::env::set_var(&var, format!("{}_invalid", val));
            let is_set: bool = EnvVariable {
                var,
                expected: Some(val),
            }
            .try_into()
            .expect("failed to check environment variable");
            assert!(!is_set);
        }
    }
}
