use structopt::StructOpt;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("error while printing help message: {0}")]
    PrintHelp(structopt::clap::Error),
}

#[derive(Clone, PartialEq, Debug, StructOpt)]
pub enum Command {
    Help,
    Backup,
    Restore,
}

impl Default for Command {
    fn default() -> Self {
        Self::Help
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_command_is_help() {
        // The default command is help if one is not given
        assert_eq!(Command::Help, Command::default());
    }
}
