//! Identity resolution against a mock PLC directory and mock hosts.

use std::sync::Arc;

use eaten_at_atproto::http::{GuardedClient, HttpError, Policy, StaticHosts, SystemHosts};
use eaten_at_atproto::identity::{
    Did, Handle, IdentityConfig, IdentityError, IdentityResolver, StaticDns, SystemDns,
};
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PLC_DID: &str = "did:plc:re3ebnp5v7ffagz6rb6xfei4";
const OTHER_DID: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const HANDLE: &str = "alice.test";

/// Build a DID document JSON body for `did` with the given PDS and aliases.
fn doc(did: &str, pds: &str, aliases: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "@context": ["https://www.w3.org/ns/did/v1"],
        "id": did,
        "alsoKnownAs": aliases.iter().map(|a| format!("at://{a}")).collect::<Vec<_>>(),
        "service": [{
            "id": "#atproto_pds",
            "type": "AtprotoPersonalDataServer",
            "serviceEndpoint": pds
        }]
    })
}

/// A guarded client that sends requests for `host` to the mock server.
fn client_for(server: &MockServer, host: &str, policy: Policy) -> GuardedClient {
    let hosts = StaticHosts::new().with(host, *server.address());
    GuardedClient::new(policy, Arc::new(hosts), "eaten-at-tests").unwrap()
}

fn config(server: &MockServer) -> IdentityConfig {
    IdentityConfig {
        plc_directory: Url::parse(&server.uri()).unwrap(),
    }
}

fn resolver(server: &MockServer, dns: StaticDns) -> IdentityResolver {
    IdentityResolver::new(
        client_for(server, HANDLE, Policy::for_tests()),
        Arc::new(dns),
        config(server),
    )
}

async fn mount_plc(server: &MockServer, did: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(format!("/{did}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn resolves_plc_did_to_pds() {
    let server = MockServer::start().await;
    mount_plc(&server, PLC_DID, doc(PLC_DID, &server.uri(), &[HANDLE])).await;

    let identity = resolver(&server, StaticDns::new())
        .resolve_did(&Did::parse(PLC_DID).unwrap())
        .await
        .unwrap();

    assert_eq!(identity.did.as_str(), PLC_DID);
    assert_eq!(identity.pds.as_str().trim_end_matches('/'), server.uri());
    assert_eq!(identity.claimed_handle().unwrap().as_str(), HANDLE);
    // The claim is not verified: no DNS record and no well-known file.
    assert!(identity.handle.is_none());
    assert_eq!(identity.display_name(), PLC_DID);
}

#[tokio::test]
async fn resolve_did_verifies_the_claimed_handle_when_it_resolves_back() {
    let server = MockServer::start().await;
    mount_plc(&server, PLC_DID, doc(PLC_DID, &server.uri(), &[HANDLE])).await;
    let dns =
        StaticDns::new().with_txt(&format!("_atproto.{HANDLE}"), &[&format!("did={PLC_DID}")]);
    let identity = resolver(&server, dns)
        .resolve_did(&Did::parse(PLC_DID).unwrap())
        .await
        .unwrap();
    assert_eq!(identity.handle.unwrap().as_str(), HANDLE);
}

#[tokio::test]
async fn resolve_did_ignores_a_claimed_handle_owned_by_someone_else() {
    let server = MockServer::start().await;
    mount_plc(&server, PLC_DID, doc(PLC_DID, &server.uri(), &[HANDLE])).await;
    let dns = StaticDns::new().with_txt(
        &format!("_atproto.{HANDLE}"),
        &[&format!("did={OTHER_DID}")],
    );
    let identity = resolver(&server, dns)
        .resolve_did(&Did::parse(PLC_DID).unwrap())
        .await
        .unwrap();
    assert!(identity.handle.is_none());
}

#[tokio::test]
async fn unknown_plc_did_is_not_found() {
    let server = MockServer::start().await;
    let err = resolver(&server, StaticDns::new())
        .resolve_did(&Did::parse(PLC_DID).unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, IdentityError::DidNotFound(_)), "{err}");
}

#[tokio::test]
async fn document_for_a_different_did_is_malformed() {
    let server = MockServer::start().await;
    mount_plc(&server, PLC_DID, doc(OTHER_DID, &server.uri(), &[])).await;
    let err = resolver(&server, StaticDns::new())
        .resolve_did(&Did::parse(PLC_DID).unwrap())
        .await
        .unwrap_err();
    assert!(
        matches!(err, IdentityError::MalformedDocument { .. }),
        "{err}"
    );
}

#[tokio::test]
async fn invalid_json_is_malformed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/{PLC_DID}")))
        .respond_with(ResponseTemplate::new(200).set_body_string("<html>nope</html>"))
        .mount(&server)
        .await;
    let err = resolver(&server, StaticDns::new())
        .resolve_did(&Did::parse(PLC_DID).unwrap())
        .await
        .unwrap_err();
    assert!(
        matches!(err, IdentityError::MalformedDocument { .. }),
        "{err}"
    );
}

