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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvCondition {
    Hostname(Combinator<Hostname>),
    #[serde(rename = "os")]
    OperatingSystem(Combinator<OperatingSystem>),
    #[serde(rename = "env")]
    EnvVariable(Combinator<EnvVariable>),
    ExeExists(Combinator<ExeExists>),
    FileExists(Combinator<PathExists>),
}

impl TryInto<bool> for EnvCondition {
    type Error = Error;

    fn try_into(self) -> Result<bool, Self::Error> {
        match self {
            EnvCondition::Hostname(hostname) => hostname.try_into(),
            EnvCondition::OperatingSystem(os) => os.try_into().map_err(Into::into),
            EnvCondition::EnvVariable(env) => env.try_into().map_err(Into::into),
            EnvCondition::ExeExists(exe) => exe.try_into().map_err(Error::ExeExists),
            EnvCondition::FileExists(file) => file.try_into().map_err(Into::into),
        }
    }
}

impl EnvCondition {
    pub fn validate(&self) -> Result<(), Error> {
        match self {
            Self::Hostname(comb) => {
                if comb.is_only_and() || comb.is_complex() {
                    return Err(Error::InvalidCondition {
                        condition_str: comb.to_string(),
                        message: String::from("machines cannot have multiple hostnames at once!"),
                    });
                }
            }
            Self::OperatingSystem(comb) => {
                if comb.is_only_and() || comb.is_complex() {
                    return Err(Error::InvalidCondition {
                        condition_str: comb.to_string(),
                        message: String::from(
                            "machines cannot have multiple operating systems at once!",
                        ),
                    });
                }
            }
            _ => {}
        };

        Ok(())
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
            let condition = EnvCondition::Hostname(combinator);

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
            let condition = EnvCondition::Hostname(combinator);

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

            EnvCondition::Hostname(combinator)
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
            let condition = EnvCondition::OperatingSystem(combinator);

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
            let condition = EnvCondition::OperatingSystem(combinator);

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

            EnvCondition::OperatingSystem(combinator)
                .validate()
                .expect("expecting one of two operating systems should succeed");
        }
    }
}
