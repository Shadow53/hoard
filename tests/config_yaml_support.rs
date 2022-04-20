mod common;

use crate::common::base::DefaultConfigTester;
use common::tester::Tester;
use hoard::config::builder::{environment::Environment, Builder};
use tokio::fs;

#[tokio::test]
async fn test_yaml_support() {
    let tester = Tester::new(common::base::BASE_CONFIG).await;
    let path = tester.config_dir().join("config.yaml");

    let builder: Builder = toml::from_str(common::base::BASE_CONFIG).expect("failed to parse TOML");
    let content = serde_yaml::to_vec(&builder).expect("failed to serialize to YAML");
    fs::write(&path, &content).await.expect("failed to write to YAML config file");

    let config = Builder::from_file(&path)
        .await
        .expect("failed to parse YAML config")
        .build()
        .expect("failed to build config");

    assert_eq!(&config, tester.config());

    let new_path = tester.config_dir().join("config.yml");
    fs::rename(path, &new_path)
        .await
        .expect("renaming file should succeed");

    let config = Builder::from_file(&new_path)
        .await
        .expect("failed to parse YAML config")
        .build()
        .expect("failed to build config");

    assert_eq!(&config, tester.config());
}

#[tokio::test]
async fn test_toml_takes_precedence() {
    let tester = DefaultConfigTester::new().await;
    let yaml_path = tester.config_dir().join("config.yaml");
    let yml_path = tester.config_dir().join("config.yml");
    let toml_path = tester.config_dir().join("config.toml");

    let toml_config = Builder::new()
        .set_environments(maplit::btreemap! { "toml".parse().unwrap() => Environment::default() });
    let yaml_config = Builder::new()
        .set_environments(maplit::btreemap! { "yaml".parse().unwrap() => Environment::default() });
    {

        let toml_bytes = toml::to_vec(&toml_config).expect("failed to serialize TOML");
        fs::write(&toml_path, &toml_bytes).await.expect("failed to write TOML to file");
    }
    {
        let content = serde_yaml::to_vec(&yaml_config).expect("failed to serialize YAML");
        fs::write(&yaml_path, &content).await.expect("failed to write to YAML file");
    }
    {
        let content = serde_yaml::to_vec(&yaml_config).expect("failed to serialize YAML");
        fs::write(&yml_path, &content).await.expect("failed to write to YML file");
    }

    std::thread::sleep(std::time::Duration::from_millis(500));

    let config = Builder::from_default_file()
        .await
        .expect("failed to parse from default file");

    assert_eq!(config, toml_config);

    fs::remove_file(toml_path)
        .await
        .expect("failed to delete TOML file");

    let config = Builder::from_default_file()
        .await
        .expect("failed to parse YAML config");

    assert_eq!(config, yaml_config);

    drop(tester);
}
