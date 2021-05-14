pub mod envvar;
pub mod exe;
pub mod hostname;
pub mod os;
pub mod path;

use crate::combinator::Combinator;
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use self::envvar::EnvVariable;
pub use self::exe::ExeExists;
pub use self::hostname::Hostname;
pub use self::os::OperatingSystem;
pub use self::path::PathExists;
use std::convert::{Infallible, TryInto};

#[derive(Debug, Error)]
pub enum Error {
    #[error("failed to detect hostname: {0}")]
    Hostname(#[from] std::io::Error),
    #[error("failed to detect if exe exists in path: {0}")]
    ExeExists(#[from] which::Error),
    #[error("condition {condition_str} is invalid: {message}")]
    InvalidCondition {
        condition_str: String,
        message: String,
    },
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unimplemented!("this should never happen");
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Hash)]
pub struct Environment {
    hostname: Option<Combinator<Hostname>>,
    os: Option<Combinator<OperatingSystem>>,
    env: Option<Combinator<EnvVariable>>,
    exe_exists: Option<Combinator<ExeExists>>,
    path_exists: Option<Combinator<PathExists>>,
}

// Note to self: this is a good candidate for a derive macro
// if Combinator is put into its own library
impl TryInto<bool> for Environment {
    type Error = Error;

    fn try_into(self) -> Result<bool, Self::Error> {
        let Environment {
            hostname,
            os,
            env,
            exe_exists,
            path_exists,
        } = self;

        let hostname_cond: bool = hostname.map(TryInto::try_into).unwrap_or(Ok(true))?;
        let os_cond: bool = os.map(TryInto::try_into).unwrap_or(Ok(true))?;
        let env_cond: bool = env.map(TryInto::try_into).unwrap_or(Ok(true))?;
        let exe_cond: bool = exe_exists.map(TryInto::try_into).unwrap_or(Ok(true))?;
        let path_cond: bool = path_exists.map(TryInto::try_into).unwrap_or(Ok(true))?;

        Ok(hostname_cond && os_cond && env_cond && exe_cond && path_cond)
    }
}

impl Environment {
    pub fn validate(&self) -> Result<(), Error> {
        let Environment { hostname, os, .. } = self;
        if let Some(comb) = hostname {
            if comb.is_only_and() || comb.is_complex() {
                return Err(Error::InvalidCondition {
                    condition_str: comb.to_string(),
                    message: String::from("machines cannot have multiple hostnames at once!"),
                });
            }
        }

        if let Some(comb) = os {
            if comb.is_only_and() || comb.is_complex() {
                return Err(Error::InvalidCondition {
                    condition_str: comb.to_string(),
                    message: String::from(
                        "machines cannot have multiple operating systems at once!",
                    ),
                });
            }
        }

        Ok(())
    }
}

impl Default for Environment {
    fn default() -> Self {
        Environment {
            hostname: None,
            os: None,
            env: None,
            exe_exists: None,
            path_exists: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combinator::CombinatorInner;

    mod validate_hostname {
        use super::*;

        #[test]
        fn test_env_condition_invalid_hostname_only_and() {
            let combinator = Combinator(vec![CombinatorInner::Multiple(vec![
                Hostname("hostname.one".to_string()),
                Hostname("hostname.two".to_string()),
            ])]);

            let condition = Environment {
                hostname: Some(combinator),
                ..Default::default()
            };

            let err = condition
                .validate()
                .expect_err("expecting two hostnames at the same time should fail");
            match err {
                Error::InvalidCondition { .. } => {}
                err => panic!("unexpected error: {}", err),
            }
        }

        #[test]
        fn test_env_condition_invalid_hostname_complex() {
            let combinator = Combinator(vec![
                CombinatorInner::Single(Hostname("hostname.single".to_string())),
                CombinatorInner::Multiple(vec![
                    Hostname("hostname.one".to_string()),
                    Hostname("hostname.two".to_string()),
                ]),
            ]);

            let condition = Environment {
                hostname: Some(combinator),
                ..Default::default()
            };

            let err = condition
                .validate()
                .expect_err("expecting two hostnames at the same time should fail");
            match err {
                Error::InvalidCondition { .. } => {}
                err => panic!("unexpected error: {}", err),
            }
        }

        #[test]
        fn test_env_condition_valid_hostname() {
            let combinator = Combinator(vec![
                CombinatorInner::Single(Hostname("hostname.one".to_string())),
                CombinatorInner::Single(Hostname("hostname.two".to_string())),
            ]);

            let condition = Environment {
                hostname: Some(combinator),
                ..Default::default()
            };

            condition
                .validate()
                .expect("expecting one of two hostnames should succeed");
        }
    }

    mod validate_operating_system {
        use super::*;

        #[test]
        fn test_env_condition_invalid_os_only_and() {
            let combinator = Combinator(vec![CombinatorInner::Multiple(vec![
                OperatingSystem("windows".to_string()),
                OperatingSystem("linux".to_string()),
            ])]);

            let condition = Environment {
                os: Some(combinator),
                ..Default::default()
            };

            let err = condition
                .validate()
                .expect_err("expecting two operating systems at the same time should fail");
            match err {
                Error::InvalidCondition { .. } => {}
                err => panic!("unexpected error: {}", err),
            }
        }

        #[test]
        fn test_env_condition_invalid_os_complex() {
            let combinator = Combinator(vec![
                CombinatorInner::Single(OperatingSystem("macos".to_string())),
                CombinatorInner::Multiple(vec![
                    OperatingSystem("windows".to_string()),
                    OperatingSystem("linux".to_string()),
                ]),
            ]);

            let condition = Environment {
                os: Some(combinator),
                ..Default::default()
            };

            let err = condition
                .validate()
                .expect_err("expecting two operating systems at the same time should fail");
            match err {
                Error::InvalidCondition { .. } => {}
                err => panic!("unexpected error: {}", err),
            }
        }

        #[test]
        fn test_env_condition_valid_os() {
            let combinator = Combinator(vec![
                CombinatorInner::Single(OperatingSystem("windows".to_string())),
                CombinatorInner::Single(OperatingSystem("linux".to_string())),
            ]);

            let condition = Environment {
                os: Some(combinator),
                ..Default::default()
            };

            condition
                .validate()
                .expect("expecting one of two operating systems should succeed");
        }
    }
}
