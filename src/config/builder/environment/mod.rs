//! Environment definitions. For more, see [`Environment`].

use std::convert::{Infallible, TryInto};
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::combinator::Combinator;

pub use self::envvar::EnvVariable;
pub use self::exe::ExeExists;
pub use self::hostname::Hostname;
pub use self::os::OperatingSystem;
pub use self::path::PathExists;

pub mod envvar;
pub mod exe;
pub mod hostname;
pub mod os;
pub mod path;

/// Errors that may occur while evaluating an [`Environment`].
#[derive(Debug, Error)]
#[allow(variant_size_differences)]
pub enum Error {
    /// An error that occurred while determining the system hostname.
    #[error("failed to detect hostname: {0}")]
    Hostname(#[from] tokio::io::Error),
    /// An error that occurred while checking if a program exists in `$PATH`.
    #[error("failed to detect if exe exists in path: {0}")]
    ExeExists(#[from] <ExeExists as TryInto<bool>>::Error),
    /// A condition string is invalid. The `message` should indicate why.
    #[error("condition {condition_str} is invalid: {message}")]
    InvalidCondition {
        /// The invalid condition string.
        condition_str: String,
        /// A message indicating why the condition is invalid.
        message: String,
    },
}

// grcov: ignore-start
impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unimplemented!("this should never happen");
    }
}
// grcov: ignore-end

/// A combination of conditions that make up a single Environment.
///
/// # Example (TOML)
///
/// ```toml
/// [envs.first_env]
///     # [Hostname][`Hostname`] must match one of the items in the list.
///     hostname = ["localhost", "localhost.localdomain", "first.env"]
///     # The [operating system][`OperatingSystem`] must match one of the items in the list.
///     os = ["linux", "macos", "freebsd"]
///     # Either `vim`, `nvim`, or both `vi` and `nano` must exist on the system.
///     exe_exists = ["vim", "nvim", ["vi", "nano"]]
///     # Both the `Music` and `Videos` folder must exist in user shadow53's home directory.
///     path_exists = [["/home/shadow53/Music", "/home/shadow53/Videos"]]
/// ```
///
/// See the documentation for the following types for more how these items are interpreted.
///
/// - [`Combinator<T>`]
/// - [`EnvVariable`]
/// - [`ExeExists`]
/// - [`Hostname`]
/// - [`OperatingSystem`]
/// - [`PathExists`]
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(deny_unknown_fields)]
pub struct Environment {
    hostname: Option<Combinator<Hostname>>,
    os: Option<Combinator<OperatingSystem>>,
    env: Option<Combinator<EnvVariable>>,
    exe_exists: Option<Combinator<ExeExists>>,
    path_exists: Option<Combinator<PathExists>>,
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut has_previous = false;

        if let Some(hostname) = &self.hostname {
            has_previous = true;
            write!(f, "({})", hostname)?;
        }

        if let Some(os) = &self.os {
            if has_previous {
                write!(f, " AND ")?;
            }
            has_previous = true;
            write!(f, "({})", os)?;
        }

        if let Some(env) = &self.env {
            if has_previous {
                write!(f, " AND ")?;
            }
            has_previous = true;
            write!(f, "({})", env)?;
        }

        if let Some(exe_exists) = &self.exe_exists {
            if has_previous {
                write!(f, " AND ")?;
            }
            has_previous = true;
            write!(f, "({})", exe_exists)?;
        }

        if let Some(path_exists) = &self.path_exists {
            if has_previous {
                write!(f, " AND ")?;
            }
            write!(f, "({})", path_exists)?;
        }

        Ok(())
    }
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

        let hostname_cond: bool = hostname.map_or(Ok(true), TryInto::try_into)?;
        let os_cond: bool = os.map_or(Ok(true), TryInto::try_into)?;
        let env_cond: bool = env.map_or(Ok(true), TryInto::try_into)?;
        let exe_cond: bool = exe_exists.map_or(Ok(true), TryInto::try_into)?;
        let path_cond: bool = path_exists.map_or(Ok(true), TryInto::try_into)?;

        Ok(hostname_cond && os_cond && env_cond && exe_cond && path_cond)
    }
}

