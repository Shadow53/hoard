use crate::checkers::history::{get_or_generate_uuid, get_uuid_file};
use crate::Config;
use tokio::fs;

use super::DEFAULT_CONFIG;

#[tracing::instrument(skip_all)]
pub(crate) async fn run_init(config: &Config) -> Result<(), super::Error> {
    let data_dir = crate::paths::hoards_dir();
    let config_file = config.config_file.as_path();

    tracing::info!("creating data directory: {}", data_dir.display());
    fs::create_dir_all(&data_dir)
        .await
        .map_err(|error| super::Error::Init {
            path: data_dir.to_path_buf(),
            error,
        })?;

    if let Some(parent) = config_file.parent() {
        tracing::info!("creating config directory: {}", parent.display());
        fs::create_dir_all(parent)
            .await
            .map_err(|error| super::Error::Init {
                path: parent.to_path_buf(),
                error,
            })?;
    }

    let uuid_file = get_uuid_file();
    if !uuid_file.exists() {
        tracing::info!("device id not found, creating a new one");
        get_or_generate_uuid()
            .await
            .map_err(|error| super::Error::Init {
                path: uuid_file,
                error,
            })?;
    }

    if !config_file.exists() {
        tracing::info!(
            "no configuration file found, creating default at {}",
            config_file.display()
        );
        fs::write(config_file, DEFAULT_CONFIG)
            .await
            .map_err(|error| super::Error::Init {
                path: config_file.to_path_buf(),
                error,
            })?;
    }

    tracing::info!(
        "If you want to synchronize hoards between multiple machines, synchronize {}",
        data_dir.display()
    );
    tracing::info!("To synchronize your Hoard configuration as well, add an entry that backs up {}, not the whole directory", config_file.display());

    Ok(())
}
