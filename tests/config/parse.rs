const TEST_CONFIG: &str = r#"
[[environments.linux]]
    os = ["linux"]
[[environments.bsd]]
    os = ["freebsd", "dragonfly", "netbsd", "openbsd"]
"#;

#[test]
fn test_parse_config() {
    Config::parse()
}