impl Environment {
    /// Checks that there are no invalid or impossible conditions set.
    ///
    /// # Errors
    ///
    /// [`Error::InvalidCondition`]
    #[tracing::instrument(name = "validate_environment")]
    pub fn validate(&self) -> Result<(), Error> {
        let Environment { hostname, os, .. } = self;
        if let Some(comb) = hostname {
            if comb.is_only_and() || comb.is_complex() {
                return crate::create_log_error(Error::InvalidCondition {
                    condition_str: comb.to_string(),
                    message: String::from("machines cannot have multiple hostnames at once!"),
                });
            }
        }

        if let Some(comb) = os {
            if comb.is_only_and() || comb.is_complex() {
                return crate::create_log_error(Error::InvalidCondition {
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

#[cfg(test)]
mod tests {
    use crate::combinator::Inner;

    use super::*;

    mod display {
        use std::path::PathBuf;

        use crate::paths::SystemPath;

        use super::*;

        #[test]
        fn test_display_with_none() {
            assert_eq!(Environment::default().to_string(), String::new());
        }

        #[test]
        fn test_display_with_all() {
            let hostname = Hostname("hostname.one".into());
            let os = OperatingSystem("linux".into());
            let env_var = EnvVariable {
                var: "TEST_VARIABLE".to_string(),
                expected: None,
            };
            let exe_exists = ExeExists(PathBuf::from("test").try_into().unwrap());
            let path_exists = PathExists(Some(
                #[cfg(unix)]
                SystemPath::try_from(PathBuf::from("/test/path")).unwrap(),
                #[cfg(windows)]
                SystemPath::try_from(PathBuf::from("C:\\test\\path")).unwrap(),
            ));

            let env = Environment {
                hostname: Some(Combinator(vec![Inner::Single(hostname.clone())])),
                os: Some(Combinator(vec![Inner::Single(os.clone())])),
                env: Some(Combinator(vec![Inner::Single(env_var.clone())])),
                exe_exists: Some(Combinator(vec![Inner::Single(exe_exists.clone())])),
                path_exists: Some(Combinator(vec![Inner::Single(path_exists.clone())])),
            };

            let expected = vec![
                format!("({})", hostname),
                format!("({})", os),
                format!("({})", env_var),
                format!("({})", exe_exists),
                format!("({})", path_exists),
            ]
            .join(" AND ");

            assert_eq!(env.to_string(), expected);
        }
    }

    mod validate_hostname {
        use super::*;

        #[test]
        fn test_env_condition_invalid_hostname_only_and() {
            let combinator = Combinator(vec![Inner::Multiple(vec![
                Hostname("hostname.one".to_string()),
                Hostname("hostname.two".to_string()),
            ])]);

            let condition = Environment {
                hostname: Some(combinator),
                ..Environment::default()
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
                Inner::Single(Hostname("hostname.single".to_string())),
                Inner::Multiple(vec![
                    Hostname("hostname.one".to_string()),
                    Hostname("hostname.two".to_string()),
                ]),
            ]);

            let condition = Environment {
                hostname: Some(combinator),
                ..Environment::default()
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
                Inner::Single(Hostname("hostname.one".to_string())),
                Inner::Single(Hostname("hostname.two".to_string())),
            ]);

            let condition = Environment {
                hostname: Some(combinator),
                ..Environment::default()
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
            let combinator = Combinator(vec![Inner::Multiple(vec![
                OperatingSystem("windows".to_string()),
                OperatingSystem("linux".to_string()),
            ])]);

            let condition = Environment {
                os: Some(combinator),
                ..Environment::default()
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
                Inner::Single(OperatingSystem("macos".to_string())),
                Inner::Multiple(vec![
                    OperatingSystem("windows".to_string()),
                    OperatingSystem("linux".to_string()),
                ]),
            ]);

            let condition = Environment {
                os: Some(combinator),
                ..Environment::default()
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
                Inner::Single(OperatingSystem("windows".to_string())),
                Inner::Single(OperatingSystem("linux".to_string())),
            ]);

            let condition = Environment {
                os: Some(combinator),
                ..Environment::default()
            };

            condition
                .validate()
                .expect("expecting one of two operating systems should succeed");
        }
    }
}
