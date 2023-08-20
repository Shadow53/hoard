//! Expand environment variables inside of a path.

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
    /// The error returned while creating a [`SystemPath`] using [`PathWithEnv`].
    Path(PathError),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Env {
                error: error @ env::VarError::NotPresent,
                var,
            } => write!(f, "{error}: {var}"),
            // grcov: ignore-start
            // I do not think it is worth testing for this error just to get coverage.
            Self::Env {
                error: error @ env::VarError::NotUnicode(_),
                ..
            } => error.fmt(f),
            // grcov: ignore-end
            Self::Path(error) => write!(f, "{error}"),
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

/// A [`String`] that may contain one or more environment variables to be expanded.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(transparent)]
#[serde(transparent)]
pub struct StringWithEnv(String);

impl From<String> for StringWithEnv {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for StringWithEnv {
    fn from(s: &str) -> Self {
        Self::from(s.to_string())
    }
}

impl fmt::Display for StringWithEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl StringWithEnv {
    /// Replace any environment variables with their associated values.
    ///
    /// # Errors
    ///
    /// See [`Error`]
    pub fn process(self) -> Result<String, Error> {
        let mut new_string = self.0;
        let mut start: usize = 0;
        let mut old_start: usize;

        while let Some(mat) = ENV_REGEX.find(&new_string[start..]) {
            let var = mat.as_str();
            let var = &var[2..var.len() - 1];
            tracing::trace!(var, "found environment variable {}", var,);

            // Error is not logged here because:
            // (a) The context is not terribly important for the error
            // (b) This is used when parsing the configuration file, so there is no
            //     simple way to only parse the paths that apply to this system.
            let value = env::var(var).map_err(|error| Error::Env {
                error,
                var: var.to_string(),
            })?;

            old_start = start;
            start += mat.start() + value.len();
            if start > (new_string.len() + value.len() - mat.as_str().len()) {
                start = new_string.len();
            }

            let range = mat.range();
            // grcov: ignore-start
            tracing::trace!(
                var,
                path = %new_string,
                %value,
                "expanding first instance of variable in path"
            );
            // grcov: ignore-end
            new_string.replace_range(range.start + old_start..range.end + old_start, &value);
            if start >= new_string.len() {
                break;
            }
        }

        Ok(new_string)
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

impl PathWithEnv {
    /// Replace any environment variables with their associated values and attempt to convert
    /// into a [`SystemPath`].
    ///
    /// # Errors
    ///
    /// See [`Error`]
    #[tracing::instrument(level = "debug", name = "process_path_with_env")]
    pub fn process(self) -> Result<SystemPath, Error> {
        let new_path = StringWithEnv(self.0).process()?;

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
        (name: $name:ident, input: $input:literal, env: $var:literal, value: $value:literal, expected: $expected:literal, require_var: $require_var:literal) => {
            #[test]
            fn $name() {
                assert!((!$require_var || ($input).contains(&format!("${{{}}}", $var))), "input string {} doesn't contain variable {}", $input, $var);

                let old_val = std::env::var_os($var);
                std::env::set_var($var, $value);
                let (input, expected): (String, SystemPath) = {
                    #[cfg(windows)]
                    let (i, e) = (format!("C:{}", $input), format!("C:{}", $expected));
                    #[cfg(unix)]
                    let (i, e) = (String::from($input), String::from($expected));
                    (i, SystemPath::try_from(PathBuf::from(e)).unwrap())
                };
                let result = PathWithEnv::from(input).process().unwrap();
                assert_eq!(result, expected);
                if let Some(val) = old_val {
                    std::env::set_var($var, val);
                }
            }
        };
        (name: $name:ident, input: $input:literal, env: $var:literal, value: $value:literal, expected: $expected:literal) => {
            test_env!{ name: $name, input: $input, env: $var, value: $value, expected: $expected, require_var: true }
        };
    }

    test_env! {
        name: var_at_start_shorter_than_value,
        input: "${TEST_HOME}/test/file",
        env: "TEST_HOME",
        value: "/home/testuser",
        expected: "/home/testuser/test/file"
    }

    test_env! {
        name: var_in_middle_shorter_than_value,
        input: "/home/testuser/${TEST_PATH}/file",
        env: "TEST_PATH",
        value: "test/subdir/subberdir",
        expected: "/home/testuser/test/subdir/subberdir/file"
    }

    test_env! {
        name: var_at_end_shorter_than_value,
        input: "/home/testuser/${TEST_PATH}",
        env: "TEST_PATH",
        value: "test/subdir/file",
        expected: "/home/testuser/test/subdir/file"
    }

    // Same length == var name + ${}
    test_env! {
        name: var_at_start_same_length_as_value,
        input: "${TEST_HOME}/test/file",
        env: "TEST_HOME",
        value: "/home/tester",
        expected: "/home/tester/test/file"
    }

    test_env! {
        name: var_in_middle_same_length_as_value,
        input: "/home/testuser/${TEST_PATH}/file",
        env: "TEST_PATH",
        value: "/test/folder",
        expected: "/home/testuser/test/folder/file"
    }

    test_env! {
        name: var_at_end_same_length_as_value,
        input: "/home/testuser/${TEST_PATH}",
        env: "TEST_PATH",
        value: "testing/file",
        expected: "/home/testuser/testing/file"
    }

    test_env! {
        name: var_at_start_longer_than_value,
        input: "${TEST_HOME}/test/file",
        env: "TEST_HOME",
        value: "/home/test",
        expected: "/home/test/test/file"
    }

    test_env! {
        name: var_in_middle_longer_than_value,
        input: "/home/testuser/${TEST_PATH}/file",
        env: "TEST_PATH",
        value: "test/dir",
        expected: "/home/testuser/test/dir/file"
    }

    test_env! {
        name: var_at_end_longer_than_value,
        input: "/home/testuser/${TEST_PATH}",
        env: "TEST_PATH",
        value: "a/file",
        expected: "/home/testuser/a/file"
    }

    test_env! {
        name: path_without_var_stays_same,
        input: "/path/without/variables",
        env: "UNUSED",
        value: "NOTHING",
        expected: "/path/without/variables",
        require_var: false
    }

    test_env! {
        name: path_with_two_variables,
        input: "/home/${TEST_USER}/somedir/${TEST_USER}/file",
        env: "TEST_USER",
        value: "testuser",
        expected: "/home/testuser/somedir/testuser/file"
    }

    test_env! {
        name: var_without_braces_not_expanded,
        input: "/path/with/$INVALID/variable",
        env: "INVALID",
        value: "broken",
        expected: "/path/with/$INVALID/variable",
        require_var: false
    }

    test_env! {
        name: var_windows_style_not_expanded,
        input: "/path/with/%INVALID%/variable",
        env: "INVALID",
        value: "broken",
        expected: "/path/with/%INVALID%/variable",
        require_var: false
    }

    test_env! {
        name: vars_not_recursively_expanded,
        input: "/${TEST_HOME}",
        env: "TEST_HOME",
        value: "${HOME}",
        expected: "/${HOME}"
    }

    test_env! {
        name: var_inside_var,
        input: "/test/${WRAPPING${TEST_VAR}VARIABLE}/test",
        env: "TEST_VAR",
        value: "_",
        expected: "/test/${WRAPPING_VARIABLE}/test"
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
