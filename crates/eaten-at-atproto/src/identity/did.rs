//! Decentralized identifiers as used by atproto.
//!
//! atproto supports exactly two DID methods, `did:plc` and `did:web`, and
//! constrains both more tightly than the generic DID specification does.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use super::hostname::{validate_hostname, HostnameError};

/// Longest DID we will accept, in bytes. The atproto spec allows 2 KB; nothing
/// legitimate comes close.
const MAX_DID_LEN: usize = 2048;
/// `did:plc` identifiers are exactly 24 base32 characters.
const PLC_ID_LEN: usize = 24;

/// A validated atproto DID.
#[derive(Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct Did(String);

/// The DID method an identifier uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DidMethod {
    /// `did:plc:<24 base32 chars>`
    Plc,
    /// `did:web:<hostname>` with no port or path.
    Web,
}

/// Why a string is not an acceptable atproto DID.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DidError {
    #[error("DID is empty")]
    Empty,
    #[error("DID is longer than {MAX_DID_LEN} bytes")]
    TooLong,
    #[error("DID does not start with `did:`")]
    MissingScheme,
    #[error("unsupported DID method `{0}`; atproto allows only `plc` and `web`")]
    UnsupportedMethod(String),
    #[error("did:plc identifier must be {PLC_ID_LEN} lowercase base32 characters")]
    InvalidPlcId,
    #[error("did:web hostname is invalid: {0}")]
    InvalidWebHost(#[from] HostnameError),
    #[error("did:web with a port or path is not allowed in atproto")]
    WebPortOrPath,
}

impl Did {
    /// Validate and wrap a DID string.
    pub fn parse(input: &str) -> Result<Self, DidError> {
        if input.is_empty() {
            return Err(DidError::Empty);
        }
        if input.len() > MAX_DID_LEN {
            return Err(DidError::TooLong);
        }
        let rest = input.strip_prefix("did:").ok_or(DidError::MissingScheme)?;
        let (method, id) = rest
            .split_once(':')
            .ok_or_else(|| DidError::UnsupportedMethod(rest.to_owned()))?;
        match method {
            "plc" => validate_plc_id(id)?,
            "web" => validate_web_host(id)?,
            other => return Err(DidError::UnsupportedMethod(other.to_owned())),
        }
        Ok(Self(input.to_owned()))
    }

    /// The full DID string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Which method this DID uses.
    pub fn method(&self) -> DidMethod {
        if self.0.starts_with("did:plc:") {
            DidMethod::Plc
        } else {
            DidMethod::Web
        }
    }

    /// The method-specific identifier: the part after `did:plc:` or `did:web:`.
    pub fn method_specific_id(&self) -> &str {
        // Validation guarantees both prefixes are present and the same length.
        &self.0["did:plc:".len()..]
    }
}

fn validate_plc_id(id: &str) -> Result<(), DidError> {
    let ok = id.len() == PLC_ID_LEN
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || (b'2'..=b'7').contains(&b));
    if ok {
        Ok(())
    } else {
        Err(DidError::InvalidPlcId)
    }
}

fn validate_web_host(id: &str) -> Result<(), DidError> {
    // A generic did:web encodes ports as `%3A` and paths with `:`. atproto
    // forbids both, so any such character means the DID is out of scope.
    if id.contains(':') || id.contains('%') || id.contains('/') {
        return Err(DidError::WebPortOrPath);
    }
    validate_hostname(id)?;
    Ok(())
}

impl fmt::Debug for Did {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Did({})", self.0)
    }
}

impl fmt::Display for Did {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for Did {
    type Err = DidError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl<'de> Deserialize<'de> for Did {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLC: &str = "did:plc:re3ebnp5v7ffagz6rb6xfei4";

    #[test]
    fn accepts_plc() {
        let did = Did::parse(PLC).unwrap();
        assert_eq!(did.method(), DidMethod::Plc);
        assert_eq!(did.method_specific_id(), "re3ebnp5v7ffagz6rb6xfei4");
        assert_eq!(did.to_string(), PLC);
    }

    #[test]
    fn accepts_web() {
        let did = Did::parse("did:web:example.com").unwrap();
        assert_eq!(did.method(), DidMethod::Web);
        assert_eq!(did.method_specific_id(), "example.com");
    }

    #[test]
    fn rejects_malformed() {
        let cases = [
            ("", DidError::Empty),
            ("plc:abc", DidError::MissingScheme),
            ("did:plc", DidError::UnsupportedMethod("plc".into())),
            ("did:key:z6Mk", DidError::UnsupportedMethod("key".into())),
            ("did:plc:tooshort", DidError::InvalidPlcId),
            ("did:plc:RE3EBNP5V7FFAGZ6RB6XFEI4", DidError::InvalidPlcId),
            ("did:plc:re3ebnp5v7ffagz6rb6xfei1", DidError::InvalidPlcId),
            ("did:web:example.com:8080", DidError::WebPortOrPath),
            ("did:web:example.com%3A8080", DidError::WebPortOrPath),
            ("did:web:example.com:path", DidError::WebPortOrPath),
        ];
        for (input, expected) in cases {
            assert_eq!(Did::parse(input), Err(expected), "input {input:?}");
        }
        assert!(matches!(
            Did::parse("did:web:-bad.example"),
            Err(DidError::InvalidWebHost(_))
        ));
        let long = format!("did:web:{}.example", "a".repeat(MAX_DID_LEN));
        assert_eq!(Did::parse(&long), Err(DidError::TooLong));
    }

    #[test]
    fn serde_round_trip_validates() {
        let did: Did = serde_json::from_str(&format!("\"{PLC}\"")).unwrap();
        assert_eq!(did.as_str(), PLC);
        assert!(serde_json::from_str::<Did>("\"did:key:abc\"").is_err());
        assert_eq!(serde_json::to_string(&did).unwrap(), format!("\"{PLC}\""));
    }
}
