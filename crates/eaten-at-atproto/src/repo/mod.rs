//! Read access to records in a PDS repository, and app-password writes.

pub mod write;

use reqwest::StatusCode;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::at_uri::AtUri;
use crate::http::{GuardedClient, HttpError};
use crate::identity::Did;

/// Largest response body we read from a PDS, in bytes. `listRecords` pages
/// of long documents are the biggest legitimate case.
const MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
/// Largest `limit` a PDS accepts for `listRecords`.
pub const MAX_LIST_LIMIT: u32 = 100;

/// Errors from repository reads.
#[derive(Debug, thiserror::Error)]
pub enum RepoError {
    #[error("record not found")]
    RecordNotFound,
    #[error("repository not found")]
    RepoNotFound,
    #[error("PDS returned {status} {error}: {message}")]
    Xrpc {
        status: StatusCode,
        error: String,
        message: String,
    },
    #[error("could not decode response from {url}: {reason}")]
    Decode { url: Url, reason: String },
    #[error(transparent)]
    Http(#[from] HttpError),
}

/// A record together with its address and content hash.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record<T> {
    pub uri: AtUri,
    pub cid: String,
    pub value: T,
}

impl<T> Record<T> {
    /// The record key, from the URI.
    pub fn rkey(&self) -> &str {
        self.uri.rkey()
    }

    /// Re-type the value, keeping the address and CID.
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Record<U> {
        Record {
            uri: self.uri,
            cid: self.cid,
            value: f(self.value),
        }
    }
}

/// One page of a `listRecords` call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPage<T> {
    pub records: Vec<Record<T>>,
    /// Pass back as `cursor` to fetch the next page. `None` on the last page.
    pub cursor: Option<String>,
    /// Records on this page that did not decode as `T` and were skipped.
    pub skipped: usize,
}

/// Parameters for `listRecords`.
#[derive(Debug, Clone, Default)]
pub struct ListParams {
    /// Page size, clamped to [`MAX_LIST_LIMIT`].
    pub limit: Option<u32>,
    pub cursor: Option<String>,
    /// Oldest first. The default (`false`) is newest first, which is what a
    /// publication listing wants.
    pub reverse: bool,
}

#[derive(Deserialize)]
struct XrpcErrorBody {
    #[serde(default)]
    error: String,
    #[serde(default)]
    message: String,
}

#[derive(Deserialize)]
struct RawListPage {
    records: Vec<Record<serde_json::Value>>,
    cursor: Option<String>,
}

/// Read-only client for one PDS. Cheap to clone.
#[derive(Debug, Clone)]
pub struct RepoClient {
    http: GuardedClient,
    pds: Url,
}

impl RepoClient {
    /// Create a client for the PDS at `pds` (scheme policy is the caller's).
    pub fn new(http: GuardedClient, pds: Url) -> Self {
        Self { http, pds }
    }

    /// The PDS this client talks to.
    pub fn pds(&self) -> &Url {
        &self.pds
    }

    /// `com.atproto.repo.getRecord`.
    pub async fn get_record<T: DeserializeOwned>(
        &self,
        did: &Did,
        collection: &str,
        rkey: &str,
    ) -> Result<Record<T>, RepoError> {
        let url = self.xrpc(
            "com.atproto.repo.getRecord",
            &[
                ("repo", did.as_str()),
                ("collection", collection),
                ("rkey", rkey),
            ],
        );
        self.get_json(url).await
    }

    /// `com.atproto.repo.getRecord` by AT-URI.
    pub async fn get_record_at<T: DeserializeOwned>(
        &self,
        uri: &AtUri,
    ) -> Result<Record<T>, RepoError> {
        self.get_record(uri.did(), uri.collection(), uri.rkey())
            .await
    }

    /// `com.atproto.repo.listRecords`. Records that fail to decode as `T`
    /// are counted in [`ListPage::skipped`] rather than failing the page.
    pub async fn list_records<T: DeserializeOwned>(
        &self,
        did: &Did,
        collection: &str,
        params: &ListParams,
    ) -> Result<ListPage<T>, RepoError> {
        let limit = params
            .limit
            .unwrap_or(50)
            .clamp(1, MAX_LIST_LIMIT)
            .to_string();
        let mut query = vec![
            ("repo", did.as_str()),
            ("collection", collection),
            ("limit", limit.as_str()),
        ];
        if let Some(cursor) = &params.cursor {
            query.push(("cursor", cursor.as_str()));
        }
        if params.reverse {
            query.push(("reverse", "true"));
        }
        let url = self.xrpc("com.atproto.repo.listRecords", &query);
        let raw: RawListPage = self.get_json(url).await?;

        let mut skipped = 0;
        let records = raw
            .records
            .into_iter()
            .filter_map(|record| match serde_json::from_value::<T>(record.value) {
                Ok(value) => Some(Record {
                    uri: record.uri,
                    cid: record.cid,
                    value,
                }),
                Err(err) => {
                    tracing::debug!(uri = %record.uri, %err, "skipping undecodable record");
                    skipped += 1;
                    None
                }
            })
            .collect();
        Ok(ListPage {
            records,
            cursor: raw.cursor,
            skipped,
        })
    }

    /// URL of a blob in `did`'s repo, for the cover-art proxy to fetch.
    pub fn blob_url(&self, did: &Did, cid: &str) -> Url {
        self.xrpc(
            "com.atproto.sync.getBlob",
            &[("did", did.as_str()), ("cid", cid)],
        )
    }

    fn xrpc(&self, method: &str, query: &[(&str, &str)]) -> Url {
        let mut url = self.pds.clone();
        url.path_segments_mut()
            .expect("PDS URL is absolute")
            .pop_if_empty()
            .extend(["xrpc", method]);
        url.query_pairs_mut().extend_pairs(query);
        url
    }

    async fn get_json<T: DeserializeOwned>(&self, url: Url) -> Result<T, RepoError> {
        let response = self.http.get_limited(url, MAX_RESPONSE_BYTES).await?;
        if !response.status.is_success() {
            return Err(xrpc_error(response.status, &response.body));
        }
        response.json().map_err(|e| RepoError::Decode {
            url: response.url,
            reason: e.to_string(),
        })
    }
}

/// Map an XRPC error response to a typed error. PDS implementations differ
/// on status codes for a missing record, so both the `error` name and a
/// plain 404 are treated as not-found.
pub(crate) fn xrpc_error(status: StatusCode, body: &[u8]) -> RepoError {
    let parsed = serde_json::from_slice::<XrpcErrorBody>(body).unwrap_or(XrpcErrorBody {
        error: String::new(),
        message: String::new(),
    });
    match (status, parsed.error.as_str()) {
        (_, "RecordNotFound") | (StatusCode::NOT_FOUND, "") => RepoError::RecordNotFound,
        (_, "RepoNotFound") => RepoError::RepoNotFound,
        _ => RepoError::Xrpc {
            status,
            error: parsed.error,
            message: parsed.message,
        },
    }
}
