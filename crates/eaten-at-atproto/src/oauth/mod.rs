//! Signing in with an AT Protocol account (plan §5.4, §4.4).
//!
//! The flow is atproto OAuth: pushed authorization request, PKCE,
//! DPoP-bound tokens, refresh. `atrium-oauth` does that work
//! (`docs/oauth-eval.md`); this module is the small interface the
//! application sees, so the library can be swapped without touching
//! routes, and the adapters that route the library's HTTP, identity
//! resolution, and storage through ours.
//!
//! What the application does with it: [`OAuthClient::login_url`] to start
//! a sign-in, [`OAuthClient::callback`] when the authorization server
//! sends the user back, [`OAuthClient::session`] to make authenticated
//! requests as a signed-in DID, and [`OAuthClient::logout`].

mod http;
mod key;
mod resolve;
mod store;

use std::fmt;
use std::sync::Arc;

use atrium_api::types::string::Did as AtriumDid;
use atrium_oauth::{
    AtprotoClientMetadata, AtprotoLocalhostClientMetadata, AuthMethod, AuthorizeOptions, GrantType,
    KnownScope, OAuthClientConfig, OAuthResolverConfig, Scope,
};
use atrium_xrpc::http::Method;
use atrium_xrpc::{InputDataOrBytes, OutputDataOrBytes, XrpcClient, XrpcRequest};
use serde::de::DeserializeOwned;
use serde::Serialize;
use url::Url;

use crate::http::GuardedClient;
use crate::identity::{Did, Handle};
use crate::lexicon::BlobRef;
use crate::repo::write::WriteReceipt;

pub use key::{KeyError, SigningKey};
pub use resolve::{IdentityFuture, IdentitySource};
pub use store::{Kind, MemoryStore, OAuthStore, StoreError, StoreFuture, STATE_TTL};

use self::http::Adapter;
use self::resolve::{DidResolverAdapter, HandleResolverAdapter};
use self::store::{SessionStoreAdapter, StateStoreAdapter};

/// Path of the client metadata document on our origin.
pub const METADATA_PATH: &str = "/client-metadata.json";
/// Path the authorization server sends the user back to.
pub const CALLBACK_PATH: &str = "/oauth/callback";

type Inner = atrium_oauth::OAuthClient<
    StateStoreAdapter,
    SessionStoreAdapter,
    DidResolverAdapter,
    HandleResolverAdapter,
    Adapter,
>;
type InnerSession = atrium_oauth::OAuthSession<
    Adapter,
    DidResolverAdapter,
    HandleResolverAdapter,
    SessionStoreAdapter,
>;

/// Everything that fixes the client's identity towards authorization
/// servers.
#[derive(Debug, Clone)]
pub struct ClientConfig {
    /// Our origin. The client id, redirect URI, and metadata URL derive
    /// from it. A plain-HTTP loopback address (`http://127.0.0.1:3000`)
    /// makes this a *loopback client* per the atproto spec: no metadata
    /// document is fetched and the id is `http://localhost?…`.
    pub public_url: Url,
    /// Shown on the consent screen.
    pub client_name: String,
    /// Every scope the client may ever ask for, exactly as they go on the
    /// wire (§4.4). This is what the client metadata declares; a sign-in
    /// may request a subset ([`OAuthClient::login_url_scoped`]) and ask
    /// for more later (§5.7).
    pub scopes: Vec<String>,
    /// With a key the client authenticates as confidential
    /// (`private_key_jwt`); without one it is public.
    pub signing_key: Option<SigningKey>,
}

impl ClientConfig {
    fn base(&self) -> String {
        self.public_url.as_str().trim_end_matches('/').to_owned()
    }

    /// Whether this configuration is a loopback (development) client.
    pub fn is_loopback(&self) -> bool {
        self.public_url.scheme() == "http"
            && matches!(
                self.public_url.host(),
                Some(url::Host::Ipv4(ip)) if ip.is_loopback()
            )
            || matches!(self.public_url.host(), Some(url::Host::Ipv6(ip)) if ip.is_loopback())
    }

