//! Repository reads against a mock PDS.

use std::sync::Arc;

use eaten_at_atproto::http::{GuardedClient, Policy, StaticHosts};
use eaten_at_atproto::identity::Did;
use eaten_at_atproto::lexicon::{Document, Publication};
use eaten_at_atproto::repo::{ListParams, RepoClient, RepoError};
use url::Url;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DID: &str = "did:plc:re3ebnp5v7ffagz6rb6xfei4";
const DOC: &str = include_str!("fixtures/standard-site-document.json");
const PUB: &str = include_str!("fixtures/standard-site-publication.json");

fn did() -> Did {
    Did::parse(DID).unwrap()
}

fn http() -> GuardedClient {
    GuardedClient::new(
        Policy::for_tests(),
        Arc::new(StaticHosts::new()),
        "eaten-at-tests",
    )
    .unwrap()
}

fn client(server: &MockServer) -> RepoClient {
    RepoClient::new(http(), Url::parse(&server.uri()).unwrap())
}

fn record(rkey: &str, value: &serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "uri": format!("at://{DID}/site.standard.document/{rkey}"),
        "cid": "bafyreicid",
        "value": value
    })
}

#[tokio::test]
async fn get_record_decodes_a_publication() {
    let server = MockServer::start().await;
    let value: serde_json::Value = serde_json::from_str(PUB).unwrap();
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.getRecord"))
        .and(query_param("repo", DID))
        .and(query_param("collection", "site.standard.publication"))
        .and(query_param("rkey", "3me5vykp6lf2y"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "uri": format!("at://{DID}/site.standard.publication/3me5vykp6lf2y"),
            "cid": "bafyreicid",
            "value": value
        })))
        .mount(&server)
        .await;

    let record = client(&server)
        .get_record::<Publication>(&did(), "site.standard.publication", "3me5vykp6lf2y")
        .await
        .unwrap();
    assert_eq!(record.rkey(), "3me5vykp6lf2y");
    assert_eq!(record.value.name, "Standard.site");
}

#[tokio::test]
async fn record_not_found_is_typed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.getRecord"))
        .respond_with(ResponseTemplate::new(400).set_body_json(serde_json::json!({
            "error": "RecordNotFound", "message": "Could not locate record"
        })))
        .mount(&server)
        .await;
    let err = client(&server)
        .get_record::<Publication>(&did(), "site.standard.publication", "nope")
        .await
        .unwrap_err();
    assert!(matches!(err, RepoError::RecordNotFound), "{err}");
}

#[tokio::test]
async fn plain_404_is_not_found_and_other_errors_are_xrpc() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.getRecord"))
        .and(query_param("rkey", "missing"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.getRecord"))
        .and(query_param("rkey", "broken"))
        .respond_with(ResponseTemplate::new(500).set_body_json(serde_json::json!({
            "error": "InternalServerError", "message": "boom"
        })))
        .mount(&server)
        .await;
    let c = client(&server);
    assert!(matches!(
        c.get_record::<Publication>(&did(), "site.standard.publication", "missing")
            .await,
        Err(RepoError::RecordNotFound)
    ));
    assert!(matches!(
        c.get_record::<Publication>(&did(), "site.standard.publication", "broken").await,
        Err(RepoError::Xrpc { status, .. }) if status.as_u16() == 500
    ));
}

#[tokio::test]
async fn list_records_follows_cursor_and_skips_undecodable() {
    let server = MockServer::start().await;
    let good: serde_json::Value = serde_json::from_str(DOC).unwrap();
    let bad = serde_json::json!({"$type": "site.standard.document", "title": "no publishedAt"});

    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.listRecords"))
        .and(query_param("collection", "site.standard.document"))
        .and(query_param("limit", "2"))
        .and(query_param("cursor", "page1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "records": [record("c", &good)]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.listRecords"))
        .and(query_param("collection", "site.standard.document"))
        .and(query_param("limit", "2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "records": [record("a", &good), record("b", &bad)],
            "cursor": "page1"
        })))
        .mount(&server)
        .await;

    let c = client(&server);
    let first = c
        .list_records::<Document>(
            &did(),
            "site.standard.document",
            &ListParams {
                limit: Some(2),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(first.records.len(), 1);
    assert_eq!(first.records[0].rkey(), "a");
    assert_eq!(first.skipped, 1);
    assert_eq!(first.cursor.as_deref(), Some("page1"));

    let second = c
        .list_records::<Document>(
            &did(),
            "site.standard.document",
            &ListParams {
                limit: Some(2),
                cursor: first.cursor,
                reverse: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(second.records.len(), 1);
    assert_eq!(second.records[0].rkey(), "c");
    assert!(second.cursor.is_none());
}

#[tokio::test]
async fn limit_is_clamped_and_reverse_is_sent() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.listRecords"))
        .and(query_param("limit", "100"))
        .and(query_param("reverse", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({"records": []})))
        .expect(1)
        .mount(&server)
        .await;
    let page = client(&server)
        .list_records::<Document>(
            &did(),
            "site.standard.document",
            &ListParams {
                limit: Some(5000),
                cursor: None,
                reverse: true,
            },
        )
        .await
        .unwrap();
    assert!(page.records.is_empty());
}

#[test]
fn blob_url_targets_sync_get_blob() {
    let c = RepoClient::new(http(), Url::parse("https://pds.example/").unwrap());
    let url = c.blob_url(&did(), "bafkreiabc");
    assert_eq!(
        url.as_str(),
        "https://pds.example/xrpc/com.atproto.sync.getBlob?did=did%3Aplc%3Are3ebnp5v7ffagz6rb6xfei4&cid=bafkreiabc"
    );
}
