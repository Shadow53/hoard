use std::collections::BTreeMap;
use std::env;
use serde::{Deserialize, Serialize};
use crate::env_vars::StringWithEnv;

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(transparent)]
#[repr(transparent)]
pub struct EnvVarDefaults(BTreeMap<String, String>);

impl EnvVarDefaults {
    pub(super) fn merge_with(&mut self, other: Self) {
        for (var, value) in other.0 {
            self.0.insert(var, value);
        }
    }

    /// Attempt to apply every default value to any unset environment variables, expanding their
    /// values first.
    ///
    /// This will repeatedly attempt to apply variables as long as at least one successfully applies.
    pub(super) fn apply(self) -> Result<(), ()> {
        // Add one just so the `while` condition is true once.
        let mut remaining_last_loop = self.0.len() + 1;
        let mut last_loop = BTreeMap::new();
        let mut this_loop = self.0;

        while remaining_last_loop != this_loop.len() {
            last_loop = std::mem::take(&mut this_loop);
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
            Err(())
        }
    }
}