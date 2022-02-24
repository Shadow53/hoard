mod common;

use std::fs;
use std::io::Write;
use common::test_helper::Tester;
use hoard::config::builder::{Builder, environment::Environment};
use pretty_assertions::assert_eq;

#[test]
#[serial_test::serial]
fn test_yaml_support() {
    let tester = Tester::new(common::BASE_CONFIG);
    let path = tester.config_dir().join("config.yaml");

    let mut file = fs::File::create(&path)
        .expect("failed to create YAML config file");
    let builder: Builder = toml::from_str(common::BASE_CONFIG)
        .expect("failed to parse TOML");
    serde_yaml::to_writer(&mut file, &builder).expect("failed to serialize to YAML");
    drop(file);

    let config = Builder::from_file(&path)
        .expect("failed to parse YAML config")
        .build()
        .expect("failed to build config");

    assert_eq!(&config, tester.config());

    let new_path = tester.config_dir().join("config.yml");
    fs::rename(path, &new_path);

    let config = Builder::from_file(&new_path)
        .expect("failed to parse YAML config")
        .build()
        .expect("failed to build config");
    
    assert_eq!(&config, tester.config());
}

#[test]
#[serial_test::serial]
fn test_toml_takes_precedence() {
    let tester = Tester::new(common::BASE_CONFIG);
    let yaml_path = tester.config_dir().join("config.yaml");
    let yml_path = tester.config_dir().join("config.yml");
    let toml_path = tester.config_dir().join("config.toml");
    
    let toml_config = Builder::new()
        .set_environments(maplit::btreemap! { String::from("toml") => Environment::default() });
    let yaml_config = Builder::new()
        .set_environments(maplit::btreemap! { String::from("yaml") => Environment::default() });
    {
        let mut file = fs::File::create(&toml_path)
            .expect("failed to create TOML config file");
        let toml_bytes = toml::to_vec(&toml_config)
            .expect("failed to serialize TOML");
        file.write_all(&toml_bytes)
            .expect("failed to write TOML to file");

        let mut file = fs::File::create(yaml_path)
            .expect("failed to create YAML config file");
        serde_yaml::to_writer(&mut file, &yaml_config)
            .expect("failed to write YAML to file");
        
        let mut file = fs::File::create(yml_path)
            .expect("failed to create YML config file");
        serde_yaml::to_writer(&mut file, &yaml_config)
            .expect("failed to write YML to file");
    }

    let config = Builder::from_default_file()
        .expect("failed to parse from default file");
        
    assert_eq!(config, toml_config);

    fs::remove_file(toml_path)
        .expect("failed to delete TOML file");

    let config = Builder::from_default_file()
        .expect("failed to parse YAML config");
    
    assert_eq!(config, yaml_config);
}