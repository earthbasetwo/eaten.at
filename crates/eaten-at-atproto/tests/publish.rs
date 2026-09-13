//! Lexicon publishing against a mock PDS, and NSID resolution.

use std::sync::Arc;

use eaten_at_atproto::http::{GuardedClient, HttpError, Policy, StaticHosts, SystemHosts};
use eaten_at_atproto::identity::{Did, IdentityConfig, IdentityResolver, StaticDns, SystemDns};
use eaten_at_atproto::lexicon::publish::{apply, plan, Action};
use eaten_at_atproto::lexicon::resolve::{
    authority_did, lexicon_dns_name, resolve_lexicon, ResolveError,
};
use eaten_at_atproto::lexicon::schema::SchemaFile;
use eaten_at_atproto::repo::write::AppPasswordSession;
use eaten_at_atproto::repo::{RepoClient, RepoError};
use serde_json::{json, Value};
use url::Url;
use wiremock::matchers::{body_json, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DID: &str = "did:plc:re3ebnp5v7ffagz6rb6xfei4";
const SUBJECT: &str = include_str!("../../../lexicons/at.eaten.subject.json");
const PREFS: &str = include_str!("../../../lexicons/at.eaten.preferences.json");

fn http() -> GuardedClient {
    GuardedClient::new(
        Policy::for_tests(),
        Arc::new(StaticHosts::new()),
        "eaten-at-tests",
    )
    .unwrap()
}

fn schemas() -> Vec<SchemaFile> {
    vec![
        SchemaFile::parse(PREFS, "at.eaten.preferences").unwrap(),
        SchemaFile::parse(SUBJECT, "at.eaten.subject").unwrap(),
    ]
}

async fn mount_existing(server: &MockServer, nsid: &str, value: Value) {
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.getRecord"))
        .and(query_param("collection", "com.atproto.lexicon.schema"))
        .and(query_param("rkey", nsid))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "uri": format!("at://{DID}/com.atproto.lexicon.schema/{nsid}"), "cid": "bafyold", "value": value
        })))
        .mount(server)
        .await;
}

async fn mount_not_found(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.getRecord"))
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"error": "RecordNotFound", "message": "no"})),
        )
        .mount(server)
        .await;
}

async fn mount_login(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.server.createSession"))
        .and(body_json(json!({"identifier": "eaten.test", "password": "app-pw"})))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "did": DID, "handle": "eaten.test", "accessJwt": "access-token", "refreshJwt": "refresh-token"
        })))
        .mount(server)
        .await;
}

#[tokio::test]
async fn plan_distinguishes_unchanged_changed_and_missing() {
    let server = MockServer::start().await;
    let [prefs, subject] = <[SchemaFile; 2]>::try_from(schemas()).unwrap();
    // Published copy of the subject with the $type present and one field changed.
    let mut changed = subject.record_value();
    changed["defs"]["main"]["properties"]["title"]["maxGraphemes"] = json!(100);
    mount_existing(&server, "at.eaten.subject", changed).await;
    mount_existing(&server, "at.eaten.preferences", prefs.record_value()).await;

    let reader = RepoClient::new(http(), Url::parse(&server.uri()).unwrap());
    let entries = plan(&reader, &Did::parse(DID).unwrap(), &[prefs, subject])
        .await
        .unwrap();
    assert_eq!(entries[0].action, Action::Unchanged);
    assert!(matches!(entries[1].action, Action::Update { .. }));
    let Action::Update { current } = &entries[1].action else {
        unreachable!()
    };
    assert!(current.get("$type").is_none(), "compared without $type");

    let server = MockServer::start().await;
    mount_not_found(&server).await;
    let reader = RepoClient::new(http(), Url::parse(&server.uri()).unwrap());
    let entries = plan(&reader, &Did::parse(DID).unwrap(), &schemas())
        .await
        .unwrap();
    assert!(entries.iter().all(|e| e.action == Action::Create));
}

