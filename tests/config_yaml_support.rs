mod common;

use crate::common::base::DefaultConfigTester;
use common::tester::Tester;
use hoard::config::builder::{environment::Environment, Builder};
use tokio::{fs, io::AsyncWriteExt};

#[tokio::test]
async fn test_yaml_support() {
    let tester = Tester::new(common::base::BASE_CONFIG).await;
    let path = tester.config_dir().join("config.yaml");

    let mut file = fs::File::create(&path).await.expect("failed to create YAML config file").into_std().await;
    let builder: Builder = toml::from_str(common::base::BASE_CONFIG).expect("failed to parse TOML");
    serde_yaml::to_writer(&mut file, &builder).expect("failed to serialize to YAML");
    drop(file);

    let config = Builder::from_file(&path)
        .await
        .expect("failed to parse YAML config")
        .build()
        .expect("failed to build config");

    assert_eq!(&config, tester.config());

    let new_path = tester.config_dir().join("config.yml");
    fs::rename(path, &new_path).await.expect("renaming file should succeed");

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
        let mut file = fs::File::create(&toml_path).await.expect("failed to create TOML config file");
        let toml_bytes = toml::to_vec(&toml_config).expect("failed to serialize TOML");
        file.write_all(&toml_bytes)
            .await
            .expect("failed to write TOML to file");
    }
    {
        let mut file = fs::File::create(yaml_path).await.expect("failed to create YAML config file").into_std().await;
        serde_yaml::to_writer(&mut file, &yaml_config).expect("failed to write YAML to file");
    }
    {
        let mut file = fs::File::create(yml_path).await.expect("failed to create YML config file").into_std().await;
        serde_yaml::to_writer(&mut file, &yaml_config).expect("failed to write YML to file");
    }

    std::thread::sleep(std::time::Duration::from_millis(500));

    let config = Builder::from_default_file().await.expect("failed to parse from default file");

    assert_eq!(config, toml_config);

    fs::remove_file(toml_path).await.expect("failed to delete TOML file");

    let config = Builder::from_default_file().await.expect("failed to parse YAML config");

    assert_eq!(config, yaml_config);

    drop(tester);
}
