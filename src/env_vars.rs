//! Expand environment variables inside of a path.
//!
//! The only function exported from this module is [`expand_env_in_path`].

use crate::paths::{Error as PathError, SystemPath};
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::{env, fmt};

// Following the example of `std::env::set_var`, the only things disallowed are
// the equals sign and the NUL character.
//
// The `+?` is non-greedy matching, which is necessary for if there are multiple variables.
static ENV_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\$\{[^(=|\x{0}|$)]+?}"#).expect("failed to compile regular expression")
});

/// An error that may occur during expansion.
///
#[derive(Debug)]
pub enum Error {
    /// Environment variable was not set.
    ///
    /// This is a wrapper for [`std::env::VarError`] that shows what environment variable
    /// could not be found.
    Env {
        /// The error that occurred.
        error: env::VarError,
        /// The variable that caused the error.
        var: String,
    },
    /// The error returned while creating a [`SystemPath`] using [`expand_env_in_path`].
    Path(PathError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Env {
                error: error @ env::VarError::NotPresent,
                var,
            } => write!(f, "{}: {}", error, var),
            // grcov: ignore-start
            // I do not think it is worth testing for this error just to get coverage.
            Self::Env {
                error: error @ env::VarError::NotUnicode(_),
                ..
            } => error.fmt(f),
            // grcov: ignore-end
            Self::Path(error) => write!(f, "{}", error),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match &self {
            Error::Env { error, .. } => Some(error),
            Error::Path(error) => Some(error),
        }
    }
}

/// A [`String`] representing a path that may contain one or more environment variables to be
/// expanded.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct PathWithEnv(String);