#[tokio::test]
async fn apply_writes_only_what_differs_with_exact_bodies() {
    let server = MockServer::start().await;
    let [prefs, subject] = <[SchemaFile; 2]>::try_from(schemas()).unwrap();
    mount_existing(&server, "at.eaten.preferences", prefs.record_value()).await;
    mount_not_found(&server).await;
    mount_login(&server).await;
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.putRecord"))
        .and(header("authorization", "Bearer access-token"))
        .and(body_json(json!({
            "repo": DID, "collection": "com.atproto.lexicon.schema",
            "rkey": "at.eaten.subject", "record": subject.record_value()
        })))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "uri": format!("at://{DID}/com.atproto.lexicon.schema/at.eaten.subject"), "cid": "bafynew"
        })))
        .expect(1)
        .mount(&server)
        .await;

    let session = AppPasswordSession::login(
        http(),
        Url::parse(&server.uri()).unwrap(),
        "eaten.test",
        "app-pw",
    )
    .await
    .unwrap();
    assert_eq!(session.did.as_str(), DID);
    let entries = plan(&session.reader(), &session.did, &[prefs, subject])
        .await
        .unwrap();
    let written = apply(&session, &entries).await.unwrap();
    assert_eq!(written.len(), 1);
    assert_eq!(written[0].0, "at.eaten.subject");
    assert_eq!(written[0].1.cid, "bafynew");
}

#[tokio::test]
async fn bad_credentials_are_an_xrpc_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.server.createSession"))
        .respond_with(ResponseTemplate::new(401).set_body_json(
            json!({"error": "AuthenticationRequired", "message": "Invalid identifier or password"}),
        ))
        .mount(&server)
        .await;
    let err = AppPasswordSession::login(http(), Url::parse(&server.uri()).unwrap(), "x", "y")
        .await
        .unwrap_err();
    assert!(
        matches!(err, RepoError::Xrpc { status, .. } if status.as_u16() == 401),
        "{err}"
    );
}

#[tokio::test]
async fn post_is_never_redirected() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(307).insert_header("location", "http://10.0.0.1/steal"))
        .mount(&server)
        .await;
    let err = http()
        .post_json(
            Url::parse(&format!("{}/x", server.uri())).unwrap(),
            &json!({}),
            Some("secret"),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Blocked { .. }), "{err}");
}

#[tokio::test]
async fn resolves_nsid_through_dns_txt_and_the_authority_repo() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/{DID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": DID, "alsoKnownAs": ["at://eaten.test"],
            "service": [{"id": "#atproto_pds", "type": "AtprotoPersonalDataServer", "serviceEndpoint": server.uri()}]
        })))
        .mount(&server)
        .await;
    let subject = SchemaFile::parse(SUBJECT, "at.eaten.subject").unwrap();
    mount_existing(&server, "at.eaten.subject", subject.record_value()).await;
    mount_not_found(&server).await;

    assert_eq!(
        lexicon_dns_name("at.eaten.subject").as_deref(),
        Some("_lexicon.eaten.at")
    );
    let dns = StaticDns::new().with_txt("_lexicon.eaten.at", &["v=spf1", &format!("did={DID}")]);
    assert_eq!(
        authority_did(&dns, "at.eaten.subject")
            .await
            .unwrap()
            .as_str(),
        DID
    );

    let identity = IdentityResolver::new(
        http(),
        Arc::new(dns.clone()),
        IdentityConfig {
            plc_directory: Url::parse(&server.uri()).unwrap(),
        },
    );
    let value = resolve_lexicon(&identity, &dns, "at.eaten.subject")
        .await
        .unwrap();
    assert_eq!(value, subject.value());

    let err = resolve_lexicon(&identity, &dns, "at.eaten.missing")
        .await
        .unwrap_err();
    assert!(matches!(err, ResolveError::NoRecord(_)), "{err}");
    let err = resolve_lexicon(&identity, &StaticDns::new(), "at.eaten.subject")
        .await
        .unwrap_err();
    assert!(matches!(err, ResolveError::NoDnsRecord(_)), "{err}");
    let ambiguous = StaticDns::new().with_txt(
        "_lexicon.eaten.at",
        &[
            &format!("did={DID}"),
            "did=did:plc:aaaaaaaaaaaaaaaaaaaaaaaa",
        ],
    );
    assert!(matches!(
        authority_did(&ambiguous, "at.eaten.subject").await,
        Err(ResolveError::Ambiguous(_))
    ));
}

/// Third-party proof: standard.site's schemas resolve through the same path.
#[tokio::test]
#[ignore = "hits public DNS, plc.directory, and a PDS"]
async fn live_resolves_a_standard_site_schema() {
    let http = GuardedClient::new(
        Policy::production(),
        Arc::new(SystemHosts),
        "eaten-at-tests",
    )
    .unwrap();
    let dns = SystemDns::from_system_conf().unwrap();
    let identity = IdentityResolver::new(http, Arc::new(dns.clone()), IdentityConfig::default());
    let value = resolve_lexicon(&identity, &dns, "site.standard.document")
        .await
        .unwrap();
    assert_eq!(value["id"], "site.standard.document");
    assert!(value["defs"]["main"]["record"]["properties"]["title"].is_object());
}
