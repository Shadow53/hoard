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
    Regex::new(r#"\$\{[^(=|\x{0})]+?}"#).expect("failed to compile regular expression")
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

    while let Some(mat) = ENV_REGEX.find(&new_path[start..]) {
        let var = mat.as_str();
        let var = &var[2..var.len() - 1];
        let value = env::var(var)?;

        old_start = start;
        start = start + mat.start() + value.len() + 1;
        if start > (new_path.len() + value.len() - mat.as_str().len()) {
            start = new_path.len();
        }

        let range = mat.range();
        new_path.replace_range(range.start + old_start..range.end + old_start, &value);
    }

    // Splitting into components and collecting will collapse multiple separators.
    Ok(PathBuf::from(new_path).components().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::MAIN_SEPARATOR;

    fn get_home_without_starting_sep() -> String {
        let home = env::var("HOME").expect("failed to find HOME");
        home.strip_prefix(MAIN_SEPARATOR)
            .map_or(home.clone(), std::borrow::ToOwned::to_owned)
    }

    #[test]
    fn path_starting_with_var() {
        let home = get_home_without_starting_sep();
        let expected: PathBuf = ["/", &home, "testdir", "testfile"].iter().collect();
        let result =
            expand_env_in_path("${HOME}/testdir/testfile").expect("failed to expand env in path");
        assert_eq!(result, expected);
    }

    #[test]
    fn path_wrapping_var() {
        let home = get_home_without_starting_sep();
        let expected: PathBuf = vec!["/start", &home, "testdir"].into_iter().collect();
        let result =
            expand_env_in_path("/start/${HOME}/testdir").expect("failed to expand env in path");
        assert_eq!(result, expected);
    }

    #[test]
    fn path_ending_in_var() {
        let home = get_home_without_starting_sep();
        let expected: PathBuf = vec!["/start", "testdir", &home].into_iter().collect();
        let result =
            expand_env_in_path("/start/testdir/${HOME}").expect("failed to expand env in path");
        assert_eq!(result, expected);
    }

    #[test]
    fn path_without_var_stays_same() {
        let template = "/path/without/variables";
        let expected = PathBuf::from(template);
        let result =
            expand_env_in_path(template).expect("failed to process path without variables");
        assert_eq!(result, expected);
    }

    #[test]
    fn path_with_two_variables() {
        let home = get_home_without_starting_sep();
        let expected: PathBuf = vec!["/start", &home, "testdir", &home, "end"]
            .into_iter()
            .collect();
        let result = expand_env_in_path("/start/${HOME}/testdir/${HOME}/end")
            .expect("failed to expand env in path");
        assert_eq!(result, expected);
    }

    #[test]
    fn path_with_variables_without_braces_not_expanded() {
        let template = "/path/with/$INVALID/variable";
        let expected = PathBuf::from(template);
        let result =
            expand_env_in_path(template).expect("failed to process path with invalid variable");
        assert_eq!(result, expected);
    }

    #[test]
    fn path_with_win32_style_variable_not_expanded() {
        let template = "/path/with/%INVALID%/variable";
        let expected = PathBuf::from(template);
        let result =
            expand_env_in_path(template).expect("failed to process path with invalid variable");
        assert_eq!(result, expected);
    }
}