    pub fn redirect_uri(&self) -> String {
        format!("{}{CALLBACK_PATH}", self.base())
    }

    fn scopes(&self) -> Vec<Scope> {
        wire_scopes(&self.scopes)
    }
}

fn wire_scopes(scopes: &[String]) -> Vec<Scope> {
    scopes
        .iter()
        .map(|s| match s.as_str() {
            "atproto" => Scope::Known(KnownScope::Atproto),
            other => Scope::Unknown(other.to_owned()),
        })
        .collect()
}

/// What the authorization server sent to the callback URL.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Deserialize)]
pub struct CallbackParams {
    pub code: Option<String>,
    pub state: Option<String>,
    pub iss: Option<String>,
    pub error: Option<String>,
    pub error_description: Option<String>,
}

/// A completed sign-in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Callback {
    pub did: Did,
    /// Whatever the application attached when the login started.
    pub app_state: Option<String>,
}

/// Anything sign-in or a signed-in request can fail with.
#[derive(Debug, thiserror::Error)]
pub enum OAuthError {
    #[error("`{0}` is not a handle, a DID, or an https URL")]
    InvalidInput(String),
    #[error("no AT Protocol account found for `{0}`")]
    AccountNotFound(String),
    #[error("the authorization server refused: {error}")]
    Denied {
        error: String,
        description: Option<String>,
    },
    #[error("no session for {0}")]
    NoSession(Did),
    #[error("the authorization code could not be exchanged for tokens")]
    Exchange,
    #[error("client metadata: {0}")]
    Metadata(String),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error("the session is no longer valid")]
    Unauthenticated,
    #[error("PDS returned {status} {error}: {message}")]
    Xrpc {
        status: u16,
        error: String,
        message: String,
    },
    #[error("request failed: {0}")]
    Transport(String),
    #[error(transparent)]
    Library(#[from] atrium_oauth::Error),
}

/// The OAuth client. Cheap to clone.
#[derive(Clone)]
pub struct OAuthClient {
    inner: Arc<Inner>,
    store: Arc<dyn OAuthStore>,
    config: ClientConfig,
}

impl fmt::Debug for OAuthClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OAuthClient")
            .field("client_id", &self.client_id())
            .field("store", &self.store)
            .finish_non_exhaustive()
    }
}

impl OAuthClient {
    /// Build a client. Fails only when the configuration cannot describe
    /// a valid atproto client (a malformed public URL, no `atproto` scope).
    pub fn new(
        config: ClientConfig,
        http: GuardedClient,
        identity: Arc<dyn IdentitySource>,
        store: Arc<dyn OAuthStore>,
    ) -> Result<Self, OAuthError> {
        if !config.scopes.iter().any(|s| s == "atproto") {
            return Err(OAuthError::Metadata(
                "the `atproto` scope is required".to_owned(),
            ));
        }
        // The metadata cache config types are not exported by the library,
        // so `Default::default()` is the only way to spell them.
        #[allow(clippy::default_trait_access)]
        let resolver = OAuthResolverConfig {
            did_resolver: DidResolverAdapter(Arc::clone(&identity)),
            handle_resolver: HandleResolverAdapter(identity),
            authorization_server_metadata: Default::default(),
            protected_resource_metadata: Default::default(),
        };
        let keys = config.signing_key.as_ref().map(|k| vec![k.jwk()]);
        let state_store = StateStoreAdapter(Arc::clone(&store));
        let session_store = SessionStoreAdapter(Arc::clone(&store));
        let http_client = Adapter(http);
        let inner = if config.is_loopback() {
            Inner::new(OAuthClientConfig {
                client_metadata: AtprotoLocalhostClientMetadata {
                    redirect_uris: Some(vec![config.redirect_uri()]),
                    scopes: Some(config.scopes()),
                },
                keys: None,
                state_store,
                session_store,
                resolver,
                http_client,
            })
        } else {
            let (auth, alg) = if keys.is_some() {
                (AuthMethod::PrivateKeyJwt, Some("ES256".to_owned()))
            } else {
                (AuthMethod::None, None)
            };
            Inner::new(OAuthClientConfig {
                client_metadata: AtprotoClientMetadata {
                    client_id: format!("{}{METADATA_PATH}", config.base()),
                    client_uri: Some(config.base()),
                    redirect_uris: vec![config.redirect_uri()],
                    token_endpoint_auth_method: auth,
                    grant_types: vec![GrantType::AuthorizationCode, GrantType::RefreshToken],
                    scopes: config.scopes(),
                    jwks_uri: None,
                    token_endpoint_auth_signing_alg: alg,
                },
                keys,
                state_store,
                session_store,
                resolver,
                http_client,
            })
        }
        .map_err(|e| OAuthError::Metadata(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(inner),
            store,
            config,
        })
    }

