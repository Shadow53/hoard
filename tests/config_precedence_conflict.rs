use hoard::config::builder::envtrie::Error as TrieError;
use hoard::config::builder::hoard::Error as HoardError;
use hoard::config::builder::{Builder, Error as BuildError};
use maplit::hashset;

const CONFIG: &str = r#"
# Both foo and baz are preferred to bar
exclusivity = [
    ["foo", "bar"],
    ["baz", "bar"],
]

# CARGO should be set when running tests
[envs.bar]
    env = [{ var = "CARGO" }]
[envs.baz]
    env = [{ var = "CARGO" }]
[envs.foo]
    env = [{ var = "CARGO" }]

# Two unrelated envs that do not conflict with each other
# should have the same score and cause these paths to conflict.
[hoards.test]
    "foo" = "/some/path"
    "baz" = "/some/other/path"
"#;

#[test]
fn test_results_in_indecision() {
    let builder: Builder = toml::from_str(CONFIG).expect("parsing toml");
    let err = builder.build().expect_err("determining paths should fail");
    match err {
        BuildError::ProcessHoard(HoardError::EnvTrie(err)) => match err {
            TrieError::Indecision(left, right) => assert_eq!(
                hashset! { left, right },
                hashset! { "foo".parse().unwrap(), "baz".parse().unwrap() }
            ),
            _ => panic!("Unexpected error: {err}"),
        },
        _ => panic!("Unexpected error: {err}"),
    }
}
