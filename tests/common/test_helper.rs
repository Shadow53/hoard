use std::{path::{Path, PathBuf}, ops::Deref};

use super::test_subscriber::MemorySubscriber;
use hoard::{config::{Config, Error}, command::Command};

pub struct Tester {
    config: Config,
    home_dir: PathBuf,
    config_dir: PathBuf,
    data_dir: PathBuf,
    subscriber: MemorySubscriber,
    temp_dirs: [tempfile::TempDir; 3],
}

impl Tester {
    pub fn new(toml_str: &str) -> Self {
        Self::with_log_level(toml_str, tracing::Level::INFO)
    }

    pub fn with_log_level(toml_str: &str, log_level: tracing::Level) -> Self {
        let home_tmp = tempfile::tempdir().expect("failed to create temporary directory");
        let config_tmp = tempfile::tempdir().expect("failed to create temporary directory");
        let data_tmp = tempfile::tempdir().expect("failed to create temporary directory");

        #[cfg(all(not(unix), not(windows)))]
        panic!("this target is not supported!");

        #[cfg(target_os = "macos")]
        let (home_dir, config_dir, data_dir) = {
            let home_path = _home_tmp.path();
            ::std::env::set_var("HOME", home_path);
            (
                home_path.to_path_buf(),
                home_path.join("Library").join("Application Support").join("com.shadow53.hoard"),
                home_path.join("Library").join("Application Support").join("com.shadow53.hoard"),
            )
        };

        #[cfg(windows)]
        let (home_dir, config_dir, data_dir) = {
            ::std::env::set_var("APPDATA", _config_tmp.path());
            ::std::env::set_var("USERPROFILE", _home_tmp.path());
            (
                _home_tmp.path().to_path_buf(),
                _config_tmp.path().join("shadow53").join("hoard"),
                _config_tmp.path().join("shadow53").join("hoard").join("data"),
            )
        };

        #[cfg(all(not(target_os = "macos"), unix))]
        let (home_dir, config_dir, data_dir) = {
            ::std::env::set_var("HOME", home_tmp.path());
            ::std::env::set_var("XDG_CONFIG_HOME", config_tmp.path());
            ::std::env::set_var("XDG_DATA_HOME", data_tmp.path());
            (
                home_tmp.path().to_path_buf(),
                config_tmp.path().join("hoard"),
                data_tmp.path().join("hoard"),
            )
        };

        ::std::fs::create_dir_all(&config_dir).expect("failed to create test config dir");
        ::std::fs::create_dir_all(&home_dir).expect("failed to create test home dir");
        ::std::fs::create_dir_all(&data_dir).expect("failed to create test data dir");

        let config = ::toml::from_str::<::hoard::config::builder::Builder>(toml_str)
            .expect("failed to parse configuration from TOML")
            .build()
            .expect("failed to build config");
        
        Self {
            config,
            home_dir,
            config_dir,
            data_dir,
            subscriber: MemorySubscriber::new(log_level),
            temp_dirs: [home_tmp, config_tmp, data_tmp],
        }
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn mut_config(&mut self) -> &mut Config {
        &mut self.config
    }

    pub fn home_dir(&self) -> &Path { &self.home_dir }

    pub fn config_dir(&self) -> &Path { &self.config_dir }

    pub fn data_dir(&self) -> &Path { &self.data_dir }

    pub fn run_command(&self, command: Command, force: bool) -> Result<(), Error> {
        let config = Config {
            command,
            force,
            .. self.config.clone()
        };

        self.clear_output();
        config.run()
    }

    pub fn clear_output(&self) {
        self.subscriber.clear();
    }

    pub fn output(&self) -> String {
        String::from_utf8_lossy(self.subscriber.output().deref()).to_string()
    }

    fn has_output(&self, output: &str) -> bool {
        let as_bytes = output.as_bytes();
        self.subscriber.output()
            .windows(as_bytes.len())
            .any(|window| window == as_bytes)
    }

    pub fn assert_has_output(&self, output: &str) {
        assert!(self.has_output(output), "expected \"{}\" in program output\n{:?}\n{}", output, self.config, self.output());
    }

    pub fn assert_not_has_output(&self, output: &str) {
    assert!(!self.has_output(output), "expected \"{}\" NOT in program output\n{:?}\n{}", output, self.config, self.output());
    }
}