    pub fn client_id(&self) -> &str {
        &self.inner.client_metadata.client_id
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    /// The client metadata document, to serve at [`METADATA_PATH`].
    pub fn metadata_document(&self) -> serde_json::Value {
        // Metadata is plain strings and lists; serializing it cannot fail.
        let mut doc = serde_json::to_value(&self.inner.client_metadata).unwrap_or_default();
        doc["client_name"] = serde_json::Value::String(self.config.client_name.clone());
        doc["application_type"] = serde_json::Value::String("web".to_owned());
        doc["response_types"] = serde_json::json!(["code"]);
        doc
    }

    /// Start a sign-in for a handle, a DID, or (for an account whose
    /// handle does not resolve) an `https://` PDS URL, requesting every
    /// scope the client declares. Returns the URL to send the user to.
    /// `app_state` comes back from [`callback`].
    ///
    /// [`callback`]: Self::callback
    pub async fn login_url(
        &self,
        input: &str,
        app_state: Option<String>,
    ) -> Result<Url, OAuthError> {
        let scopes = self.config.scopes.clone();
        self.login_url_scoped(input, &scopes, app_state).await
    }

    /// [`login_url`](Self::login_url) requesting only `scopes`, which the
    /// authorization server requires to be among the client's declared
    /// ones. A later authorization with a larger set replaces the stored
    /// session for that DID with one carrying the larger grant (§5.7).
    pub async fn login_url_scoped(
        &self,
        input: &str,
        scopes: &[String],
        app_state: Option<String>,
    ) -> Result<Url, OAuthError> {
        let subject = normalize_input(input)?;
        let url = self
            .inner
            .authorize(
                &subject,
                AuthorizeOptions {
                    redirect_uri: None,
                    scopes: wire_scopes(scopes),
                    prompt: None,
                    state: app_state,
                },
            )
            .await
            .map_err(|err| match err {
                atrium_oauth::Error::Identity(atrium_identity::Error::NotFound) => {
                    OAuthError::AccountNotFound(subject.clone())
                }
                other => OAuthError::Library(other),
            })?;
        Url::parse(&url).map_err(|e| OAuthError::Transport(format!("bad authorize URL: {e}")))
    }

    /// Finish a sign-in: exchange the code for tokens and store the
    /// session. The `state` must be one this client minted, so a
    /// forged or replayed callback fails here.
    pub async fn callback(&self, params: CallbackParams) -> Result<Callback, OAuthError> {
        if let Some(error) = params.error {
            return Err(OAuthError::Denied {
                error,
                description: params.error_description,
            });
        }
        let Some(code) = params.code else {
            return Err(OAuthError::Denied {
                error: "invalid_request".to_owned(),
                description: Some("the callback carried no code".to_owned()),
            });
        };
        let inner = Arc::clone(&self.inner);
        let params = atrium_oauth::CallbackParams {
            code,
            state: params.state,
            iss: params.iss,
        };
        // The library panics if the token exchange itself fails (an
        // unfinished branch in 0.1.7). A panic in a spawned task is
        // contained and becomes an error here instead of taking the
        // connection down.
        let outcome = tokio::spawn(async move { inner.callback(params).await }).await;
        let (session, app_state) = match outcome {
            Ok(Ok(result)) => result,
            Ok(Err(err)) => return Err(err.into()),
            Err(err) => {
                tracing::warn!(%err, "token exchange failed");
                return Err(OAuthError::Exchange);
            }
        };
        let did = atrium_api::agent::SessionManager::did(&session)
            .await
            .and_then(|did| Did::parse(did.as_str()).ok())
            .ok_or(OAuthError::Exchange)?;
        Ok(Callback { did, app_state })
    }

    /// The session for a signed-in DID, or [`OAuthError::NoSession`].
    /// Tokens are refreshed on demand when a request finds them expired.
    pub async fn session(&self, did: &Did) -> Result<AuthorizedSession, OAuthError> {
        if self.store.get(Kind::Session, did.as_str()).await?.is_none() {
            return Err(OAuthError::NoSession(did.clone()));
        }
        let inner = self.inner.restore(&atrium_did(did)?).await?;
        Ok(AuthorizedSession {
            inner,
            did: did.clone(),
        })
    }

    /// The scopes the authorization server granted a signed-in DID, as
    /// the token response listed them. Empty when there is no session or
    /// the response named none.
    pub async fn granted_scopes(&self, did: &Did) -> Result<Vec<String>, OAuthError> {
        let Some(bytes) = self.store.get(Kind::Session, did.as_str()).await? else {
            return Ok(Vec::new());
        };
        let session: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
        Ok(session["token_set"]["scope"]
            .as_str()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_owned)
            .collect())
    }

