use hoard::config::Error;
use hoard::Config;
use tracing::Level;
use tracing_subscriber::{EnvFilter, FmtSubscriber};

fn error_and_exit(err: Error) -> ! {
    tracing::error!("{}", err);
    std::process::exit(1);
}

fn main() {
    // Set up default logging
    let env_filter = EnvFilter::from_env("HOARD_LOG")
        .add_directive(Level::WARN.into())
        .add_directive(
            "hoard=info"
                .parse()
                .expect("failed to parse tracing directive"),
        );
    FmtSubscriber::builder()
        .compact()
        .with_ansi(true)
        .with_level(true)
        .with_env_filter(env_filter)
        .init();

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
