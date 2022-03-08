use std::ops::DerefMut;
use std::{
    fs, io,
    ops::Deref,
    path::{Path, PathBuf},
};

use super::test_subscriber::MemorySubscriber;
use hoard::{
    command::Command,
    config::{Builder, Config, Error},
};

pub struct Tester {
    config: Config,
    home_dir: PathBuf,
    config_dir: PathBuf,
    data_dir: PathBuf,
    subscriber: MemorySubscriber,
    temp_dirs: [tempfile::TempDir; 3],
    local_uuid: uuid::Uuid,
    remote_uuid: uuid::Uuid,
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
                home_path
                    .join("Library")
                    .join("Application Support")
                    .join("com.shadow53.hoard"),
                home_path
                    .join("Library")
                    .join("Application Support")
                    .join("com.shadow53.hoard"),
            )
        };

        #[cfg(windows)]
        let (home_dir, config_dir, data_dir) = {
            ::std::env::set_var("APPDATA", _config_tmp.path());
            ::std::env::set_var("USERPROFILE", _home_tmp.path());
            (
                _home_tmp.path().to_path_buf(),
                _config_tmp.path().join("shadow53").join("hoard"),
                _config_tmp
                    .path()
                    .join("shadow53")
                    .join("hoard")
                    .join("data"),
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

        let config = {
            ::toml::from_str::<Builder>(toml_str)
                .expect("failed to parse configuration from TOML")
                .build()
                .expect("failed to build config")
        };

        Self {
            config,
            home_dir,
            config_dir,
            data_dir,
            subscriber: MemorySubscriber::new(log_level),
            temp_dirs: [home_tmp, config_tmp, data_tmp],
            local_uuid: uuid::Uuid::new_v4(),
            remote_uuid: uuid::Uuid::new_v4(),
        }
    }

    pub fn local_uuid(&self) -> &uuid::Uuid {
        &self.local_uuid
    }

    pub fn remote_uuid(&self) -> &uuid::Uuid {
        &self.remote_uuid
    }

    pub fn use_local_uuid(&self) {
        fs::write(self.uuid_path(), self.local_uuid.to_string())
            .expect("failed to write to uuid file");
    }

    pub fn use_remote_uuid(&self) {
        fs::write(self.uuid_path(), self.remote_uuid.to_string())
            .expect("failed to write to uuid file");
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn mut_config(&mut self) -> &mut Config {
        &mut self.config
    }

    pub fn reset_config(&mut self, toml_str: &str) {
        self.config = toml::from_str::<Builder>(toml_str)
            .expect("configuration should parse correctly")
            .build()
            .expect("configuration should build correctly");
    }

    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_dir
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    fn inner_run_command(&self, command: Command, force: bool) -> Result<(), Error> {
        let config = Config {
            command,
            force,
            ..self.config.clone()
        };

        self.clear_output();
        config.run()
    }

    #[inline]
    pub fn run_command(&self, command: Command) -> Result<(), Error> {
        self.inner_run_command(command, false)
    }

    #[inline]
    pub fn force_command(&self, command: Command) -> Result<(), Error> {
        self.inner_run_command(command, true)
    }

    pub fn extra_logging_output(&self) -> String {
        let list_home = Self::list_dir_to_string(self.home_dir(), 3, 0);
        let list_data = Self::list_dir_to_string(self.data_dir(), 4, 0);
        let list_env: String = std::env::vars()
            .map(|(key, val)| format!("{} = {}", key, val))
            .collect::<Vec<String>>()
            .join("\n");
        format!(
            "CONFIG:\n{:#?}\nOUTPUT:\n{}\nENV\n{}\nHOME:\n{}\nDATA DIR:\n{}",
            self.config(),
            self.output(),
            list_env,
            list_home,
            list_data
        )
    }

    fn list_dir_to_string(dir: &Path, max_depth: u8, depth: u8) -> String {
        let this_path = format!("{}|- {}", " ".repeat(depth.into()), dir.display());
        let content = match fs::read_dir(dir) {
            Err(error) => format!("ERROR: {}", error),
            Ok(iter) => iter
                .map(|entry| {
                    let entry_str = match entry {
                        Err(error) => format!("ERROR: {}", error),
                        Ok(entry) => {
                            let sub_entry = if entry.path().is_dir() && depth < max_depth {
                                format!(
                                    "\n{}",
                                    Self::list_dir_to_string(&entry.path(), max_depth, depth + 1)
                                )
                            } else {
                                String::new()
                            };
                            format!("{}{}", entry.path().display(), sub_entry)
                        }
                    };

                    let prefix = format!("{}|-", " ".repeat(depth.into()));

                    format!("\n{} {}", prefix, entry_str)
                })
                .collect::<String>(),
        };
        format!("{}{}", this_path, content)
    }

    fn handle_command_result(&self, command: Command, result: Result<(), Error>) {
        if let Err(error) = result {
            let debug_output = Self::extra_logging_output(self);
            panic!(
                "command {:?} failed: {:?}\n{}",
                command, error, debug_output
            );
        }
    }

    pub fn expect_command(&self, command: Command) {
        self.handle_command_result(command.clone(), self.run_command(command));
    }

    pub fn expect_forced_command(&self, command: Command) {
        self.handle_command_result(command.clone(), self.force_command(command));
    }

    pub fn clear_output(&self) {
        self.subscriber.clear();
    }

    pub fn output(&self) -> String {
        String::from_utf8_lossy(self.subscriber.output().deref()).to_string()
    }

    fn has_output(&self, output: &str) -> bool {
        let as_bytes = output.as_bytes();

        if self.subscriber.output().len() < as_bytes.len() {
            return false;
        }

        self.subscriber
            .output()
            .windows(as_bytes.len())
            .any(|window| window == as_bytes)
    }

    fn assert_output(&self, output: &str, matches: bool) {
        let debug_output = self.extra_logging_output();
        let not_or_not = if matches { "" } else { "not " };
        assert!(
            self.has_output(output) == matches,
            "expected \"{}\" {}in program output\n{}",
            output,
            not_or_not,
            debug_output
        );
    }

    pub fn assert_has_output(&self, output: &str) {
        self.assert_output(output, true);
    }

    pub fn assert_not_has_output(&self, output: &str) {
        self.assert_output(output, false);
    }

    fn uuid_path(&self) -> PathBuf {
        self.config_dir().join("uuid")
    }

    pub fn get_uuid(&self) -> io::Result<String> {
        fs::read_to_string(self.uuid_path())
    }

    pub fn set_uuid(&self, content: &str) -> io::Result<()> {
        fs::write(self.uuid_path(), content)
    }
}