    /// Forget a session, telling the authorization server to revoke its
    /// tokens. Revocation is best effort (RFC 7009 §2.2: a client cannot
    /// act on its failure); the local session is removed regardless.
    pub async fn logout(&self, did: &Did) -> Result<(), OAuthError> {
        if let Err(err) = self.inner.revoke(&atrium_did(did)?).await {
            tracing::debug!(%did, %err, "token revocation did not confirm; dropping session anyway");
        }
        self.store.delete(Kind::Session, did.as_str()).await?;
        Ok(())
    }
}

/// A signed-in DID's access to its own PDS.
pub struct AuthorizedSession {
    inner: InnerSession,
    did: Did,
}

impl fmt::Debug for AuthorizedSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AuthorizedSession")
            .field("did", &self.did)
            .field("pds", &self.pds())
            .finish_non_exhaustive()
    }
}

impl AuthorizedSession {
    pub fn did(&self) -> &Did {
        &self.did
    }

    /// The PDS the tokens are bound to.
    pub fn pds(&self) -> String {
        self.inner.base_uri()
    }

    /// An XRPC query (`GET`) with the given parameters.
    pub async fn query<O: DeserializeOwned + Send + Sync>(
        &self,
        nsid: &str,
        params: &[(&str, &str)],
    ) -> Result<O, OAuthError> {
        let parameters = (!params.is_empty()).then(|| {
            params
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect::<Vec<_>>()
        });
        self.send(XrpcRequest {
            method: Method::GET,
            nsid: nsid.to_owned(),
            parameters,
            input: None::<InputDataOrBytes<()>>,
            encoding: None,
        })
        .await
    }

    /// An XRPC procedure (`POST`) with a JSON body.
    pub async fn procedure<I: Serialize + Send + Sync, O: DeserializeOwned + Send + Sync>(
        &self,
        nsid: &str,
        input: I,
    ) -> Result<O, OAuthError> {
        self.send(XrpcRequest {
            method: Method::POST,
            nsid: nsid.to_owned(),
            parameters: None::<()>,
            input: Some(InputDataOrBytes::Data(input)),
            encoding: Some("application/json".to_owned()),
        })
        .await
    }

    /// An XRPC procedure with a raw body (`uploadBlob`).
    pub async fn procedure_bytes<O: DeserializeOwned + Send + Sync>(
        &self,
        nsid: &str,
        bytes: Vec<u8>,
        content_type: &str,
    ) -> Result<O, OAuthError> {
        self.send(XrpcRequest {
            method: Method::POST,
            nsid: nsid.to_owned(),
            parameters: None::<()>,
            input: Some(InputDataOrBytes::<()>::Bytes(bytes)),
            encoding: Some(content_type.to_owned()),
        })
        .await
    }

