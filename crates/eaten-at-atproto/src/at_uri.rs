//! `at://` URIs naming records in a repository.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::identity::{Did, DidError};

/// Longest AT-URI we accept, in bytes (the spec's limit is 8 KB).
const MAX_AT_URI_LEN: usize = 8 * 1024;
/// Longest record key, in bytes.
const MAX_RKEY_LEN: usize = 512;

/// An `at://<did>/<collection>/<rkey>` URI. Only the DID-authority, full
/// record form is supported; handle authorities and partial URIs are rejected
/// because nothing in this app produces or stores them.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct AtUri {
    raw: String,
    did: Did,
}

/// Why a string is not an acceptable record AT-URI.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AtUriError {
    #[error("AT-URI is longer than {MAX_AT_URI_LEN} bytes")]
    TooLong,
    #[error("AT-URI does not start with `at://`")]
    MissingScheme,
    #[error("AT-URI must have the form at://<did>/<collection>/<rkey>")]
    WrongShape,
    #[error("AT-URI authority is not a DID: {0}")]
    Authority(#[from] DidError),
    #[error("AT-URI collection {0:?} is not a valid NSID")]
    Collection(String),
    #[error("AT-URI record key {0:?} is invalid")]
    Rkey(String),
}

impl AtUri {
    /// Validate and wrap.
    pub fn parse(input: &str) -> Result<Self, AtUriError> {
        if input.len() > MAX_AT_URI_LEN {
            return Err(AtUriError::TooLong);
        }
        let rest = input
            .strip_prefix("at://")
            .ok_or(AtUriError::MissingScheme)?;
        let mut parts = rest.split('/');
        let (Some(authority), Some(collection), Some(rkey), None) =
            (parts.next(), parts.next(), parts.next(), parts.next())
        else {
            return Err(AtUriError::WrongShape);
        };
        let did = Did::parse(authority)?;
        if !is_nsid(collection) {
            return Err(AtUriError::Collection(collection.to_owned()));
        }
        if !is_rkey(rkey) {
            return Err(AtUriError::Rkey(rkey.to_owned()));
        }
        Ok(Self {
            raw: input.to_owned(),
            did,
        })
    }

    /// Build from parts that are already validated.
    pub fn from_parts(did: &Did, collection: &str, rkey: &str) -> Result<Self, AtUriError> {
        Self::parse(&format!("at://{did}/{collection}/{rkey}"))
    }

    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// The repository DID.
    pub fn did(&self) -> &Did {
        &self.did
    }

    /// The collection NSID.
    pub fn collection(&self) -> &str {
        self.segment(1)
    }

    /// The record key.
    pub fn rkey(&self) -> &str {
        self.segment(2)
    }

    /// Path segment after the authority. Validation guarantees exactly two.
    fn segment(&self, index: usize) -> &str {
        self.raw["at://".len()..]
            .split('/')
            .nth(index)
            .unwrap_or_default()
    }
}

/// Loose NSID check: at least three dot-separated labels of `[A-Za-z0-9-]`,
/// no label empty. Enough to reject path tricks; not a full spec validator.
fn is_nsid(s: &str) -> bool {
    let labels: Vec<&str> = s.split('.').collect();
    labels.len() >= 3
        && labels.iter().all(|l| {
            !l.is_empty()
                && l.len() <= 63
                && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// Record keys: 1–512 bytes of `[A-Za-z0-9._:~-]`, and not `.` or `..`.
fn is_rkey(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_RKEY_LEN
        && s != "."
        && s != ".."
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_:~".contains(&b))
}

impl fmt::Debug for AtUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "AtUri({})", self.raw)
    }
}

impl fmt::Display for AtUri {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl FromStr for AtUri {
    type Err = AtUriError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl Serialize for AtUri {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for AtUri {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const URI: &str =
        "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/3lb2c4d5e6f7g";

    #[test]
    fn parses_record_uri() {
        let uri = AtUri::parse(URI).unwrap();
        assert_eq!(uri.did().as_str(), "did:plc:re3ebnp5v7ffagz6rb6xfei4");
        assert_eq!(uri.collection(), "site.standard.publication");
        assert_eq!(uri.rkey(), "3lb2c4d5e6f7g");
        assert_eq!(uri.to_string(), URI);
    }

    #[test]
    fn rejects_bad_uris() {
        let cases = [
            ("https://example.com", AtUriError::MissingScheme),
            (
                "at://did:plc:re3ebnp5v7ffagz6rb6xfei4",
                AtUriError::WrongShape,
            ),
            (
                "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication",
                AtUriError::WrongShape,
            ),
            (&format!("{URI}/extra"), AtUriError::WrongShape),
            (
                "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/notannsid/abc",
                AtUriError::Collection("notannsid".into()),
            ),
            (
                "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/..",
                AtUriError::Rkey("..".into()),
            ),
            (
                "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/a b",
                AtUriError::Rkey("a b".into()),
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(AtUri::parse(input), Err(expected), "input {input:?}");
        }
        assert!(matches!(
            AtUri::parse("at://alice.test/a.b.c/x"),
            Err(AtUriError::Authority(_))
        ));
    }

    #[test]
    fn literal_self_is_a_valid_rkey() {
        let uri = AtUri::parse("at://did:plc:re3ebnp5v7ffagz6rb6xfei4/at.eaten.preferences/self")
            .unwrap();
        assert_eq!(uri.rkey(), "self");
    }
}
