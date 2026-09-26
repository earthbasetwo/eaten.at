//! Sign-in routes against a mock PLC directory, PDS, and authorization
//! server: the pages, the cookie, the landing page's account line, and
//! the origin rule.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use eaten_at::app::router;
use eaten_at::cache::{Cache, SystemClock};
use eaten_at::state::{AppConfig, AppState};
use eaten_at_atproto::http::{GuardedClient, Policy, StaticHosts};
use eaten_at_atproto::identity::{IdentityConfig, StaticDns};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use url::Url;
use wiremock::matchers::{body_string_contains, header as header_is, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DID: &str = "did:plc:re3ebnp5v7ffagz6rb6xfei4";
const HANDLE: &str = "alice.test";

async fn mount(server: &MockServer) {
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
    Mock::given(method("POST"))
        .and(path("/oauth/par"))
        .respond_with(ResponseTemplate::new(201).set_body_json(
            json!({"request_uri": "urn:ietf:params:oauth:request_uri:abc", "expires_in": 60}),
        ))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(body_string_contains("grant_type=authorization_code"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "tok1", "token_type": "DPoP", "refresh_token": "rt1",
            "expires_in": 3600, "scope": "atproto", "sub": DID
        })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/revoke"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.server.getSession"))
        .and(header_is("authorization", "DPoP tok1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"did": DID, "handle": HANDLE})),
        )
        .mount(server)
        .await;
}

fn state_for(server: &MockServer) -> AppState {
    let http = GuardedClient::new(
        Policy::for_tests(),
        Arc::new(StaticHosts::new().with(HANDLE, *server.address())),
        "eaten-at-tests",
    )
    .unwrap();
    let dns = StaticDns::new().with_txt(&format!("_atproto.{HANDLE}"), &[&format!("did={DID}")]);
    AppState::new(
        http,
        Arc::new(dns),
        AppConfig {
            identity: IdentityConfig {
                plc_directory: Url::parse(&server.uri()).unwrap(),
            },
            public_url: Url::parse("https://eaten.at").unwrap(),
            oauth_signing_key: None,
            ..AppConfig::default()
        },
        Cache::in_memory(Arc::new(SystemClock)).unwrap(),
    )
    .unwrap()
}

struct Reply {
    status: StatusCode,
    location: Option<String>,
    set_cookie: Option<String>,
    body: String,
}

async fn send(state: &AppState, request: Request<Body>) -> Reply {
    let response = router(state.clone()).oneshot(request).await.unwrap();
    let status = response.status();
    let get = |name: header::HeaderName| {
        response
            .headers()
            .get(name)
            .map(|v| v.to_str().unwrap().to_owned())
    };
    let location = get(header::LOCATION);
    let set_cookie = get(header::SET_COOKIE);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    Reply {
        status,
        location,
        set_cookie,
        body: String::from_utf8_lossy(&body).into_owned(),
    }
}

async fn get(state: &AppState, uri: &str, cookie: Option<&str>) -> Reply {
    let mut request = Request::get(uri);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    send(state, request.body(Body::empty()).unwrap()).await
}

async fn post_form(state: &AppState, uri: &str, form: &str, cookie: Option<&str>) -> Reply {
    let mut request =
        Request::post(uri).header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
    }
    send(state, request.body(Body::from(form.to_owned())).unwrap()).await
}

/// The `state` the app sent in its pushed authorization request.
async fn par_state(server: &MockServer) -> String {
    let requests = server.received_requests().await.unwrap();
    let par = requests
        .iter()
        .rfind(|r| r.url.path() == "/oauth/par")
        .expect("a PAR request was made");
    url::form_urlencoded::parse(&par.body)
        .find(|(k, _)| k == "state")
        .map(|(_, v)| v.into_owned())
        .unwrap()
}

/// The cookie pair (`name=value`) from a `Set-Cookie` header.
fn cookie_pair(set_cookie: &str) -> String {
    set_cookie.split(';').next().unwrap().to_owned()
}

