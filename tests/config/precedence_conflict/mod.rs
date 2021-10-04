use hoard::config::builder::envtrie::Error as TrieError;
use hoard::config::builder::hoard::Error as HoardError;
use hoard::config::builder::{Builder, Error as BuildError};
use maplit::hashset;

#[test]
fn test_results_in_indecision() {
    let file = std::include_str!("config.toml");
    let builder: Builder = toml::from_str(file).expect("parsing toml");
    let err = builder.build().expect_err("determining paths should fail");
    match err {
        BuildError::ProcessHoard(HoardError::EnvTrie(err)) => match err {
            TrieError::Indecision(left, right) => assert_eq!(
                hashset! { left, right },
                hashset! { "foo".into(), "baz".into() }
            ),
            _ => panic!("Unexpected error: {}", err), // grcov: ignore
        },
        _ => panic!("Unexpected error: {}", err), // grcov: ignore
    }
}
