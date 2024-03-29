//! Types for working with files that are managed by Hoard.
//!
//! [`HoardItem`] manages only the related paths. All checks for existence, content, etc. are done
//! in the methods that return the value.
//!
//! [`CachedHoardItem`] reads all of the relevant information at creation time and returns cached
//! values for content, etc. It provides the same interface as [`HoardItem`].

mod cached;
#[allow(clippy::module_inception)]
mod hoard_item;

pub use hoard_item::HoardItem;

#[allow(clippy::useless_attribute)]
#[allow(clippy::module_name_repetitions)]
pub use cached::CachedHoardItem;
