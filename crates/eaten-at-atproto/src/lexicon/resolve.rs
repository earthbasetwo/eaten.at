//! Resolve an NSID to its published schema the way a third-party client
//! does (plan §4.5): DNS TXT at `_lexicon.<authority domain>`, DID → PDS,
//! then the `com.atproto.lexicon.schema` record keyed by the NSID.
//!
//! The app never needs this for its own schemas; it exists so the publish
//! tool can prove that what we published is what the world will see.

use serde_json::Value;

use super::schema::{authority_domain, is_nsid, SCHEMA_COLLECTION};
use crate::identity::{Did, DnsResolver, IdentityError, IdentityResolver};
use crate::repo::{RepoClient, RepoError};

/// Why an NSID could not be resolved.
#[derive(Debug, thiserror::Error)]
pub enum ResolveError {
    #[error("{0:?} is not an NSID")]
    NotNsid(String),
    #[error("no `_lexicon` TXT record at {0}")]
    NoDnsRecord(String),
    #[error("`_lexicon` TXT record at {0} names conflicting DIDs")]
    Ambiguous(String),
    #[error("DNS lookup failed: {0}")]
    Dns(String),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("schema record for {0} not found in the authority's repo")]
    NoRecord(String),
    #[error(transparent)]
    Repo(#[from] RepoError),
}

/// The DNS name carrying the authority's `did=` record.
pub fn lexicon_dns_name(nsid: &str) -> Option<String> {
    authority_domain(nsid).map(|d| format!("_lexicon.{d}"))
}

/// Find the DID that publishes schemas for `nsid`'s authority.
pub async fn authority_did(dns: &dyn DnsResolver, nsid: &str) -> Result<Did, ResolveError> {
    if !is_nsid(nsid) {
        return Err(ResolveError::NotNsid(nsid.to_owned()));
    }
    let name = lexicon_dns_name(nsid).ok_or_else(|| ResolveError::NotNsid(nsid.to_owned()))?;
    let values = dns
        .lookup_txt(&name)
        .await
        .map_err(|e| ResolveError::Dns(e.to_string()))?;
    let mut dids: Vec<Did> = values
        .iter()
        .filter_map(|v| v.trim().strip_prefix("did="))
        .filter_map(|raw| Did::parse(raw.trim()).ok())
        .collect();
    dids.dedup();
    match dids.len() {
        0 => Err(ResolveError::NoDnsRecord(name)),
        1 => Ok(dids.remove(0)),
        _ => Err(ResolveError::Ambiguous(name)),
    }
}

/// Fetch the published schema for `nsid`, as the record value without
/// its `$type`.
pub async fn resolve_lexicon(
    identity: &IdentityResolver,
    dns: &dyn DnsResolver,
    nsid: &str,
) -> Result<Value, ResolveError> {
    let did = authority_did(dns, nsid).await?;
    let resolved = identity.resolve_did(&did).await?;
    let repo = RepoClient::new(identity.http().clone(), resolved.pds.clone());
    let record = repo
        .get_record::<Value>(&did, SCHEMA_COLLECTION, nsid)
        .await
        .map_err(|e| match e {
            RepoError::RecordNotFound => ResolveError::NoRecord(nsid.to_owned()),
            other => ResolveError::Repo(other),
        })?;
    let mut value = record.value;
    if let Some(object) = value.as_object_mut() {
        object.remove("$type");
    }
    Ok(value)
}
