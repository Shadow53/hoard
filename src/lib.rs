//! A CLI program for managing files across multiple devices.
//!
//! You can think of `hoard` as a dotfiles management tool, though its intended use extends
//! beyond that. `hoard` can be used for backing up and restoring any kind of file from/to any
//! location on the filesystem. In fact, the original purpose behind writing it was to synchronize
//! save files for games that don't support cloud saves.
//!
//! # Terminology
//!
//! The following terms have special meanings when talking about `hoard`.
//!
//! - [`Hoard`](crate::config::builder::hoard::Hoard): A collection at least one
//!   [`Pile`](crate::config::builder::hoard::Pile).
//! - [`Pile`](crate::config::builder::hoard::Pile): A single file or directory in a
//!   [`Hoard`](crate::config::builder::hoard::Hoard).
//! - [`Environment`](crate::config::builder::environment::Environment): A combination of conditions
//!   that can be used to determine where to find files in a [`Pile`](crate::config::builder::hoard::Pile).

#![deny(clippy::all)]
#![deny(clippy::correctness)]
#![deny(clippy::style)]
#![deny(clippy::complexity)]
#![deny(clippy::perf)]
#![deny(clippy::pedantic)]
// See https://github.com/rust-lang/rust/issues/87858
//#![deny(rustdoc::missing_doc_code_examples)]
#![deny(
absolute_paths_not_starting_with_crate,
anonymous_parameters,
bad_style,
dead_code,
keyword_idents,
improper_ctypes,
macro_use_extern_crate,
meta_variable_misuse, // May have false positives
missing_abi,
missing_debug_implementations, // can affect compile time/code size
missing_docs,
no_mangle_generic_items,
non_shorthand_field_patterns,
noop_method_call,
overflowing_literals,
path_statements,
patterns_in_fns_without_body,
semicolon_in_expressions_from_macros,
single_use_lifetimes,
trivial_casts,
trivial_numeric_casts,
unconditional_recursion,
unreachable_pub,
unsafe_code,
unused,
unused_allocation,
unused_comparisons,
unused_extern_crates,
unused_import_braces,
unused_lifetimes,
unused_parens,
unused_qualifications,
variant_size_differences,
while_true
)]

pub use config::Config;

pub mod checkers;
pub mod checksum;
pub mod combinator;
pub mod command;
pub mod config;
pub(crate) mod diff;
pub mod dirs;
pub mod env_vars;
pub mod filters;
pub mod hoard;
pub mod hoard_item;
pub mod logging;
pub mod newtypes;
pub mod paths;
pub mod test;

/// The default file stem of the configuration file (i.e. without file extension).
pub const CONFIG_FILE_STEM: &str = "config";

/// The name of the directory containing the backed up hoards.
pub const HOARDS_DIR_SLUG: &str = "hoards";

#[inline]
pub(crate) fn tap_log_error<E: std::error::Error>(error: &E) {
    tracing::error!(%error);
}

#[inline]
pub(crate) fn tap_log_error_msg<E: std::error::Error>(msg: &'_ str) -> impl Fn(&E) + '_ {
    move |error| {
        tracing::error!(%error, "{}", msg);
    }
}

#[inline]
pub(crate) fn create_log_error<T, E: std::error::Error>(error: E) -> Result<T, E> {
    tap_log_error(&error);
    Err(error)
}

#[inline]
pub(crate) fn create_log_error_msg<T, E: std::error::Error>(msg: &str, error: E) -> Result<T, E> {
    tap_log_error_msg(msg)(&error);
    Err(error)
}

pub(crate) fn map_log_error<E1: std::error::Error, E2: std::error::Error>(
    map: impl Fn(E1) -> E2,
) -> impl Fn(E1) -> E2 {
    move |error| {
        let error = map(error);
        tap_log_error(&error);
        error
    }
}

pub(crate) fn map_log_error_msg<'m, E1: std::error::Error, E2: std::error::Error>(
    msg: &'m str,
    map: impl Fn(E1) -> E2 + 'm,
) -> impl Fn(E1) -> E2 + 'm {
    move |error| {
        let error = map(error);
        tap_log_error_msg(msg)(&error);
        error
    }
}
