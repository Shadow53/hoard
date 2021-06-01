use hoard::config::Error;
use hoard::Config;

fn main() -> Result<(), Error> {
    let config = Config::load()?;
    config.run()
}
