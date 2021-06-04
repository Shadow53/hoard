use env_logger::Builder;
use hoard::config::Error;
use hoard::Config;
use log::LevelFilter;

fn error_and_exit(err: Error) -> ! {
    log::error!("{}", err);
    std::process::exit(1);
}

fn main() {
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
    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => error_and_exit(err),
    };

    // Use configured log level
    log::set_max_level(config.log_level.to_level_filter());

    // Run command with config
    if let Err(err) = config.run() {
        error_and_exit(err);
    }
}
