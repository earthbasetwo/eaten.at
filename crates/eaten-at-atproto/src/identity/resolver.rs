//! Handle → DID and DID → DID document resolution.

use std::sync::Arc;

use reqwest::StatusCode;
use url::Url;

use crate::http::{GuardedClient, HttpError};

use super::did::{Did, DidMethod};
use super::dns::DnsResolver;
use super::document::DidDocument;
use super::handle::Handle;

/// Default PLC directory.
const DEFAULT_PLC_DIRECTORY: &str = "https://plc.directory";
/// Path of the HTTPS handle-verification document.
const HANDLE_WELL_KNOWN_PATH: &str = "/.well-known/atproto-did";
/// Path of a `did:web` document.
const DID_WEB_PATH: &str = "/.well-known/did.json";
/// Prefix of a handle TXT record's value.
const DNS_DID_PREFIX: &str = "did=";
/// Largest DID document body we will read, in bytes.
const MAX_DOCUMENT_BYTES: usize = 64 * 1024;

/// Errors from identity resolution.
#[derive(Debug, thiserror::Error)]
pub enum IdentityError {
    #[error("no DID found for handle {0}")]
    HandleNotFound(Handle),
    #[error("handle {0} has conflicting DNS records")]
    AmbiguousDns(Handle),
    #[error("handle {handle} points at {did}, but that DID does not claim the handle")]
    HandleNotClaimed { handle: Handle, did: Did },
    #[error("DID {0} not found")]
    DidNotFound(Did),
    #[error("DID document for {requested} is malformed: {reason}")]
    MalformedDocument { requested: Did, reason: String },
    #[error("DID {0} declares no personal data server")]
    NoPds(Did),
    #[error("endpoint {0} is not https")]
    InsecureEndpoint(Url),
    #[error("upstream {url} responded {status}")]
    UpstreamStatus { url: Url, status: StatusCode },
    #[error(transparent)]
    Http(#[from] HttpError),
}

/// Tunables for [`IdentityResolver`].
#[derive(Debug, Clone)]
pub struct IdentityConfig {
    /// Base URL of the PLC directory.
    pub plc_directory: Url,
}

impl Default for IdentityConfig {
    fn default() -> Self {
        Self {
            plc_directory: Url::parse(DEFAULT_PLC_DIRECTORY).expect("constant URL is valid"),
        }
    }
}

/// A resolved identity: the DID, its document, and the verified PDS endpoint.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Identity {
    pub did: Did,
    pub document: DidDocument,
    /// The PDS base URL, already checked against the scheme policy.
    pub pds: Url,
    /// The handle this DID claims *and* that resolves back to it. `None`
    /// when the document claims no handle or the claim did not verify;
    /// display code should then fall back to the DID.
    pub handle: Option<Handle>,
}

impl Identity {
    /// The handle the DID document claims, verified or not. Prefer
    /// [`Identity::handle`] for anything shown to a reader.
    pub fn claimed_handle(&self) -> Option<Handle> {
        self.document.claimed_handle()
    }

    /// The verified handle, or the DID as text.
    pub fn display_name(&self) -> String {
        self.handle
            .as_ref()
            .map_or_else(|| self.did.to_string(), ToString::to_string)
    }
}

/// Resolves handles and DIDs. Cheap to clone.
#[derive(Debug, Clone)]
pub struct IdentityResolver {
    http: GuardedClient,
    dns: Arc<dyn DnsResolver>,
    config: IdentityConfig,
}

impl IdentityResolver {
    /// Create a resolver over the given HTTP client and DNS backend.
    pub fn new(http: GuardedClient, dns: Arc<dyn DnsResolver>, config: IdentityConfig) -> Self {
        Self { http, dns, config }
    }

    /// The HTTP client this resolver fetches with.
    pub fn http(&self) -> &GuardedClient {
        &self.http
    }

    /// Resolve a DID to its document and PDS endpoint, and verify its
    /// claimed handle (best-effort: a handle that fails to verify leaves
    /// [`Identity::handle`] empty rather than failing the resolution).
    pub async fn resolve_did(&self, did: &Did) -> Result<Identity, IdentityError> {
        let mut identity = self.resolve_did_unverified(did).await?;
        if let Some(claimed) = identity.claimed_handle() {
            match self.did_for_handle(&claimed).await {
                Ok(Some(resolved)) if resolved == *did => identity.handle = Some(claimed),
                Ok(_) => tracing::debug!(%did, %claimed, "claimed handle does not resolve to DID"),
                Err(err) => tracing::debug!(%did, %claimed, %err, "handle verification failed"),
            }
        }
        Ok(identity)
    }

