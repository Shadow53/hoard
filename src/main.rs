use env_logger::Builder;
use hoard::config::Error;
use hoard::Config;
use log::LevelFilter;

fn main() -> Result<(), Error> {
    // Set up default logging
    let mut builder = Builder::new();
    builder.filter_level(if cfg!(debug_assertions) {
        LevelFilter::Debug
    } else {
        LevelFilter::Info
    });
    builder.parse_env("HOARD_LOG");
    builder.init();

    // Get configuration
    let config = Config::load()?;

    // Use configured log level
    log::set_max_level(config.log_level.to_level_filter());

    // Run command with config
    config.run()
}
