#![deny(clippy::all)]
#![deny(clippy::correctness)]
#![deny(clippy::style)]
#![deny(clippy::complexity)]
#![deny(clippy::perf)]

use std::collections::HashMap;
use std::path::PathBuf;

use directories::ProjectDirs;
use log::{debug, Level};
use structopt::{clap::Error as ClapError, StructOpt};
use thiserror::Error;

use config::builder;
pub use config::builder::Builder;
use config::environment::Environment;

use crate::combinator::Combinator;

pub mod combinator;
mod command;
pub mod config;

pub const CONFIG_FILE_NAME: &str = "config.toml";
pub const GAMES_DIR_SLUG: &str = "saves";
