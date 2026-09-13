//! Authenticated repository writes with an app password.
//!
//! OAuth is the login story for readers who become authors (Phase 3). App
//! passwords are for project infrastructure: the publish tool writing
//! schema records into the `eaten.at` account from a shell or CI.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use super::{xrpc_error, RepoError};
use crate::http::GuardedClient;
use crate::identity::Did;

/// A session created with `com.atproto.server.createSession`.
#[derive(Debug, Clone)]
pub struct AppPasswordSession {
    http: GuardedClient,
    /// The entry host the session was created on; writes go here too.
    pds: Url,
    pub did: Did,
    access_jwt: String,
}

#[derive(Deserialize)]
struct SessionResponse {
    did: Did,
    #[serde(rename = "accessJwt")]
    access_jwt: String,
}

/// Result of a write: the record's address and content hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteReceipt {
    pub uri: String,
    pub cid: String,
}

impl AppPasswordSession {
    /// Log in. `identifier` is a handle or DID; `password` must be an app
    /// password, never an account password.
    pub async fn login(
        http: GuardedClient,
        pds: Url,
        identifier: &str,
        password: &str,
    ) -> Result<Self, RepoError> {
        #[derive(Serialize)]
        struct Body<'a> {
            identifier: &'a str,
            password: &'a str,
        }
        let url = xrpc(&pds, "com.atproto.server.createSession");
        let response = http
            .post_json(
                url.clone(),
                &Body {
                    identifier,
                    password,
                },
                None,
            )
            .await?;
        if !response.status.is_success() {
            return Err(xrpc_error(response.status, &response.body));
        }
        let session: SessionResponse = response.json().map_err(|e| RepoError::Decode {
            url,
            reason: e.to_string(),
        })?;
        Ok(Self {
            http,
            pds,
            did: session.did,
            access_jwt: session.access_jwt,
        })
    }

    /// `com.atproto.repo.putRecord`: create or replace a record.
    pub async fn put_record(
        &self,
        collection: &str,
        rkey: &str,
        record: &Value,
    ) -> Result<WriteReceipt, RepoError> {
        #[derive(Serialize)]
        struct Body<'a> {
            repo: &'a str,
            collection: &'a str,
            rkey: &'a str,
            record: &'a Value,
        }
        let url = xrpc(&self.pds, "com.atproto.repo.putRecord");
        let response = self
            .http
            .post_json(
                url.clone(),
                &Body {
                    repo: self.did.as_str(),
                    collection,
                    rkey,
                    record,
                },
                Some(&self.access_jwt),
            )
            .await?;
        if !response.status.is_success() {
            return Err(xrpc_error(response.status, &response.body));
        }
        response.json().map_err(|e| RepoError::Decode {
            url,
            reason: e.to_string(),
        })
    }

    /// A read client for the same entry host.
    pub fn reader(&self) -> super::RepoClient {
        super::RepoClient::new(self.http.clone(), self.pds.clone())
    }
}

fn xrpc(pds: &Url, method: &str) -> Url {
    let mut url = pds.clone();
    url.path_segments_mut()
        .expect("PDS URL is absolute")
        .pop_if_empty()
        .extend(["xrpc", method]);
    url
}
