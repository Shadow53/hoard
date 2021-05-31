#![deny(clippy::all)]
#![deny(clippy::correctness)]
#![deny(clippy::style)]
#![deny(clippy::complexity)]
#![deny(clippy::perf)]

pub use config::builder::Builder;

pub mod combinator;
mod command;
pub mod config;

pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const GAMES_DIR_SLUG: &str = "saves";