    /// Resolve a DID without checking its claimed handle.
    async fn resolve_did_unverified(&self, did: &Did) -> Result<Identity, IdentityError> {
        let url = match did.method() {
            DidMethod::Plc => self.plc_url(did),
            DidMethod::Web => self.https_url(did.method_specific_id(), DID_WEB_PATH),
        };
        let Some(body) = self.fetch_text(url, MAX_DOCUMENT_BYTES).await? else {
            return Err(IdentityError::DidNotFound(did.clone()));
        };
        let document: DidDocument =
            serde_json::from_str(&body).map_err(|e| IdentityError::MalformedDocument {
                requested: did.clone(),
                reason: e.to_string(),
            })?;
        if &document.id != did {
            return Err(IdentityError::MalformedDocument {
                requested: did.clone(),
                reason: format!("document id is {}", document.id),
            });
        }
        let pds = document
            .pds_endpoint()
            .ok_or_else(|| IdentityError::NoPds(did.clone()))?;
        self.check_scheme(&pds)?;
        Ok(Identity {
            did: did.clone(),
            document,
            pds,
            handle: None,
        })
    }

    /// Resolve a handle to a DID, verify the DID claims the handle, and
    /// return the full identity.
    ///
    /// Order follows the atproto spec: DNS TXT first, then the HTTPS
    /// well-known document. A DNS *failure* (as opposed to an empty answer)
    /// is logged and treated as empty so a flaky resolver does not hide a
    /// working well-known file.
    pub async fn resolve_handle(&self, handle: &Handle) -> Result<Identity, IdentityError> {
        let did = self
            .did_for_handle(handle)
            .await?
            .ok_or_else(|| IdentityError::HandleNotFound(handle.clone()))?;
        let mut identity = self.resolve_did_unverified(&did).await?;
        if !identity.document.claims_handle(handle) {
            return Err(IdentityError::HandleNotClaimed {
                handle: handle.clone(),
                did,
            });
        }
        identity.handle = Some(handle.clone());
        Ok(identity)
    }

    /// Forward resolution only: DNS TXT, then the well-known document.
    async fn did_for_handle(&self, handle: &Handle) -> Result<Option<Did>, IdentityError> {
        if let Some(did) = self.did_from_dns(handle).await? {
            return Ok(Some(did));
        }
        self.did_from_well_known(handle).await
    }

    async fn did_from_dns(&self, handle: &Handle) -> Result<Option<Did>, IdentityError> {
        let name = handle.dns_txt_name();
        let values = match self.dns.lookup_txt(&name).await {
            Ok(values) => values,
            Err(err) => {
                tracing::warn!(%handle, %err, "DNS lookup failed; falling back to well-known");
                return Ok(None);
            }
        };
        let mut dids = values
            .iter()
            .filter_map(|v| v.strip_prefix(DNS_DID_PREFIX))
            .filter_map(|raw| Did::parse(raw.trim()).ok())
            .collect::<Vec<_>>();
        dids.dedup();
        match dids.len() {
            0 => Ok(None),
            1 => Ok(dids.pop()),
            _ => Err(IdentityError::AmbiguousDns(handle.clone())),
        }
    }

    async fn did_from_well_known(&self, handle: &Handle) -> Result<Option<Did>, IdentityError> {
        let url = self.https_url(handle.as_str(), HANDLE_WELL_KNOWN_PATH);
        // Any failure here means "this handle does not verify over HTTPS",
        // which is a not-found, not an upstream error: the host is chosen by
        // whoever typed the handle and may not exist at all.
        let Ok(Some(body)) = self.fetch_text(url, 1024).await else {
            return Ok(None);
        };
        Ok(Did::parse(body.trim()).ok())
    }

    /// Fetch `url` as text. Returns `Ok(None)` on 404.
    async fn fetch_text(
        &self,
        url: Url,
        max_bytes: usize,
    ) -> Result<Option<String>, IdentityError> {
        let response = self.http.get_limited(url, max_bytes).await?;
        if response.status == StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status.is_success() {
            return Err(IdentityError::UpstreamStatus {
                url: response.url,
                status: response.status,
            });
        }
        Ok(Some(response.text().into_owned()))
    }

    /// `{plc_directory}/{did}`. Built by pushing a path segment rather than
    /// `Url::join`, which would parse `did:` as a scheme.
    fn plc_url(&self, did: &Did) -> Url {
        let mut url = self.config.plc_directory.clone();
        url.path_segments_mut()
            .expect("plc_directory is an absolute http(s) URL")
            .pop_if_empty()
            .push(did.as_str());
        url
    }

    fn https_url(&self, host: &str, path: &str) -> Url {
        let scheme = self.http.scheme();
        Url::parse(&format!("{scheme}://{host}{path}")).expect("validated host forms a URL")
    }

    /// A PDS endpoint from a DID document must use a scheme the client
    /// would accept; rejecting it here gives a clearer error than a blocked
    /// fetch later.
    fn check_scheme(&self, url: &Url) -> Result<(), IdentityError> {
        let ok =
            url.scheme() == "https" || (self.http.policy().allow_http && url.scheme() == "http");
        if ok {
            Ok(())
        } else {
            Err(IdentityError::InsecureEndpoint(url.clone()))
        }
    }
}
