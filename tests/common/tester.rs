use futures::TryStreamExt;
use std::io::ErrorKind;
use std::ops::DerefMut;
use std::{
    ops::Deref,
    path::{Path, PathBuf},
};
use tokio::{fs, io};
use tokio_stream::wrappers::ReadDirStream;

use super::test_subscriber::MemorySubscriber;
use hoard::dirs::{COMPANY, PROJECT, TLD};
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
    #[cfg(windows)]
    old_appdata: PathBuf,
}

#[cfg(windows)]
impl Drop for Tester {
    fn drop(&mut self) {
        hoard::dirs::set_known_folder(hoard::dirs::FOLDERID_RoamingAppData, &self.old_appdata)
            .expect("restoring APPDATA should not fail")
    }
}

impl Tester {
    pub async fn new(toml_str: &str) -> Self {
        Self::with_log_level(toml_str, tracing::Level::INFO).await
    }

    pub async fn with_log_level(toml_str: &str, log_level: tracing::Level) -> Self {
        #[cfg(all(not(unix), not(windows)))]
        panic!("this target is not supported!");

        let home_tmp = tempfile::tempdir().expect("failed to create temporary directory");
        let config_tmp = tempfile::tempdir().expect("failed to create temporary directory");
        let data_tmp = tempfile::tempdir().expect("failed to create temporary directory");

        #[cfg(target_os = "macos")]
        let (home_dir, config_dir, data_dir) = {
            ::std::env::set_var("HOME", home_tmp.path());
            let config_dir = home_tmp
                .path()
                .join("Library")
                .join("Application Support")
                .join(format!("{}.{}.{}", TLD, COMPANY, PROJECT));
            (
                home_tmp.path().to_path_buf(),
                config_dir.clone(),
                config_dir,
            )
        };

        #[cfg(windows)]
        let old_appdata = hoard::dirs::get_known_folder(hoard::dirs::FOLDERID_RoamingAppData)
            .expect("getting APPDATA dir should not fail");
        #[cfg(windows)]
        let (home_dir, config_dir, data_dir) = {
            ::std::env::set_var("HOARD_TMP", home_tmp.path());
            hoard::dirs::set_known_folder(hoard::dirs::FOLDERID_RoamingAppData, config_tmp.path())
                .expect("failed to set APPDATA");
            let appdata = config_tmp.path().join(COMPANY).join(PROJECT);
            (
                home_tmp.path().to_path_buf(),
                appdata.join("config"),
                appdata.join("data"),
            )
        };

        #[cfg(all(not(target_os = "macos"), unix))]
        let (home_dir, config_dir, data_dir) = {
            ::std::env::set_var("HOME", home_tmp.path());
            ::std::env::set_var("XDG_CONFIG_HOME", config_tmp.path());
            ::std::env::set_var("XDG_DATA_HOME", data_tmp.path());
            (
                home_tmp.path().to_path_buf(),
                config_tmp.path().join(PROJECT),
                data_tmp.path().join(PROJECT),
            )
        };

        fs::create_dir_all(&config_dir)
            .await
            .expect("failed to create test config dir");
        fs::create_dir_all(&home_dir)
            .await
            .expect("failed to create test home dir");
        fs::create_dir_all(&data_dir)
            .await
            .expect("failed to create test data dir");

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
            #[cfg(windows)]
            old_appdata,
        }
    }

    pub fn local_uuid(&self) -> &uuid::Uuid {
        &self.local_uuid
    }

    pub fn remote_uuid(&self) -> &uuid::Uuid {
        &self.remote_uuid
    }

    pub async fn use_local_uuid(&self) {
        fs::write(self.uuid_path(), self.local_uuid.to_string())
            .await
            .expect("failed to write to uuid file");
    }

    pub async fn use_remote_uuid(&self) {
        fs::write(self.uuid_path(), self.remote_uuid.to_string())
            .await
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

    async fn inner_run_command(&self, command: Command, force: bool) -> Result<(), Error> {
        let config = Config {
            command,
            force,
            ..self.config.clone()
        };

        self.clear_output();
        config.run().await
    }

    #[inline]
    pub async fn run_command(&self, command: Command) -> Result<(), Error> {
        self.inner_run_command(command, false).await
    }

    #[inline]
    pub async fn force_command(&self, command: Command) -> Result<(), Error> {
        self.inner_run_command(command, true).await
    }

    pub async fn extra_logging_output(&self) -> String {
        let list_home = Self::list_dir_to_string(self.home_dir(), 3, 0).await;
        let list_data = Self::list_dir_to_string(self.data_dir(), 4, 0).await;
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

    #[async_recursion::async_recursion]
    async fn list_dir_to_string(dir: &Path, max_depth: u8, depth: u8) -> String {
        let this_path = format!("{}|- {}", " ".repeat(depth.into()), dir.display());
        let content = match fs::read_dir(dir).await {
            Err(error) => format!("ERROR: {}", error),
            Ok(iter) => ReadDirStream::new(iter)
                .and_then(|entry| async move {
                    let entry_str = {
                        let sub_entry = if entry.path().is_dir() && depth < max_depth {
                            format!(
                                "\n{}",
                                Self::list_dir_to_string(&entry.path(), max_depth, depth + 1).await
                            )
                        } else {
                            String::new()
                        };
                        format!("{}{}", entry.path().display(), sub_entry)
                    };

                    let prefix = format!("{}|-", " ".repeat(depth.into()));

                    Ok(format!("\n{} {}", prefix, entry_str))
                })
                .try_collect::<String>()
                .await
                .unwrap(),
        };
        format!("{}{}", this_path, content)
    }

    async fn handle_command_result(&self, command: Command, result: Result<(), Error>) {
        if let Err(error) = result {
            let debug_output = Self::extra_logging_output(self).await;
            panic!(
                "command {:?} failed: {:?}\n{}",
                command, error, debug_output
            );
        }
    }

    pub async fn expect_command(&self, command: Command) {
        self.handle_command_result(command.clone(), self.run_command(command).await);
    }

    pub async fn expect_forced_command(&self, command: Command) {
        self.handle_command_result(command.clone(), self.force_command(command).await);
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

    async fn assert_output(&self, output: &str, matches: bool) {
        let debug_output = self.extra_logging_output().await;
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

    pub async fn get_uuid(&self) -> io::Result<String> {
        fs::read_to_string(self.uuid_path()).await
    }

    pub async fn current_uuid(&self) -> Option<uuid::Uuid> {
        match self.get_uuid().await {
            Ok(s) => s.parse().ok(),
            Err(err) => match err.kind() {
                ErrorKind::NotFound => None,
                _ => panic!("unexpected error while reading UUID: {}", err),
            },
        }
    }

    pub async fn set_uuid(&self, content: &str) -> io::Result<()> {
        fs::write(self.uuid_path(), content).await
    }

    pub async fn clear_data_dir(&self) {
        let mut stream = fs::read_dir(self.data_dir())
            .await
            .map(ReadDirStream::new)
            .unwrap();
        while let Some(entry) = stream.try_next().await.unwrap() {
            if entry.path().is_file() {
                fs::remove_file(entry.path()).await.unwrap();
            } else if entry.path().is_dir() {
                fs::remove_dir_all(entry.path()).await.unwrap();
            }
        }
    }
}
