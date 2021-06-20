//! Expand environment variables inside of a path.
//!
//! The only function exported from this module is [`expand_env_in_path`].

use once_cell::sync::Lazy;
use regex::Regex;
use std::env;
use std::path::PathBuf;

// Following the example of `std::env::set_var`, the only things disallowed are
// the equals sign and the NUL character.
//
// The `+?` is non-greedy matching, which is necessary for if there are multiple variables.
static ENV_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"\$\{[^(=|\x{0}|$)]+?}"#).expect("failed to compile regular expression")
});

/// Takes the input string, expands all environment variables, and returns the
/// expanded string as a [`PathBuf`].
///
/// # Example
///
/// ```
/// use hoard::env_vars::expand_env_in_path;
/// use std::path::PathBuf;
///
/// let template = "/some/${CUSTOM_VAR}/path";
/// std::env::set_var("CUSTOM_VAR", "foobar");
/// let path = expand_env_in_path(template)
///     .expect("failed to expand path");
/// assert_eq!(path, PathBuf::from("/some/foobar/path"));
/// ```
///
/// # Errors
///
/// - Any [`VarError`](env::VarError) from looking up the environment variable's value.
pub fn expand_env_in_path(path: &str) -> Result<PathBuf, env::VarError> {
    let mut new_path = path.to_owned();
    let mut start: usize = 0;
    let mut old_start: usize;

    let _span = tracing::debug_span!("expand_env_in_path", %path).entered();

    while let Some(mat) = ENV_REGEX.find(&new_path[start..]) {
        let var = mat.as_str();
        let var = &var[2..var.len() - 1];
        tracing::trace!(var, "found environment variable {}", var,);
        let value = env::var(var)?;

        old_start = start;
        start += value.len();
        if start > (new_path.len() + value.len() - mat.as_str().len()) {
            start = new_path.len();
        }

        let range = mat.range();
        tracing::trace!(
            var,
            path = %new_path,
            %value,
            "expanding first instance of variable in path"
        );
        new_path.replace_range(range.start + old_start..range.end + old_start, &value);
        if start >= new_path.len() {
            break;
        }
    }

    // Splitting into components and collecting will collapse multiple separators.
    Ok(PathBuf::from(new_path).components().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test_env {
        (name: $name:ident, input: $input:literal, env: $var:literal, value: $value:literal, expected: $expected:expr, require_var: $require_var:literal) => {
            #[test]
            #[serial_test::serial]
            fn $name() {
                if $require_var && !($input).contains(&format!("${{{}}}", $var)) {
                    panic!("input string {} doesn't contain variable {}", $input, $var);
                }

                std::env::set_var($var, $value);
                let expected: PathBuf = $expected;
                let result = expand_env_in_path($input).expect("failed to expand env in path");
                assert_eq!(result, expected);
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
        expected: PathBuf::from("/home/testuser/test/file")
    }

    test_env! {
        name: var_in_middle_shorter_than_value,
        input: "/home/testuser/${TEST_PATH}/file",
        env: "TEST_PATH",
        value: "test/subdir/subberdir",
        expected: PathBuf::from("/home/testuser/test/subdir/subberdir/file")
    }

    test_env! {
        name: var_at_end_shorter_than_value,
        input: "/home/testuser/${TEST_PATH}",
        env: "TEST_PATH",
        value: "test/subdir/file",
        expected: PathBuf::from("/home/testuser/test/subdir/file")
    }

    // Same length == var name + ${}
    test_env! {
        name: var_at_start_same_length_as_value,
        input: "${TEST_HOME}/test/file",
        env: "TEST_HOME",
        value: "/home/tester",
        expected: PathBuf::from("/home/tester/test/file")
    }

    test_env! {
        name: var_in_middle_same_length_as_value,
        input: "/home/testuser/${TEST_PATH}/file",
        env: "TEST_PATH",
        value: "/test/folder",
        expected: PathBuf::from("/home/testuser/test/folder/file")
    }

    test_env! {
        name: var_at_end_same_length_as_value,
        input: "/home/testuser/${TEST_PATH}",
        env: "TEST_PATH",
        value: "testing/file",
        expected: PathBuf::from("/home/testuser/testing/file")
    }

    test_env! {
        name: var_at_start_longer_than_value,
        input: "${TEST_HOME}/test/file",
        env: "TEST_HOME",
        value: "/home/test",
        expected: PathBuf::from("/home/test/test/file")
    }

    test_env! {
        name: var_in_middle_longer_than_value,
        input: "/home/testuser/${TEST_PATH}/file",
        env: "TEST_PATH",
        value: "test/dir",
        expected: PathBuf::from("/home/testuser/test/dir/file")
    }

    test_env! {
        name: var_at_end_longer_than_value,
        input: "/home/testuser/${TEST_PATH}",
        env: "TEST_PATH",
        value: "a/file",
        expected: PathBuf::from("/home/testuser/a/file")
    }

    test_env! {
        name: path_without_var_stays_same,
        input: "/path/without/variables",
        env: "UNUSED",
        value: "NOTHING",
        expected: PathBuf::from("/path/without/variables"),
        require_var: false
    }

    test_env! {
        name: path_with_two_variables,
        input: "/home/${TEST_USER}/somedir/${TEST_USER}/file",
        env: "TEST_USER",
        value: "testuser",
        expected: PathBuf::from("/home/testuser/somedir/testuser/file")
    }

    test_env! {
        name: var_without_braces_not_expanded,
        input: "/path/with/$INVALID/variable",
        env: "INVALID",
        value: "broken",
        expected: PathBuf::from("/path/with/$INVALID/variable"),
        require_var: false
    }

    test_env! {
        name: var_windows_style_not_expanded,
        input: "/path/with/%INVALID%/variable",
        env: "INVALID",
        value: "broken",
        expected: PathBuf::from("/path/with/%INVALID%/variable"),
        require_var: false
    }

    test_env! {
        name: vars_not_recursively_expanded,
        input: "${TEST_HOME}",
        env: "TEST_HOME",
        value: "${HOME}",
        expected: PathBuf::from("${HOME}")
    }

    test_env! {
        name: var_inside_var,
        input: "${WRAPPING${TEST_VAR}VARIABLE}",
        env: "TEST_VAR",
        value: "_",
        expected: PathBuf::from("${WRAPPING_VARIABLE}")
    }
}