#[tokio::test]
async fn document_without_pds_is_rejected() {
    let server = MockServer::start().await;
    mount_plc(
        &server,
        PLC_DID,
        serde_json::json!({"id": PLC_DID, "service": []}),
    )
    .await;
    let err = resolver(&server, StaticDns::new())
        .resolve_did(&Did::parse(PLC_DID).unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, IdentityError::NoPds(_)), "{err}");
}

#[tokio::test]
async fn non_http_pds_endpoint_is_rejected() {
    let server = MockServer::start().await;
    mount_plc(&server, PLC_DID, doc(PLC_DID, "ftp://pds.example", &[])).await;
    let err = resolver(&server, StaticDns::new())
        .resolve_did(&Did::parse(PLC_DID).unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, IdentityError::InsecureEndpoint(_)), "{err}");
}

#[tokio::test]
async fn plain_http_is_refused_unless_allowed() {
    let server = MockServer::start().await;
    mount_plc(&server, PLC_DID, doc(PLC_DID, &server.uri(), &[])).await;
    let resolver = IdentityResolver::new(
        client_for(&server, HANDLE, Policy::production()),
        Arc::new(StaticDns::new()),
        config(&server),
    );
    let err = resolver
        .resolve_did(&Did::parse(PLC_DID).unwrap())
        .await
        .unwrap_err();
    assert!(
        matches!(err, IdentityError::Http(HttpError::Blocked { .. })),
        "{err}"
    );
}

#[tokio::test]
async fn upstream_server_error_is_reported() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/{PLC_DID}")))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;
    let err = resolver(&server, StaticDns::new())
        .resolve_did(&Did::parse(PLC_DID).unwrap())
        .await
        .unwrap_err();
    assert!(
        matches!(err, IdentityError::UpstreamStatus { status, .. } if status.as_u16() == 503),
        "{err}"
    );
}

#[tokio::test]
async fn resolves_did_web_from_well_known() {
    let server = MockServer::start().await;
    let did = format!("did:web:{HANDLE}");
    Mock::given(method("GET"))
        .and(path("/.well-known/did.json"))
        .respond_with(ResponseTemplate::new(200).set_body_json(doc(&did, &server.uri(), &[HANDLE])))
        .mount(&server)
        .await;

    let identity = resolver(&server, StaticDns::new())
        .resolve_did(&Did::parse(&did).unwrap())
        .await
        .unwrap();
    assert_eq!(identity.did.as_str(), did);
}

#[tokio::test]
async fn resolves_handle_via_dns_txt() {
    let server = MockServer::start().await;
    mount_plc(&server, PLC_DID, doc(PLC_DID, &server.uri(), &[HANDLE])).await;
    let dns = StaticDns::new().with_txt(
        &format!("_atproto.{HANDLE}"),
        &["v=spf1 -all", &format!("did={PLC_DID}")],
    );

    let identity = resolver(&server, dns)
        .resolve_handle(&Handle::parse(HANDLE).unwrap())
        .await
        .unwrap();
    assert_eq!(identity.did.as_str(), PLC_DID);
}

