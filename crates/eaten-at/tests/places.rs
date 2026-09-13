//! The Open Places client against a mock server: what a search returns,
//! how it is cached, and how each failure reads.

use std::sync::Arc;

use eaten_at::bsky::BskyConfig;
use eaten_at::cache::{Cache, SystemClock};
use eaten_at::places::{PlacesConfig, Point, SearchError};
use eaten_at::state::{AppConfig, AppState};
use eaten_at_atproto::http::{GuardedClient, Policy, StaticHosts};
use eaten_at_atproto::identity::{IdentityConfig, StaticDns};
use serde_json::json;
use url::Url;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn state_for(server: &MockServer, key: Option<&str>) -> AppState {
    let http = GuardedClient::new(
        Policy::for_tests(),
        Arc::new(StaticHosts::new()),
        "eaten-at-tests",
    )
    .unwrap();
    AppState::new(
        http,
        Arc::new(StaticDns::new()),
        AppConfig {
            identity: IdentityConfig {
                plc_directory: Url::parse(&server.uri()).unwrap(),
            },
            bsky: BskyConfig {
                appview: Url::parse(&server.uri()).unwrap(),
            },
            public_url: Url::parse("https://eaten.at").unwrap(),
            oauth_signing_key: None,
            places: PlacesConfig {
                base_url: Url::parse(&server.uri()).unwrap(),
                api_key: key.map(str::to_owned),
            },
        },
        Cache::in_memory(Arc::new(SystemClock)).unwrap(),
    )
    .unwrap()
}

fn brooklyn() -> Point {
    Point {
        lat: 40.6888,
        lon: -73.9799,
    }
}

fn results() -> serde_json::Value {
    json!({
        "results": [
            {
                "place_id": "overture:76f1250d-8e38-40b3-a021-bfe1c16b4e1c",
                "name": "Devocion", "lat": 40.701_607, "lon": -73.986_565, "distance_mi": 0.95,
                "category": "coffee_shop",
                "address": {"formatted": "105 York St", "street": "105 York St", "locality": "Brooklyn", "region": "ny", "postal_code": "11201"},
                "website": "https://www.devocion.com/", "confidence": 0.95
            },
            {"place_id": "overture:44017831-5500-4780-a4e7-a8fac208e6fe", "name": "Devocion Flatiron", "lat": 40.739_05, "lon": -73.989_155, "distance_mi": 3.51}
        ],
        "meta": {"data_release": "2026-08-19.0", "warnings": []}
    })
}

#[tokio::test]
async fn a_search_sends_the_key_and_the_point_and_is_cached() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/places"))
        .and(header("authorization", "Bearer test-key"))
        .and(query_param("q", "devocion"))
        .and(query_param("lat", "40.6888"))
        .and(query_param("lon", "-73.9799"))
        .and(query_param("limit", "10"))
        .respond_with(ResponseTemplate::new(200).set_body_json(results()))
        .expect(1)
        .mount(&server)
        .await;
    let state = state_for(&server, Some("test-key"));
    assert!(state.places_enabled());
    let hits = state
        .search_places("  Devocion ", brooklyn())
        .await
        .unwrap();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0].gers_id, "76f1250d-8e38-40b3-a021-bfe1c16b4e1c");
    assert_eq!(
        hits[0].address.as_deref(),
        Some("105 York St, Brooklyn, NY 11201")
    );
    assert_eq!(hits[1].address, None);
    // The same search, differently spaced and cased, is a cache hit; so
    // is one from a point a few metres away.
    let again = state.search_places("DEVOCION", brooklyn()).await.unwrap();
    assert_eq!(again, hits);
    let nearby = Point {
        lat: 40.68884,
        lon: -73.97991,
    };
    assert_eq!(state.search_places("devocion", nearby).await.unwrap(), hits);
}

#[tokio::test]
async fn failures_read_calmly_and_are_not_cached() {
    let server = MockServer::start().await;
    let state = state_for(&server, Some("test-key"));
    let point = brooklyn();

    let quota = ResponseTemplate::new(402)
        .insert_header("x-request-id", "req-1")
        .insert_header("x-quota-remaining", "0")
        .set_body_json(json!({"error": {"code": "quota_exhausted", "message": "Monthly quota exhausted.", "request_id": "req-1"}}));
    Mock::given(path("/v1/places"))
        .respond_with(quota)
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        state.search_places("katz", point).await,
        Err(SearchError::Unavailable)
    );
    server.reset().await;

    Mock::given(path("/v1/places"))
        .respond_with(ResponseTemplate::new(400).set_body_json(json!({
            "error": {"code": "validation_error", "message": "q needs at least 3 searchable characters."}
        })))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        state.search_places("k9", point).await,
        Err(SearchError::Query(
            "q needs at least 3 searchable characters.".into()
        ))
    );
    server.reset().await;

    Mock::given(path("/v1/places"))
        .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(
        state.search_places("katz", point).await,
        Err(SearchError::Unavailable)
    );
    server.reset().await;

    // A failure is not remembered: the next call asks again.
    Mock::given(path("/v1/places"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"results": []})))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(state.search_places("katz", point).await.unwrap(), vec![]);
}

#[tokio::test]
async fn short_queries_and_a_missing_key_never_reach_the_api() {
    let server = MockServer::start().await;
    Mock::given(path("/v1/places"))
        .respond_with(ResponseTemplate::new(200).set_body_json(results()))
        .expect(0)
        .mount(&server)
        .await;
    let state = state_for(&server, Some("test-key"));
    assert!(matches!(
        state.search_places("k", brooklyn()).await,
        Err(SearchError::Query(_))
    ));
    assert!(matches!(
        state.search_places(&"x".repeat(129), brooklyn()).await,
        Err(SearchError::Query(_))
    ));
    let disabled = state_for(&server, None);
    assert!(!disabled.places_enabled());
    assert_eq!(
        disabled.search_places("katz", brooklyn()).await,
        Err(SearchError::Disabled)
    );
}
