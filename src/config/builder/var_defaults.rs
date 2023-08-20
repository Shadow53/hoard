//! See [`EnvVarDefaults`].

use crate::env_vars::StringWithEnv;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Formatter;
use std::{env, fmt};

/// Failed to apply one or more environment variables in [`EnvVarDefaults::apply`].
///
/// Most common reasons for this occurring is trying to use an unset variable in a default value,
/// or having two unset variables' values dependent on each other.
#[derive(Debug, thiserror::Error)]
pub struct EnvVarDefaultsError(BTreeMap<String, String>);

impl fmt::Display for EnvVarDefaultsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        assert!(!self.0.is_empty());
        write!(f, "Could not apply environment variable defaults. One or more default values requires an unset variable.")?;
        for (var, value) in &self.0 {
            write!(f, "\n{var}: {value:?}")?;
        }
        Ok(())
    }
}

/// Define variables and their default values, should that variable not otherwise be defined.
///
/// Variable default values can interpolate the values of other environment variables.
/// See [`StringWithEnv`] for the required syntax.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(transparent)]
#[repr(transparent)]
#[allow(clippy::module_name_repetitions)]
pub struct EnvVarDefaults(BTreeMap<String, String>);

impl EnvVarDefaults {
    #[cfg(test)]
    pub(super) fn insert(&mut self, var: String, value: String) -> Option<String> {
        self.0.insert(var, value)
    }

    pub(super) fn merge_with(&mut self, other: Self) {
        for (var, value) in other.0 {
            self.0.insert(var, value);
        }
    }

    /// Attempt to apply every default value to any unset environment variables, expanding their
    /// values first.
    ///
    /// This will repeatedly attempt to apply variables as long as at least one successfully applies.
    ///
    /// # Errors
    ///
    /// See [`EnvVarDefaultsError`].
    pub(super) fn apply(self) -> Result<(), EnvVarDefaultsError> {
        // Add one just so the `while` condition is true once.
        let mut remaining_last_loop = self.0.len() + 1;
        let mut this_loop = self.0;

        while remaining_last_loop != this_loop.len() {
            let last_loop = std::mem::take(&mut this_loop);
            remaining_last_loop = last_loop.len();
            for (var, value) in last_loop {
                if env::var_os(&var).is_none() {
                    match StringWithEnv::from(value.clone()).process() {
                        Err(_) => _ = this_loop.insert(var, value),
                        Ok(value) => {
                            // This function can panic under certain circumstances. Either check for
                            // them or catch the panic.
                            env::set_var(var, value);
                        }
                    }
                }
            }
        }

        if this_loop.is_empty() {
            Ok(())
        } else {
            Err(EnvVarDefaultsError(this_loop))
        }
    }
}
