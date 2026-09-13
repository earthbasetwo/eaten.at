//! The OAuth client against a mock PLC directory, PDS, and authorization
//! server: the whole flow, a restart, a token refresh, and the ways a
//! callback can be wrong.

use std::sync::Arc;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use eaten_at_atproto::http::{GuardedClient, Policy, StaticHosts};
use eaten_at_atproto::identity::{Did, IdentityConfig, IdentityResolver, StaticDns};
use eaten_at_atproto::oauth::{
    CallbackParams, ClientConfig, Kind, MemoryStore, OAuthClient, OAuthError, OAuthStore,
    SigningKey,
};
use serde_json::{json, Value};
use url::Url;
use wiremock::matchers::{body_string_contains, header, method, path};
use wiremock::{Match, Mock, MockServer, Request, ResponseTemplate};

const DID: &str = "did:plc:re3ebnp5v7ffagz6rb6xfei4";
const HANDLE: &str = "alice.test";
const NONCE: &str = "server-nonce-1";

/// Matches requests whose `DPoP` proof does (or does not) carry a nonce.
struct DpopNonce(bool);

impl Match for DpopNonce {
    fn matches(&self, request: &Request) -> bool {
        let proof = request
            .headers
            .get("dpop")
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        let payload = proof
            .split('.')
            .nth(1)
            .and_then(|p| URL_SAFE_NO_PAD.decode(p).ok())
            .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
        let has_nonce = payload.is_some_and(|p| p.get("nonce").is_some_and(|n| n == NONCE));
        has_nonce == self.0
    }
}

/// Whether the first token exchange returns an already-expired token.
struct Scenario {
    expired_on_login: bool,
}

async fn mount(server: &MockServer, scenario: &Scenario) {
    mount_discovery(server).await;
    mount_tokens(server, scenario).await;
}

/// The PLC directory, the PDS's resource metadata, and the authorization
/// server's metadata.
async fn mount_discovery(server: &MockServer) {
    let uri = server.uri();
    Mock::given(method("GET"))
        .and(path(format!("/{DID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": DID, "alsoKnownAs": [format!("at://{HANDLE}")],
            "service": [{"id": "#atproto_pds", "type": "AtprotoPersonalDataServer", "serviceEndpoint": uri}]
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-protected-resource"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "resource": uri, "authorization_servers": [uri], "scopes_supported": []
        })))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": uri,
            "authorization_endpoint": format!("{uri}/oauth/authorize"),
            "token_endpoint": format!("{uri}/oauth/token"),
            "revocation_endpoint": format!("{uri}/oauth/revoke"),
            "pushed_authorization_request_endpoint": format!("{uri}/oauth/par"),
            "require_pushed_authorization_requests": true,
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code", "refresh_token"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["none", "private_key_jwt"],
            "token_endpoint_auth_signing_alg_values_supported": ["ES256"],
            "dpop_signing_alg_values_supported": ["ES256"],
            "authorization_response_iss_parameter_supported": true,
            "client_id_metadata_document_supported": true,
            "scopes_supported": ["atproto"],
            "protected_resources": [uri]
        })))
        .mount(server)
        .await;
}

