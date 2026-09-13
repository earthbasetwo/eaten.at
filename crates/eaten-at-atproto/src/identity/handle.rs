//! atproto handles: mutable, human-readable aliases for a DID.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::hostname::{validate_hostname, HostnameError};

/// A validated, lowercase atproto handle.
#[derive(Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Handle(String);

impl Handle {
    /// Normalize (trim, strip a leading `@`, lowercase) and validate.
    pub fn parse(input: &str) -> Result<Self, HostnameError> {
        let trimmed = input.trim().trim_start_matches('@');
        let lowered = trimmed.to_ascii_lowercase();
        validate_hostname(&lowered)?;
        Ok(Self(lowered))
    }

    /// The handle as a bare hostname.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The DNS name that carries this handle's `did=` TXT record.
    pub fn dns_txt_name(&self) -> String {
        format!("_atproto.{}", self.0)
    }
}

impl fmt::Debug for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Handle({})", self.0)
    }
}

impl fmt::Display for Handle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Handle {
    type Err = HostnameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl<'de> Deserialize<'de> for Handle {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_at_sign_and_whitespace() {
        let h = Handle::parse("  @Alice.Bsky.Social ").unwrap();
        assert_eq!(h.as_str(), "alice.bsky.social");
        assert_eq!(h.dns_txt_name(), "_atproto.alice.bsky.social");
    }

    #[test]
    fn rejects_invalid() {
        assert!(Handle::parse("@").is_err());
        assert!(Handle::parse("alice").is_err());
        assert!(Handle::parse("alice.localhost").is_err());
        assert!(Handle::parse("al ice.example.com").is_err());
    }
}
