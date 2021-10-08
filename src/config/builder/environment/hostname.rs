//! See [`Hostname`].

use serde::{Deserialize, Serialize};
use std::convert::TryInto;
use std::fmt;
use std::fmt::Formatter;

/// A conditional structure that compares the system's hostname to the given string.
#[derive(Clone, PartialEq, Debug, Serialize, Deserialize, Hash)]
#[serde(transparent)]
pub struct Hostname(pub String);

impl TryInto<bool> for Hostname {
    type Error = super::Error;

    fn try_into(self) -> Result<bool, super::Error> {
        let Hostname(expected) = self;
        let host = hostname::get().map_err(super::Error::Hostname)?;

        // grcov: ignore-start
        tracing::trace!(
            hostname = host.to_string_lossy().as_ref(),
            %expected,
            "checking if system hostname matches expected",
        );
        // grcov: ignore-end

        Ok(host == expected.as_str())
    }
}

impl fmt::Display for Hostname {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Hostname(hostname) = self;
        write!(f, "HOSTNAME == {}", hostname)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_correct_hostname() {
        let host = hostname::get()
            .expect("failed to get hostname for testing")
            .to_str()
            .expect("failed to convert to str")
            .to_owned();

        let hostname_test = Hostname(host);

        let has_hostname: bool = hostname_test.try_into().expect("checking hostname failed");

        assert!(has_hostname);
    }

    #[test]
    fn test_incorrect_hostname() {
        let mut hostname = hostname::get()
            .expect("failed to get hostname for testing")
            .to_str()
            .expect("failed to convert to str")
            .to_owned();
        hostname.push_str("-invalid");

        let hostname_test = Hostname(hostname);

        let has_hostname: bool = hostname_test.try_into().expect("checking hostname failed");

        assert!(!has_hostname);
    }
}