#[tokio::test]
async fn sign_in_sets_a_session_and_sign_out_clears_it() {
    let server = MockServer::start().await;
    mount(&server).await;
    let state = state_for(&server);

    let form = get(&state, "/login?return_to=/write", None).await;
    assert_eq!(form.status, StatusCode::OK);
    assert!(
        form.body.contains("action=\"/login\" method=\"post\""),
        "{}",
        form.body
    );
    assert!(
        form.body.contains("name=\"return_to\" value=\"/write\""),
        "{}",
        form.body
    );

    let started = post_form(
        &state,
        "/login",
        "handle=%40Alice.Test&return_to=%2Fwrite",
        None,
    )
    .await;
    assert_eq!(started.status, StatusCode::OK, "{}", started.body);
    let authorize = format!("{}/oauth/authorize?", server.uri());
    assert!(
        started.body.contains(&format!(
            "http-equiv=\"refresh\" content=\"0;url={authorize}"
        )),
        "{}",
        started.body
    );
    assert!(
        started.body.contains("Continuing to 127.0.0.1"),
        "{}",
        started.body
    );
    assert!(
        started.body.contains("class=\"button\""),
        "a visible link as well: {}",
        started.body
    );

    let state_param = par_state(&server).await;
    let done = get(
        &state,
        &format!(
            "/oauth/callback?code=c1&state={state_param}&iss={}",
            server.uri()
        ),
        None,
    )
    .await;
    assert_eq!(done.status, StatusCode::SEE_OTHER, "{}", done.body);
    assert_eq!(done.location.as_deref(), Some("/write"));
    let set_cookie = done.set_cookie.expect("a session cookie is set");
    assert!(
        set_cookie.ends_with(
            "; Path=/; Max-Age=2592000; HttpOnly; SameSite=Lax; Domain=eaten.at; Secure"
        ),
        "{set_cookie}"
    );
    let cookie = cookie_pair(&set_cookie);

    let home = get(&state, "/", Some(&cookie)).await;
    assert!(
        home.body
            .contains(&format!("<p class=\"meta handle\">@{HANDLE}</p>")),
        "{}",
        home.body
    );
    assert!(home.body.contains("Write a new digest"), "{}", home.body);
    assert!(home.body.contains("action=\"/logout\""), "{}", home.body);
    let anonymous = get(&state, "/", None).await;
    assert!(
        anonymous.body.contains("href=\"/login\""),
        "{}",
        anonymous.body
    );
    assert!(!anonymous.body.contains("Signed in as"));

    // Signed in, the form is skipped.
    let again = get(&state, "/login", Some(&cookie)).await;
    assert_eq!(again.status, StatusCode::SEE_OTHER);
    assert_eq!(again.location.as_deref(), Some("/"));

    let out = post_form(&state, "/logout", "", Some(&cookie)).await;
    assert_eq!(out.status, StatusCode::SEE_OTHER);
    assert_eq!(out.location.as_deref(), Some("/"));
    assert!(
        out.set_cookie
            .as_deref()
            .unwrap()
            .starts_with("ea_session=; Path=/; Max-Age=0;"),
        "{:?}",
        out.set_cookie
    );
    let after = get(&state, "/", Some(&cookie)).await;
    assert!(!after.body.contains("Signed in as"), "{}", after.body);
    assert!(server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .any(|r| r.url.path() == "/oauth/revoke"));
}

#[tokio::test]
async fn bad_handles_and_refused_callbacks_are_explained() {
    let server = MockServer::start().await;
    mount(&server).await;
    let state = state_for(&server);

    let garbage = post_form(&state, "/login", "handle=not+a+handle", None).await;
    assert_eq!(garbage.status, StatusCode::BAD_REQUEST);
    assert!(garbage.body.contains("role=\"alert\""), "{}", garbage.body);
    assert!(
        garbage.body.contains("value=\"not a handle\""),
        "the input is kept: {}",
        garbage.body
    );

    let unknown = post_form(&state, "/login", "handle=nobody.test", None).await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND, "{}", unknown.body);
    assert!(
        unknown.body.contains("No AT Protocol account"),
        "{}",
        unknown.body
    );

    let cancelled = get(&state, "/oauth/callback?error=access_denied&state=x", None).await;
    assert_eq!(cancelled.status, StatusCode::OK);
    assert!(
        cancelled.body.contains("Sign-in was cancelled"),
        "{}",
        cancelled.body
    );
    assert!(cancelled.set_cookie.is_none());

    let forged = get(&state, "/oauth/callback?code=c&state=never-issued", None).await;
    assert_eq!(forged.status, StatusCode::BAD_GATEWAY);
    assert!(forged.body.contains("Try again"), "{}", forged.body);
    assert!(forged.set_cookie.is_none());

    // A callback that would land off-site lands home instead.
    post_form(
        &state,
        "/login",
        "handle=alice.test&return_to=https://evil.example",
        None,
    )
    .await;
    let state_param = par_state(&server).await;
    let done = get(
        &state,
        &format!(
            "/oauth/callback?code=c1&state={state_param}&iss={}",
            server.uri()
        ),
        None,
    )
    .await;
    assert_eq!(done.location.as_deref(), Some("/"));
}

#[tokio::test]
async fn sign_in_lives_on_the_bare_origin_only() {
    let server = MockServer::start().await;
    mount(&server).await;
    let state = state_for(&server);

    let elsewhere = send(
        &state,
        Request::get("/login")
            .header(header::HOST, "ross.eaten.at")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(elsewhere.status, StatusCode::SEE_OTHER);
    assert_eq!(
        elsewhere.location.as_deref(),
        Some("https://eaten.at/login")
    );
    let here = send(
        &state,
        Request::get("/login")
            .header(header::HOST, "eaten.at")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(here.status, StatusCode::OK);

    let metadata = get(&state, "/client-metadata.json", None).await;
    assert_eq!(metadata.status, StatusCode::OK);
    let doc: Value = serde_json::from_str(&metadata.body).unwrap();
    assert_eq!(doc["client_id"], "https://eaten.at/client-metadata.json");
    assert_eq!(
        doc["redirect_uris"],
        json!(["https://eaten.at/oauth/callback"])
    );
    assert_eq!(doc["client_name"], "eaten.at");
    assert_eq!(
        doc["scope"],
        "atproto repo:site.standard.publication?action=create&action=update \
         repo:site.standard.document \
         repo:at.eaten.preferences?action=create&action=update \
         blob:image/* \
         repo:app.bsky.feed.post?action=create&action=delete",
        "every scope the client may ever ask for, the crosspost one last"
    );
    assert_eq!(doc["token_endpoint_auth_method"], "none");
    let hidden = send(
        &state,
        Request::get("/client-metadata.json")
            .header(header::HOST, "ross.eaten.at")
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);
}
