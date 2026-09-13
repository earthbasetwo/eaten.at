//! The client's signing key: an ES256 key that authenticates the app to
//! authorization servers (`private_key_jwt`), making it a confidential
//! client. The key is long-lived and its public half is published in the
//! client metadata.

use std::fmt;

use jose_jwk::{Class, Jwk, Key, Parameters};
use p256::SecretKey;

/// Why a key file could not be used.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum KeyError {
    #[error("signing key is not valid JWK JSON: {0}")]
    Json(String),
    #[error("signing key must be an EC P-256 private key")]
    NotP256Private,
    #[error("signing key must carry a `kid`")]
    MissingKid,
}

/// A P-256 private key with a key id.
#[derive(Clone, PartialEq, Eq)]
pub struct SigningKey(Jwk);

impl SigningKey {
    /// Mint a fresh key with a random `kid`.
    pub fn generate() -> Self {
        let secret = SecretKey::random(&mut rand_core::OsRng);
        let mut kid_bytes = [0u8; 8];
        rand_core::RngCore::fill_bytes(&mut rand_core::OsRng, &mut kid_bytes);
        let kid = kid_bytes.iter().fold(String::new(), |mut out, b| {
            use std::fmt::Write as _;
            let _ = write!(out, "{b:02x}");
            out
        });
        Self(Jwk {
            key: Key::Ec(jose_jwk::Ec::from(&secret)),
            prm: Parameters {
                kid: Some(kid),
                cls: Some(Class::Signing),
                ..Parameters::default()
            },
        })
    }

    /// Parse a private JWK (as written by [`to_json`](Self::to_json)).
    pub fn from_json(json: &str) -> Result<Self, KeyError> {
        let jwk: Jwk = serde_json::from_str(json).map_err(|e| KeyError::Json(e.to_string()))?;
        match &jwk.key {
            Key::Ec(ec) if ec.crv == jose_jwk::EcCurves::P256 && ec.d.is_some() => {}
            _ => return Err(KeyError::NotP256Private),
        }
        if jwk.prm.kid.as_deref().is_none_or(str::is_empty) {
            return Err(KeyError::MissingKid);
        }
        Ok(Self(jwk))
    }

    /// The private key as JWK JSON. Treat the output as a secret.
    pub fn to_json(&self) -> String {
        // A JWK built from a valid key always serializes.
        serde_json::to_string(&self.0).unwrap_or_default()
    }

    pub fn kid(&self) -> &str {
        self.0.prm.kid.as_deref().unwrap_or_default()
    }

    /// The public half, for publishing.
    pub fn public_jwk(&self) -> Jwk {
        let mut jwk = self.0.clone();
        if let Key::Ec(ec) = &mut jwk.key {
            ec.d = None;
        }
        jwk
    }

    pub(super) fn jwk(&self) -> Jwk {
        self.0.clone()
    }
}

impl fmt::Debug for SigningKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SigningKey")
            .field("kid", &self.kid())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_key_round_trips_and_publishes_without_secret() {
        let key = SigningKey::generate();
        assert_eq!(key.kid().len(), 16);
        let json = key.to_json();
        assert!(json.contains("\"d\":"), "{json}");
        let again = SigningKey::from_json(&json).unwrap();
        assert_eq!(again, key);
        let public = serde_json::to_string(&key.public_jwk()).unwrap();
        assert!(!public.contains("\"d\":"), "{public}");
        assert!(public.contains("\"crv\":\"P-256\""), "{public}");
        assert!(!format!("{key:?}").contains("\"d\""));
    }

    #[test]
    fn rejects_public_and_unlabelled_keys() {
        let key = SigningKey::generate();
        let public = serde_json::to_string(&key.public_jwk()).unwrap();
        assert_eq!(
            SigningKey::from_json(&public).unwrap_err(),
            KeyError::NotP256Private
        );
        let mut jwk = key.jwk();
        jwk.prm.kid = None;
        assert_eq!(
            SigningKey::from_json(&serde_json::to_string(&jwk).unwrap()).unwrap_err(),
            KeyError::MissingKid
        );
        assert!(matches!(
            SigningKey::from_json("{not json"),
            Err(KeyError::Json(_))
        ));
    }
}