#[tokio::test]
async fn resolves_handle_via_well_known_when_dns_is_empty() {
    let server = MockServer::start().await;
    mount_plc(&server, PLC_DID, doc(PLC_DID, &server.uri(), &[HANDLE])).await;
    Mock::given(method("GET"))
        .and(path("/.well-known/atproto-did"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!("{PLC_DID}\n")))
        .mount(&server)
        .await;

    let identity = resolver(&server, StaticDns::new())
        .resolve_handle(&Handle::parse(HANDLE).unwrap())
        .await
        .unwrap();
    assert_eq!(identity.did.as_str(), PLC_DID);
}

#[tokio::test]
async fn dns_failure_falls_back_to_well_known() {
    let server = MockServer::start().await;
    mount_plc(&server, PLC_DID, doc(PLC_DID, &server.uri(), &[HANDLE])).await;
    Mock::given(method("GET"))
        .and(path("/.well-known/atproto-did"))
        .respond_with(ResponseTemplate::new(200).set_body_string(PLC_DID))
        .mount(&server)
        .await;

    let identity = resolver(&server, StaticDns::new().failing())
        .resolve_handle(&Handle::parse(HANDLE).unwrap())
        .await
        .unwrap();
    assert_eq!(identity.did.as_str(), PLC_DID);
}

#[tokio::test]
async fn garbage_dns_values_are_ignored() {
    let server = MockServer::start().await;
    mount_plc(&server, PLC_DID, doc(PLC_DID, &server.uri(), &[HANDLE])).await;
    let dns = StaticDns::new().with_txt(
        &format!("_atproto.{HANDLE}"),
        &[
            "did=not-a-did",
            "did=did:key:zzz",
            &format!("did={PLC_DID}"),
        ],
    );

    let identity = resolver(&server, dns)
        .resolve_handle(&Handle::parse(HANDLE).unwrap())
        .await
        .unwrap();
    assert_eq!(identity.did.as_str(), PLC_DID);
}

#[tokio::test]
async fn conflicting_dns_records_are_ambiguous() {
    let server = MockServer::start().await;
    let dns = StaticDns::new().with_txt(
        &format!("_atproto.{HANDLE}"),
        &[&format!("did={PLC_DID}"), &format!("did={OTHER_DID}")],
    );
    let err = resolver(&server, dns)
        .resolve_handle(&Handle::parse(HANDLE).unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, IdentityError::AmbiguousDns(_)), "{err}");
}

#[tokio::test]
async fn handle_not_listed_in_document_is_rejected() {
    let server = MockServer::start().await;
    mount_plc(
        &server,
        PLC_DID,
        doc(PLC_DID, &server.uri(), &["someone-else.test"]),
    )
    .await;
    let dns =
        StaticDns::new().with_txt(&format!("_atproto.{HANDLE}"), &[&format!("did={PLC_DID}")]);

    let err = resolver(&server, dns)
        .resolve_handle(&Handle::parse(HANDLE).unwrap())
        .await
        .unwrap_err();
    assert!(
        matches!(err, IdentityError::HandleNotClaimed { .. }),
        "{err}"
    );
}

#[tokio::test]
async fn unresolvable_handle_is_not_found() {
    let server = MockServer::start().await;
    let err = resolver(&server, StaticDns::new())
        .resolve_handle(&Handle::parse(HANDLE).unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, IdentityError::HandleNotFound(_)), "{err}");
}

/// Live check against the real network. Run with `cargo test -- --ignored`.
#[tokio::test]
#[ignore = "hits plc.directory and public DNS"]
async fn live_resolves_a_real_handle() {
    let http = GuardedClient::new(
        Policy::production(),
        Arc::new(SystemHosts),
        "eaten-at-tests",
    )
    .unwrap();
    let resolver = IdentityResolver::new(
        http,
        Arc::new(SystemDns::from_system_conf().unwrap()),
        IdentityConfig::default(),
    );
    let identity = resolver
        .resolve_handle(&Handle::parse("bsky.app").unwrap())
        .await
        .unwrap();
    assert_eq!(identity.pds.scheme(), "https");
    assert!(identity.claimed_handle().is_some());
}
