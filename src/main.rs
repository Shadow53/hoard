use hoard::Config;
use std::io::Stdout;
use tracing::level_filters::LevelFilter;
use tracing::Level;
use tracing_subscriber::fmt::format::{Compact, DefaultFields, Format, Full, Pretty};
use tracing_subscriber::fmt::time::SystemTime;
use tracing_subscriber::fmt::SubscriberBuilder;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

const LOG_ENV: &str = "HOARD_LOG";

fn error_and_exit<E: std::error::Error>(err: E) -> ! {
    // Ignore error if default subscriber already exists
    // This just helps ensure that logging happens and is
    // consistent.
    let _ = get_subscriber().try_init();
    tracing::error!("{}", err);
    std::process::exit(1);
}

fn get_subscriber() -> SubscriberBuilder<Pretty, Format<Pretty, ()>, LevelFilter, fn() -> Stdout> {
    FmtSubscriber::builder()
        .pretty()
        .with_ansi(true)
        .with_level(true)
        .with_target(false)
        .without_time()
        .with_max_level(if cfg!(debug_assertions) {
            Level::DEBUG
        } else {
            Level::INFO
        })
}

fn main() {
    // Set up default logging
    // There is no obvious way to set up a default logging level in case the env
    // isn't set, so use this match thing instead.
    let subscriber = get_subscriber();
    match std::env::var_os(LOG_ENV) {
        Some(_) => match EnvFilter::try_from_env(LOG_ENV) {
            Err(err) => error_and_exit(err),
            Ok(filter) => subscriber
                .with_env_filter(filter)
                .with_filter_reloading()
                .init(),
        },
        None => subscriber.init(),
    };

    // Get configuration
    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => error_and_exit(err),
    };

    // Run command with config
    if let Err(err) = config.run() {
        error_and_exit(err);
    }
}