    /// `com.atproto.repo.createRecord`. With no `rkey` the PDS mints a TID.
    pub async fn create_record(
        &self,
        collection: &str,
        rkey: Option<&str>,
        record: &serde_json::Value,
    ) -> Result<WriteReceipt, OAuthError> {
        let mut input = serde_json::json!({
            "repo": self.did.as_str(),
            "collection": collection,
            "record": record,
        });
        if let Some(rkey) = rkey {
            input["rkey"] = serde_json::Value::String(rkey.to_owned());
        }
        self.procedure("com.atproto.repo.createRecord", input).await
    }

    /// `com.atproto.repo.putRecord`: create or replace.
    pub async fn put_record(
        &self,
        collection: &str,
        rkey: &str,
        record: &serde_json::Value,
    ) -> Result<WriteReceipt, OAuthError> {
        self.procedure(
            "com.atproto.repo.putRecord",
            serde_json::json!({
                "repo": self.did.as_str(),
                "collection": collection,
                "rkey": rkey,
                "record": record,
            }),
        )
        .await
    }

    /// `com.atproto.repo.deleteRecord`.
    pub async fn delete_record(&self, collection: &str, rkey: &str) -> Result<(), OAuthError> {
        let _: serde_json::Value = self
            .procedure(
                "com.atproto.repo.deleteRecord",
                serde_json::json!({
                    "repo": self.did.as_str(),
                    "collection": collection,
                    "rkey": rkey,
                }),
            )
            .await?;
        Ok(())
    }

    /// `com.atproto.repo.uploadBlob`. The blob is only kept once a record
    /// references it.
    pub async fn upload_blob(&self, bytes: Vec<u8>, mime: &str) -> Result<BlobRef, OAuthError> {
        #[derive(serde::Deserialize)]
        struct Uploaded {
            blob: BlobRef,
        }
        let uploaded: Uploaded = self
            .procedure_bytes("com.atproto.repo.uploadBlob", bytes, mime)
            .await?;
        Ok(uploaded.blob)
    }

    async fn send<P, I, O>(&self, request: XrpcRequest<P, I>) -> Result<O, OAuthError>
    where
        P: Serialize + Send + Sync,
        I: Serialize + Send + Sync,
        O: DeserializeOwned + Send + Sync,
    {
        match self
            .inner
            .send_xrpc::<P, I, O, serde_json::Value>(&request)
            .await
        {
            Ok(OutputDataOrBytes::Data(data)) => Ok(data),
            Ok(OutputDataOrBytes::Bytes(_)) => Err(OAuthError::Transport(format!(
                "{} answered with a non-JSON body",
                request.nsid
            ))),
            Err(atrium_xrpc::Error::Authentication(_)) => Err(OAuthError::Unauthenticated),
            Err(atrium_xrpc::Error::XrpcResponse(err)) => {
                let (error, message) = match err.error {
                    Some(atrium_xrpc::error::XrpcErrorKind::Undefined(body)) => (
                        body.error.unwrap_or_default(),
                        body.message.unwrap_or_default(),
                    ),
                    Some(atrium_xrpc::error::XrpcErrorKind::Custom(value)) => (
                        value["error"].as_str().unwrap_or_default().to_owned(),
                        value["message"].as_str().unwrap_or_default().to_owned(),
                    ),
                    None => (String::new(), String::new()),
                };
                Err(OAuthError::Xrpc {
                    status: err.status.as_u16(),
                    error,
                    message,
                })
            }
            Err(other) => Err(OAuthError::Transport(other.to_string())),
        }
    }
}

/// Helpers for tests in other crates that need a signed-in session
/// without running the sign-in flow.
pub mod testing {
    use super::{Kind, OAuthStore, SigningKey};
    use crate::identity::Did;