impl From<String> for PathWithEnv {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for PathWithEnv {
    fn from(s: &str) -> Self {
        Self::from(s.to_string())
    }
}

impl fmt::Display for PathWithEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Takes the input string, expands all environment variables, and returns the
/// expanded string as a [`PathBuf`].
///
/// # Example
///
/// ```
/// use std::path::PathBuf;
/// use hoard::env_vars::PathWithEnv;
/// use hoard::paths::SystemPath;
///
/// #[cfg(unix)]
/// let template = "/some/${CUSTOM_VAR}/path";
/// #[cfg(windows)]
/// let template = "C:/some/${CUSTOM_VAR}/path";
/// std::env::set_var("CUSTOM_VAR", "foobar");
/// let path = PathWithEnv::from(template)
///     .process()
///     .expect("failed to expand path");
/// let expected = SystemPath::try_from(PathBuf::from("/some/foobar/path")).unwrap();
/// assert_eq!(path, expected);
/// ```
///
/// # Errors
///
/// - Any [`VarError`](env::VarError) from looking up the environment variable's value.
impl PathWithEnv {
    /// Replace any environment variables with their associated values and attempt to convert
    /// into a [`SystemPath`].
    ///
    /// # Errors
    ///
    /// See [`Error`]
    pub fn process(self) -> Result<SystemPath, Error> {
        let mut new_path = self.0;
        let mut start: usize = 0;
        let mut old_start: usize;

        let _span = tracing::debug_span!("expand_env_in_path", path=%new_path).entered();

        while let Some(mat) = ENV_REGEX.find(&new_path[start..]) {
            let var = mat.as_str();
            let var = &var[2..var.len() - 1];
            tracing::trace!(var, "found environment variable {}", var,);

            let value = env::var(var).map_err(|error| Error::Env {
                error,
                var: var.to_string(),
            })?;

            old_start = start;
            start += mat.start() + value.len();
            if start > (new_path.len() + value.len() - mat.as_str().len()) {
                start = new_path.len();
            }

            let range = mat.range();
            // grcov: ignore-start
            tracing::trace!(
                var,
                path = %new_path,
                %value,
                "expanding first instance of variable in path"
            );
            // grcov: ignore-end
            new_path.replace_range(range.start + old_start..range.end + old_start, &value);
            if start >= new_path.len() {
                break;
            }
        }

        // Splitting into components and collecting will collapse multiple separators.
        SystemPath::try_from(PathBuf::from(new_path).components().collect::<PathBuf>())
            .map_err(Error::Path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    macro_rules! test_env {
        (name: $name:ident, input: $input:literal, env: $var:literal, value: $value:literal, expected: $expected:expr, require_var: $require_var:literal) => {
            #[test]
            fn $name() {
                assert!((!$require_var || ($input).contains(&format!("${{{}}}", $var))), "input string {} doesn't contain variable {}", $input, $var);

                let old_val = std::env::var_os($var);
                std::env::set_var($var, $value);
                let expected: SystemPath = $expected;
                let result = PathWithEnv::from($input).process().unwrap();
                assert_eq!(result, expected);
                if let Some(val) = old_val {
                    std::env::set_var($var, val);
                }
            }
        };
        (name: $name:ident, input: $input:literal, env: $var:literal, value: $value:literal, expected: $expected:expr) => {
            test_env!{ name: $name, input: $input, env: $var, value: $value, expected: $expected, require_var: true }
        };
    }

    test_env! {
        name: var_at_start_shorter_than_value,
        input: "${TEST_HOME}/test/file",
        env: "TEST_HOME",
        value: "/home/testuser",
        expected: SystemPath::try_from(PathBuf::from("/home/testuser/test/file")).unwrap()
    }

    test_env! {
        name: var_in_middle_shorter_than_value,
        input: "/home/testuser/${TEST_PATH}/file",
        env: "TEST_PATH",
        value: "test/subdir/subberdir",
        expected: SystemPath::try_from(PathBuf::from("/home/testuser/test/subdir/subberdir/file")).unwrap()
    }

    test_env! {
        name: var_at_end_shorter_than_value,
        input: "/home/testuser/${TEST_PATH}",
        env: "TEST_PATH",
        value: "test/subdir/file",
        expected: SystemPath::try_from(PathBuf::from("/home/testuser/test/subdir/file")).unwrap()
    }

    // Same length == var name + ${}
    test_env! {
        name: var_at_start_same_length_as_value,
        input: "${TEST_HOME}/test/file",
        env: "TEST_HOME",
        value: "/home/tester",
        expected: SystemPath::try_from(PathBuf::from("/home/tester/test/file")).unwrap()
    }

    test_env! {
        name: var_in_middle_same_length_as_value,
        input: "/home/testuser/${TEST_PATH}/file",
        env: "TEST_PATH",
        value: "/test/folder",
        expected: SystemPath::try_from(PathBuf::from("/home/testuser/test/folder/file")).unwrap()
    }

    test_env! {
        name: var_at_end_same_length_as_value,
        input: "/home/testuser/${TEST_PATH}",
        env: "TEST_PATH",
        value: "testing/file",
        expected: SystemPath::try_from(PathBuf::from("/home/testuser/testing/file")).unwrap()
    }

    test_env! {
        name: var_at_start_longer_than_value,
        input: "${TEST_HOME}/test/file",
        env: "TEST_HOME",
        value: "/home/test",
        expected: SystemPath::try_from(PathBuf::from("/home/test/test/file")).unwrap()
    }

    test_env! {
        name: var_in_middle_longer_than_value,
        input: "/home/testuser/${TEST_PATH}/file",
        env: "TEST_PATH",
        value: "test/dir",
        expected: SystemPath::try_from(PathBuf::from("/home/testuser/test/dir/file")).unwrap()
    }

    test_env! {
        name: var_at_end_longer_than_value,
        input: "/home/testuser/${TEST_PATH}",
        env: "TEST_PATH",
        value: "a/file",
        expected: SystemPath::try_from(PathBuf::from("/home/testuser/a/file")).unwrap()
    }

    test_env! {
        name: path_without_var_stays_same,
        input: "/path/without/variables",
        env: "UNUSED",
        value: "NOTHING",
        expected: SystemPath::try_from(PathBuf::from("/path/without/variables")).unwrap(),
        require_var: false
    }

    test_env! {
        name: path_with_two_variables,
        input: "/home/${TEST_USER}/somedir/${TEST_USER}/file",
        env: "TEST_USER",
        value: "testuser",
        expected: SystemPath::try_from(PathBuf::from("/home/testuser/somedir/testuser/file")).unwrap()
    }

    test_env! {
        name: var_without_braces_not_expanded,
        input: "/path/with/$INVALID/variable",
        env: "INVALID",
        value: "broken",
        expected: SystemPath::try_from(PathBuf::from("/path/with/$INVALID/variable")).unwrap(),
        require_var: false
    }

    test_env! {
        name: var_windows_style_not_expanded,
        input: "/path/with/%INVALID%/variable",
        env: "INVALID",
        value: "broken",
        expected: SystemPath::try_from(PathBuf::from("/path/with/%INVALID%/variable")).unwrap(),
        require_var: false
    }

    test_env! {
        name: vars_not_recursively_expanded,
        input: "/${TEST_HOME}",
        env: "TEST_HOME",
        value: "${HOME}",
        expected: SystemPath::try_from(PathBuf::from("/${HOME}")).unwrap()
    }

    test_env! {
        name: var_inside_var,
        input: "/test/${WRAPPING${TEST_VAR}VARIABLE}/test",
        env: "TEST_VAR",
        value: "_",
        expected: SystemPath::try_from(PathBuf::from("/test/${WRAPPING_VARIABLE}/test")).unwrap()
    }

    #[test]
    fn test_error_traits() {
        let env_error = env::var("DOESNOTEXIST").expect_err("variable should not exist");
        let error = Error::Env {
            error: env_error,
            var: "DOESNOTEXIST".to_string(),
        };
        assert!(error.to_string().contains("DOESNOTEXIST"));
        assert!(error.source().is_some());
    }
}
