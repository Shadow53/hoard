//! Types that validate (check) Hoard configurations
//!
//! This module includes a single trait, [`Checker`], and all types that implement it.
//! Currently, that is only the [`LastPaths`](history::last_paths::LastPaths) checker.

pub mod history;

use crate::hoard::{Direction, Hoard};

/// Trait for validating [`Hoard`]s.
///
/// A [`Checker`] takes a [`Hoard`] and its name (as [`&str`]) as parameters and uses that
/// information plus any internal state to validate that it is safe to operate on that [`Hoard`].
pub trait Checker: Sized {
    /// The error type returned from the check.
    type Error: std::error::Error;
    /// Returns a new instance of the implementing Checker type.
    ///
    /// # Errors
    ///
    /// Any errors that may occur while creating an instance, such as I/O or consistency errors.
    fn new(name: &str, hoard: &Hoard, direction: Direction) -> Result<Self, Self::Error>;
    /// Returns an error if it is not safe to operate on the given [`Hoard`].
    ///
    /// # Errors
    ///
    /// Any error that prevents operations on the given [`Hoard`], or any errors that
    /// occur while performing the check.
    fn check(&mut self) -> Result<(), Self::Error>;
    /// Saves any persistent data to disk.
    ///
    /// # Errors
    ///
    /// Generally, any I/O errors that occur while persisting data.
    fn commit_to_disk(self) -> Result<(), Self::Error>;
}