    /// Write a session for `did` into `store` as if a sign-in had just
    /// completed: a fresh `DPoP` key and the given access token, bound to
    /// `pds` and issued by `issuer`. Requests made with it carry
    /// `Authorization: DPoP <access_token>`.
    ///
    /// # Panics
    ///
    /// If the store refuses the write; a test's store never does.
    pub async fn seed_session(
        store: &dyn OAuthStore,
        did: &Did,
        issuer: &str,
        pds: &str,
        access_token: &str,
    ) {
        seed_session_with_scope(store, did, issuer, pds, access_token, "atproto").await;
    }

    /// [`seed_session`] with the granted `scope` string spelled out.
    ///
    /// # Panics
    ///
    /// If the store refuses the write; a test's store never does.
    pub async fn seed_session_with_scope(
        store: &dyn OAuthStore,
        did: &Did,
        issuer: &str,
        pds: &str,
        access_token: &str,
        scope: &str,
    ) {
        let jwk: serde_json::Value =
            serde_json::from_str(&SigningKey::generate().to_json()).expect("a JWK");
        let dpop_key = serde_json::json!({
            "kty": jwk["kty"], "crv": jwk["crv"], "x": jwk["x"], "y": jwk["y"], "d": jwk["d"],
        });
        let session = serde_json::json!({
            "dpop_key": dpop_key,
            "token_set": {
                "iss": issuer,
                "sub": did.as_str(),
                "aud": pds,
                "scope": scope,
                "refresh_token": "refresh-token",
                "access_token": access_token,
                "token_type": "DPoP",
                "expires_at": "2099-01-01T00:00:00.000Z",
            }
        });
        store
            .set(
                Kind::Session,
                did.as_str(),
                serde_json::to_vec(&session).expect("JSON"),
                None,
            )
            .await
            .expect("store accepts a session");
    }
}

/// Accept a handle (with or without `@`), a DID, or an https URL.
fn normalize_input(input: &str) -> Result<String, OAuthError> {
    let trimmed = input.trim();
    if trimmed.starts_with("https://") {
        return Url::parse(trimmed)
            .map(|u| u.to_string())
            .map_err(|_| OAuthError::InvalidInput(input.to_owned()));
    }
    if let Ok(did) = Did::parse(trimmed) {
        return Ok(did.as_str().to_owned());
    }
    Handle::parse(trimmed)
        .map(|h| h.as_str().to_owned())
        .map_err(|_| OAuthError::InvalidInput(input.to_owned()))
}

fn atrium_did(did: &Did) -> Result<AtriumDid, OAuthError> {
    AtriumDid::new(did.as_str().to_owned()).map_err(|e| OAuthError::Transport(e.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_forms() {
        assert_eq!(normalize_input(" @Alice.Test ").unwrap(), "alice.test");
        assert_eq!(
            normalize_input("did:plc:re3ebnp5v7ffagz6rb6xfei4").unwrap(),
            "did:plc:re3ebnp5v7ffagz6rb6xfei4"
        );
        assert_eq!(
            normalize_input("https://pds.example").unwrap(),
            "https://pds.example/"
        );
        assert!(matches!(
            normalize_input("not a handle"),
            Err(OAuthError::InvalidInput(_))
        ));
        assert!(matches!(
            normalize_input("http://insecure.example"),
            Err(OAuthError::InvalidInput(_))
        ));
    }

    #[test]
    fn loopback_is_plain_http_to_a_loopback_ip() {
        let config = |url: &str| ClientConfig {
            public_url: Url::parse(url).unwrap(),
            client_name: String::new(),
            scopes: vec![],
            signing_key: None,
        };
        assert!(config("http://127.0.0.1:3000/").is_loopback());
        assert!(config("http://[::1]:3000").is_loopback());
        assert!(!config("http://localhost:3000").is_loopback());
        assert!(!config("https://127.0.0.1").is_loopback());
        assert!(!config("https://eaten.at").is_loopback());
        assert_eq!(
            config("https://eaten.at/").redirect_uri(),
            "https://eaten.at/oauth/callback"
        );
    }
}