/// PAR, token, revocation, and the one authenticated resource call.
async fn mount_tokens(server: &MockServer, scenario: &Scenario) {
    // PAR: the first attempt lacks a nonce and is told to retry with one.
    Mock::given(method("POST"))
        .and(path("/oauth/par"))
        .and(DpopNonce(false))
        .respond_with(
            ResponseTemplate::new(400)
                .insert_header("DPoP-Nonce", NONCE)
                .set_body_json(json!({"error": "use_dpop_nonce"})),
        )
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/par"))
        .and(DpopNonce(true))
        .respond_with(
            ResponseTemplate::new(201)
                .insert_header("DPoP-Nonce", NONCE)
                .set_body_json(json!({"request_uri": "urn:ietf:params:oauth:request_uri:abc", "expires_in": 60})),
        )
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=authorization_code"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "tok1", "token_type": "DPoP", "refresh_token": "rt1",
            "expires_in": if scenario.expired_on_login { 0 } else { 3600 },
            "scope": "atproto", "sub": DID
        })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=rt1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "tok2", "token_type": "DPoP", "refresh_token": "rt2",
            "expires_in": 3600, "scope": "atproto", "sub": DID
        })))
        .mount(server)
        .await;
    // The real provider answers revocation with 200 and `{}`.
    Mock::given(method("POST"))
        .and(path("/oauth/revoke"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(server)
        .await;
    // The resource server: tok1 is rejected once expired, tok2 is good.
    let session_body = json!({"did": DID, "handle": HANDLE});
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.server.getSession"))
        .and(header("authorization", "DPoP tok1"))
        .respond_with(if scenario.expired_on_login {
            ResponseTemplate::new(401)
                .insert_header("WWW-Authenticate", "DPoP error=\"invalid_token\"")
                .set_body_json(json!({"error": "InvalidToken", "message": "expired"}))
        } else {
            ResponseTemplate::new(200).set_body_json(session_body.clone())
        })
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.server.getSession"))
        .and(header("authorization", "DPoP tok2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(session_body))
        .mount(server)
        .await;
}

fn config(public_url: &str, key: Option<SigningKey>) -> ClientConfig {
    ClientConfig {
        public_url: Url::parse(public_url).unwrap(),
        client_name: "Test".into(),
        scopes: vec![
            "atproto".into(),
            "repo:site.standard.document".into(),
            "blob:image/*".into(),
        ],
        signing_key: key,
    }
}

fn client(server: &MockServer, store: Arc<dyn OAuthStore>, config: ClientConfig) -> OAuthClient {
    let http = GuardedClient::new(
        Policy::for_tests(),
        Arc::new(StaticHosts::new().with(HANDLE, *server.address())),
        "eaten-at-tests",
    )
    .unwrap();
    let dns = StaticDns::new().with_txt(&format!("_atproto.{HANDLE}"), &[&format!("did={DID}")]);
    let identity = IdentityResolver::new(
        http.clone(),
        Arc::new(dns),
        IdentityConfig {
            plc_directory: Url::parse(&server.uri()).unwrap(),
        },
    );
    OAuthClient::new(config, http, Arc::new(identity), store).unwrap()
}

/// The PAR carried the scopes verbatim and used the loopback redirect.
async fn assert_par_body(server: &MockServer) {
    let requests = server.received_requests().await.unwrap();
    let par = requests
        .iter()
        .find(|r| r.url.path() == "/oauth/par")
        .unwrap();
    let body = String::from_utf8_lossy(&par.body).into_owned();
    assert!(
        body.contains("&scope=atproto+repo%3Asite.standard.document+blob%3Aimage%2F*&"),
        "{body}"
    );
    assert!(
        body.contains("redirect_uri=http%3A%2F%2F127.0.0.1%3A3000%2Foauth%2Fcallback"),
        "{body}"
    );
    assert!(body.contains("login_hint=alice.test"), "{body}");
}

/// The `state` the client sent in its pushed authorization request.
async fn par_state(server: &MockServer) -> String {
    let requests = server.received_requests().await.unwrap();
    let par = requests
        .iter()
        .filter(|r| r.url.path() == "/oauth/par")
        .next_back()
        .expect("a PAR request was made");
    let body = String::from_utf8_lossy(&par.body).into_owned();
    url::form_urlencoded::parse(body.as_bytes())
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .expect("PAR carried a state")
}

#[tokio::test]
async fn login_callback_session_and_logout() {
    let server = MockServer::start().await;
    mount(
        &server,
        &Scenario {
            expired_on_login: false,
        },
    )
    .await;
    let store: Arc<dyn OAuthStore> = Arc::new(MemoryStore::new());
    let client = client(
        &server,
        Arc::clone(&store),
        config("http://127.0.0.1:3000", None),
    );

    let url = client
        .login_url("@Alice.Test", Some("/write".into()))
        .await
        .unwrap();
    assert!(
        url.as_str()
            .starts_with(&format!("{}/oauth/authorize?", server.uri())),
        "{url}"
    );
    let query: Vec<(String, String)> = url.query_pairs().into_owned().collect();
    assert!(query
        .iter()
        .any(|(k, v)| k == "client_id" && v.starts_with("http://localhost?")));
    assert!(query
        .iter()
        .any(|(k, v)| k == "request_uri" && v == "urn:ietf:params:oauth:request_uri:abc"));

    assert_par_body(&server).await;

    let state = par_state(&server).await;
    let done = client
        .callback(CallbackParams {
            code: Some("code-1".into()),
            state: Some(state.clone()),
            iss: Some(server.uri()),
            ..CallbackParams::default()
        })
        .await
        .unwrap();
    assert_eq!(done.did.as_str(), DID);
    assert_eq!(done.app_state.as_deref(), Some("/write"));
    assert!(
        store.get(Kind::State, &state).await.unwrap().is_none(),
        "state is single-use"
    );
    let stored = store.get(Kind::Session, DID).await.unwrap().unwrap();
    assert!(String::from_utf8_lossy(&stored).contains("tok1"));

    // Replaying the callback fails: the state is gone.
    let replay = client
        .callback(CallbackParams {
            code: Some("code-1".into()),
            state: Some(state),
            iss: Some(server.uri()),
            ..CallbackParams::default()
        })
        .await;
    assert!(matches!(replay, Err(OAuthError::Library(_))), "{replay:?}");

    let did = Did::parse(DID).unwrap();
    let session = client.session(&did).await.unwrap();
    assert_eq!(session.pds(), server.uri());
    let who: Value = session
        .query("com.atproto.server.getSession", &[])
        .await
        .unwrap();
    assert_eq!(who["handle"], HANDLE);
    let get_session = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.url.path() == "/xrpc/com.atproto.server.getSession")
        .unwrap();
    assert!(
        get_session.headers.contains_key("dpop"),
        "resource requests carry a DPoP proof"
    );

    client.logout(&did).await.unwrap();
    assert!(matches!(
        client.session(&did).await,
        Err(OAuthError::NoSession(_))
    ));
    assert!(server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .any(|r| r.url.path() == "/oauth/revoke"));
}

#[tokio::test]
async fn session_survives_a_restart_and_refreshes_an_expired_token() {
    let server = MockServer::start().await;
    mount(
        &server,
        &Scenario {
            expired_on_login: true,
        },
    )
    .await;
    let store: Arc<dyn OAuthStore> = Arc::new(MemoryStore::new());
    let first = client(
        &server,
        Arc::clone(&store),
        config("http://127.0.0.1:3000", None),
    );
    first.login_url(HANDLE, None).await.unwrap();
    let state = par_state(&server).await;
    first
        .callback(CallbackParams {
            code: Some("code-1".into()),
            state: Some(state),
            iss: Some(server.uri()),
            ..CallbackParams::default()
        })
        .await
        .unwrap();
    drop(first);

    // A new process: same store, fresh client.
    let second = client(
        &server,
        Arc::clone(&store),
        config("http://127.0.0.1:3000", None),
    );
    let did = Did::parse(DID).unwrap();
    let session = second.session(&did).await.unwrap();
    let who: Value = session
        .query("com.atproto.server.getSession", &[])
        .await
        .unwrap();
    assert_eq!(who["did"], DID);
    let stored = store.get(Kind::Session, DID).await.unwrap().unwrap();
    let stored = String::from_utf8_lossy(&stored);
    assert!(
        stored.contains("tok2") && !stored.contains("tok1"),
        "refreshed tokens are persisted: {stored}"
    );
}

#[tokio::test]
async fn callbacks_that_cannot_be_trusted_fail() {
    let server = MockServer::start().await;
    mount(
        &server,
        &Scenario {
            expired_on_login: false,
        },
    )
    .await;
    let client = client(
        &server,
        Arc::new(MemoryStore::new()),
        config("http://127.0.0.1:3000", None),
    );
    let denied = client
        .callback(CallbackParams {
            error: Some("access_denied".into()),
            error_description: Some("the user said no".into()),
            ..CallbackParams::default()
        })
        .await;
    assert!(
        matches!(denied, Err(OAuthError::Denied { ref error, .. }) if error == "access_denied"),
        "{denied:?}"
    );
    let no_code = client
        .callback(CallbackParams {
            state: Some("s".into()),
            ..CallbackParams::default()
        })
        .await;
    assert!(
        matches!(no_code, Err(OAuthError::Denied { .. })),
        "{no_code:?}"
    );
    let unknown_state = client
        .callback(CallbackParams {
            code: Some("c".into()),
            state: Some("never-issued".into()),
            iss: Some(server.uri()),
            ..CallbackParams::default()
        })
        .await;
    assert!(
        matches!(unknown_state, Err(OAuthError::Library(_))),
        "{unknown_state:?}"
    );
    assert!(matches!(
        client.login_url("not a handle", None).await,
        Err(OAuthError::InvalidInput(_))
    ));
    assert!(matches!(
        client
            .session(&Did::parse("did:plc:aaaaaaaaaaaaaaaaaaaaaaaa").unwrap())
            .await,
        Err(OAuthError::NoSession(_))
    ));
}

#[tokio::test]
async fn production_metadata_describes_a_confidential_client() {
    let server = MockServer::start().await;
    let key = SigningKey::generate();
    let confidential = client(
        &server,
        Arc::new(MemoryStore::new()),
        config("https://eaten.at", Some(key.clone())),
    );
    assert_eq!(
        confidential.client_id(),
        "https://eaten.at/client-metadata.json"
    );
    let doc = confidential.metadata_document();
    assert_eq!(doc["client_id"], "https://eaten.at/client-metadata.json");
    assert_eq!(doc["client_uri"], "https://eaten.at");
    assert_eq!(doc["client_name"], "Test");
    assert_eq!(doc["application_type"], "web");
    assert_eq!(
        doc["redirect_uris"],
        json!(["https://eaten.at/oauth/callback"])
    );
    assert_eq!(
        doc["scope"],
        "atproto repo:site.standard.document blob:image/*"
    );
    assert_eq!(
        doc["grant_types"],
        json!(["authorization_code", "refresh_token"])
    );
    assert_eq!(doc["response_types"], json!(["code"]));
    assert_eq!(doc["token_endpoint_auth_method"], "private_key_jwt");
    assert_eq!(doc["token_endpoint_auth_signing_alg"], "ES256");
    assert_eq!(doc["dpop_bound_access_tokens"], true);
    let keys = doc["jwks"]["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0]["kid"], key.kid());
    assert!(
        keys[0].get("d").is_none(),
        "the private half stays private: {doc}"
    );

    let public = client(
        &server,
        Arc::new(MemoryStore::new()),
        config("https://eaten.at", None),
    );
    let doc = public.metadata_document();
    assert_eq!(doc["token_endpoint_auth_method"], "none");
    assert!(doc.get("jwks").is_none());

    let http = GuardedClient::new(Policy::for_tests(), Arc::new(StaticHosts::new()), "t").unwrap();
    let identity = IdentityResolver::new(
        http.clone(),
        Arc::new(StaticDns::new()),
        IdentityConfig::default(),
    );
    let no_atproto = OAuthClient::new(
        ClientConfig {
            scopes: vec!["repo:x.y.z".into()],
            ..config("https://eaten.at", None)
        },
        http,
        Arc::new(identity),
        Arc::new(MemoryStore::new()),
    );
    assert!(matches!(no_atproto, Err(OAuthError::Metadata(_))));
}
