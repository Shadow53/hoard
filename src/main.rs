use hoard::Config;
use tracing_subscriber::util::SubscriberInitExt;
mod logging;

fn error_and_exit<E: std::error::Error>(err: E) -> ! {
    // Ignore error if default subscriber already exists
    // This just helps ensure that logging happens and is
    // consistent.
    let _guard = logging::get_subscriber().set_default();
    tracing::error!("{}", err);
    std::process::exit(1);
}

#[tokio::main]
async fn main() {
    // Set up default logging
    // There is no obvious way to set up a default logging level in case the env
    // isn't set, so use this match thing instead.
    let _guard = logging::get_subscriber().set_default();

    // Get configuration
    let config = match Config::load().await {
        Ok(config) => config,
        Err(err) => error_and_exit(err),
    };

    // Run command with config
    if let Err(err) = config.run().await {
        error_and_exit(err);
    }
}
