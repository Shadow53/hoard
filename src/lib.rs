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
#![deny(rustdoc::missing_doc_code_examples)]
#![deny(
    absolute_paths_not_starting_with_crate,
    anonymous_parameters,
    bad_style,
    const_err,
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
    pointer_structural_match,
    private_in_public,
    semicolon_in_expressions_from_macros,
    single_use_lifetimes,
    trivial_casts,
    trivial_numeric_casts,
    unaligned_references,
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
pub mod paths;

/// The default file stem of the configuration file (i.e. without file extension).
pub const CONFIG_FILE_STEM: &str = "config";

/// The name of the directory containing the backed up hoards.
pub const HOARDS_DIR_SLUG: &str = "hoards";
