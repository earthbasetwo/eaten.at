//! Site routes against a mock PLC directory and PDS.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use eaten_at::app::router;
use eaten_at::bsky::BskyConfig;
use eaten_at::cache::{Cache, SystemClock};
use eaten_at::state::{AppConfig, AppState};
use eaten_at_atproto::http::{GuardedClient, Policy, StaticHosts};
use eaten_at_atproto::identity::{IdentityConfig, StaticDns};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use url::Url;
use wiremock::matchers::{method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DID: &str = "did:plc:re3ebnp5v7ffagz6rb6xfei4";
const OTHER_DID: &str = "did:plc:aaaaaaaaaaaaaaaaaaaaaaaa";
const HANDLE: &str = "alice.test";
const PAGE: usize = 20;

/// Declarative description of a mock repo.
#[derive(Default)]
struct Repo {
    publications: Vec<(&'static str, Value)>,
    documents: Vec<(String, Value)>,
    preferences: Option<Value>,
    /// Omit the PDS service from the DID document.
    no_pds: bool,
}

fn publication(name: &str, url: &str) -> Value {
    json!({"$type": "site.standard.publication", "url": url, "name": name,
           "description": format!("{name} description")})
}

fn visit_doc(pub_rkey: &str, title: &str, place: &str, tags: &[&str]) -> Value {
    json!({
        "$type": "site.standard.document",
        "site": format!("at://{DID}/site.standard.publication/{pub_rkey}"),
        "title": title, "path": format!("/2026/09/{}", title.to_lowercase().replace(' ', "-")),
        "publishedAt": "2026-09-07T12:00:00.000Z",
        "tags": tags,
        "textContent": format!("{place} · 2026-09-06 · Recommended\n\nHeading\n\nA write-up of {place}."),
        "content": {
            "$type": "at.eaten.visit",
            "place": {
                "name": place, "address": "1 Example St", "price": 2,
                "ids": [{"service": "googlePlace", "id": "g1"}],
                "urls": [{"url": "https://example.com/official", "service": "officialSite"},
                         {"url": "https://example.com/buy", "service": "shop-thing"}]
            },
            "visitedOn": "2026-09-06",
            "meal": "dinner",
            "rating": 2,
            "body": {"$type": "at.markpub.markdown", "text": {"$type": "at.markpub.text",
                     "markdown": format!("# Heading\n\nA write-up of *{place}*.\n\nMore text here.")}}
        }
    })
}

fn plain_doc(pub_rkey: &str, title: &str) -> Value {
    json!({
        "$type": "site.standard.document",
        "site": format!("at://{DID}/site.standard.publication/{pub_rkey}"),
        "title": title, "publishedAt": "2026-09-07T12:00:00.000Z",
        "textContent": "Just a blog post."
    })
}

fn record(collection: &str, rkey: &str, value: &Value) -> Value {
    json!({"uri": format!("at://{DID}/{collection}/{rkey}"), "cid": "bafycid", "value": value})
}

fn not_found() -> ResponseTemplate {
    ResponseTemplate::new(400).set_body_json(json!({"error": "RecordNotFound", "message": "nope"}))
}

async fn mount(repo: &Repo) -> MockServer {
    let server = MockServer::start().await;
    let services = if repo.no_pds {
        json!([])
    } else {
        json!([{"id": "#atproto_pds", "type": "AtprotoPersonalDataServer", "serviceEndpoint": server.uri()}])
    };
    Mock::given(method("GET"))
        .and(path(format!("/{DID}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id": DID, "alsoKnownAs": [format!("at://{HANDLE}")], "service": services
        })))
        .mount(&server)
        .await;

    let pubs: Vec<Value> = repo
        .publications
        .iter()
        .map(|(rkey, v)| record("site.standard.publication", rkey, v))
        .collect();
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.listRecords"))
        .and(query_param("collection", "site.standard.publication"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({"records": pubs})))
        .mount(&server)
        .await;
    for (rkey, value) in &repo.publications {
        Mock::given(method("GET"))
            .and(path("/xrpc/com.atproto.repo.getRecord"))
            .and(query_param("collection", "site.standard.publication"))
            .and(query_param("rkey", *rkey))
            .respond_with(ResponseTemplate::new(200).set_body_json(record(
                "site.standard.publication",
                rkey,
                value,
            )))
            .mount(&server)
            .await;
    }

    mount_documents(&server, repo).await;

    if let Some(prefs) = &repo.preferences {
        Mock::given(method("GET"))
            .and(path("/xrpc/com.atproto.repo.getRecord"))
            .and(query_param("collection", "at.eaten.preferences"))
            .respond_with(ResponseTemplate::new(200).set_body_json(record(
                "at.eaten.preferences",
                "self",
                prefs,
            )))
            .mount(&server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.getRecord"))
        .respond_with(not_found())
        .mount(&server)
        .await;
    server
}

/// Document listing, paged by PAGE with cursor = last rkey of the page,
/// plus getRecord for each document.
async fn mount_documents(server: &MockServer, repo: &Repo) {
    // Document listing, paged by PAGE with cursor = last rkey of the page.
    let docs: Vec<Value> = repo
        .documents
        .iter()
        .map(|(rkey, v)| record("site.standard.document", rkey, v))
        .collect();
    let pages: Vec<&[Value]> = docs.chunks(PAGE).collect();
    for (i, page) in pages.iter().enumerate() {
        let next =
            (i + 1 < pages.len()).then(|| repo.documents[i * PAGE + page.len() - 1].0.clone());
        let mut body = json!({"records": page});
        if let Some(next) = &next {
            body["cursor"] = Value::String(next.clone());
        }
        let m = Mock::given(method("GET"))
            .and(path("/xrpc/com.atproto.repo.listRecords"))
            .and(query_param("collection", "site.standard.document"));
        let m = if i == 0 {
            m.and(query_param_is_missing("cursor"))
        } else {
            m.and(query_param("cursor", &repo.documents[i * PAGE - 1].0))
        };
        m.respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(server)
            .await;
    }
    if pages.is_empty() {
        Mock::given(method("GET"))
            .and(path("/xrpc/com.atproto.repo.listRecords"))
            .and(query_param("collection", "site.standard.document"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({"records": []})))
            .mount(server)
            .await;
    }
    for (rkey, value) in &repo.documents {
        Mock::given(method("GET"))
            .and(path("/xrpc/com.atproto.repo.getRecord"))
            .and(query_param("collection", "site.standard.document"))
            .and(query_param("rkey", rkey.as_str()))
            .respond_with(ResponseTemplate::new(200).set_body_json(record(
                "site.standard.document",
                rkey,
                value,
            )))
            .mount(server)
            .await;
    }
}

fn state_for(server: &MockServer, dns: StaticDns) -> AppState {
    let http = GuardedClient::new(
        Policy::for_tests(),
        Arc::new(StaticHosts::new().with(HANDLE, *server.address())),
        "eaten-at-tests",
    )
    .unwrap();
    AppState::new(
        http,
        Arc::new(dns),
        AppConfig {
            identity: IdentityConfig {
                plc_directory: Url::parse(&server.uri()).unwrap(),
            },
            bsky: BskyConfig {
                appview: Url::parse(&server.uri()).unwrap(),
            },
            public_url: Url::parse("https://eaten.at").unwrap(),
            oauth_signing_key: None,
        },
        Cache::in_memory(Arc::new(SystemClock)).unwrap(),
    )
    .unwrap()
}

fn dns_for_handle() -> StaticDns {
    StaticDns::new().with_txt(&format!("_atproto.{HANDLE}"), &[&format!("did={DID}")])
}

async fn get(state: &AppState, uri: &str) -> (StatusCode, Option<String>, String) {
    let response = router(state.clone())
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let location = response
        .headers()
        .get(header::LOCATION)
        .map(|v| v.to_str().unwrap().to_owned());
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        location,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

fn one_publication() -> Repo {
    Repo {
        publications: vec![("pub1", publication("Ross Writes", "https://ross.eaten.at"))],
        documents: vec![
            (
                "d3".into(),
                visit_doc("pub1", "Third Post", "Third Place", &["longform"]),
            ),
            ("d2".into(), plain_doc("pub1", "Plain Post")),
            (
                "d1".into(),
                visit_doc("pub1", "First Post", "First Place", &[]),
            ),
        ],
        ..Repo::default()
    }
}

#[tokio::test]
async fn landing_page_has_lookup_form() {
    let server = mount(&Repo::default()).await;
    let (status, _, body) = get(&state_for(&server, StaticDns::new()), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("action=\"/lookup\""), "{body}");
    assert!(body.contains("<main id=\"main\">"), "{body}");
}

#[tokio::test]
async fn lookup_form_redirects_to_handle_route_or_rejects_garbage() {
    let server = mount(&Repo::default()).await;
    let state = state_for(&server, StaticDns::new());
    let (status, location, _) = get(&state, "/lookup?handle=%40Alice.Test").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some("/@alice.test"));
    let (status, _, body) = get(&state, "/lookup?handle=not%20a%20handle").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("role=\"alert\""), "{body}");
}

#[tokio::test]
async fn handle_routes_redirect_to_did_routes() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, dns_for_handle());
    let (status, location, _) = get(&state, &format!("/@{HANDLE}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some(&*format!("/at/{DID}/")));
    let (status, location, _) = get(&state, &format!("/@{HANDLE}/pub1/d1")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some(&*format!("/at/{DID}/pub1/d1")));
}

#[tokio::test]
async fn unresolvable_handle_is_404() {
    let server = mount(&one_publication()).await;
    let (status, _, _) = get(&state_for(&server, StaticDns::new()), "/@nobody.test").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn single_publication_redirects_from_repo_root() {
    let server = mount(&one_publication()).await;
    let (status, location, _) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/"),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some(&*format!("/at/{DID}/pub1/")));
}

#[tokio::test]
async fn preference_picks_among_several_publications() {
    let mut repo = one_publication();
    repo.publications
        .push(("pub2", publication("Side Blog", "https://side.example")));
    repo.preferences =
        Some(json!({"defaultPublication": format!("at://{DID}/site.standard.publication/pub2")}));
    let server = mount(&repo).await;
    let (status, location, _) =
        get(&state_for(&server, StaticDns::new()), &format!("/at/{DID}")).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some(&*format!("/at/{DID}/pub2/")));
}

#[tokio::test]
async fn dangling_preference_or_no_preference_shows_chooser() {
    let mut repo = one_publication();
    repo.publications
        .push(("pub2", publication("Side Blog", "https://side.example")));
    repo.preferences = Some(
        json!({"defaultPublication": format!("at://{DID}/site.standard.publication/deleted")}),
    );
    let server = mount(&repo).await;
    let (status, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Ross Writes") && body.contains("Side Blog"),
        "{body}"
    );
    assert!(body.contains(&format!("/at/{DID}/pub2/")), "{body}");

    repo.preferences = None;
    let server = mount(&repo).await;
    let (status, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Pick one"), "{body}");
}

#[tokio::test]
async fn repo_without_publications_says_so() {
    let server = mount(&Repo::default()).await;
    let (status, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("no publications"), "{body}");
}

#[tokio::test]
async fn publication_page_lists_only_visit_documents_of_this_publication() {
    let mut repo = one_publication();
    repo.publications
        .push(("pub2", publication("Side Blog", "https://side.example")));
    repo.documents.insert(
        0,
        (
            "d4".into(),
            visit_doc("pub2", "Elsewhere", "Other Place", &[]),
        ),
    );
    let server = mount(&repo).await;
    let (status, _, body) = get(
        &state_for(&server, dns_for_handle()),
        &format!("/at/{DID}/pub1/"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("Third Post") && body.contains("First Post"),
        "{body}"
    );
    assert!(!body.contains("Plain Post"), "{body}");
    assert!(!body.contains("Elsewhere"), "{body}");
    assert!(
        body.contains("A write-up of Third Place."),
        "excerpt derived: {body}"
    );
    assert!(body.contains(&format!("/at/{DID}/pub1/d3")), "{body}");
    assert!(body.contains("alice.test"), "verified handle shown: {body}");
    assert!(
        !body.contains("rel=\"next\""),
        "no pagination on a single page: {body}"
    );
}

#[tokio::test]
async fn listing_paginates_with_cursor() {
    let mut repo = one_publication();
    repo.documents = (0..45)
        .map(|i| {
            (
                format!("d{:03}", 100 - i),
                visit_doc("pub1", &format!("Post {i}"), &format!("Place {i}"), &[]),
            )
        })
        .collect();
    let server = mount(&repo).await;
    let state = state_for(&server, StaticDns::new());
    let (status, _, body) = get(&state, &format!("/at/{DID}/pub1/")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Post 0") && body.contains("Post 19") && !body.contains("Post 20"),
        "{body}"
    );
    assert!(
        body.contains(&format!("/at/{DID}/pub1/?cursor=d081")),
        "{body}"
    );

    let (status, _, body) = get(&state, &format!("/at/{DID}/pub1/?cursor=d081")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Post 20") && body.contains("Post 39"),
        "{body}"
    );
    assert!(body.contains("rel=\"first\""), "{body}");

    let (status, _, body) = get(&state, &format!("/at/{DID}/pub1/?cursor=d061")).await;
    assert!(
        body.contains("Post 44") && !body.contains("rel=\"next\""),
        "{body}"
    );
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn heavy_non_visit_collection_hits_scan_cap() {
    let mut repo = one_publication();
    let mut docs: Vec<(String, Value)> = (0..110)
        .map(|i| {
            (
                format!("p{:03}", 999 - i),
                plain_doc("pub1", &format!("Blog {i}")),
            )
        })
        .collect();
    docs.push((
        "a000".into(),
        visit_doc("pub1", "Buried Visit Post", "Deep Cut", &[]),
    ));
    repo.documents = docs;
    let server = mount(&repo).await;
    let (status, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub1/"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains("Buried Visit Post"), "{body}");
    assert!(body.contains("There may be older ones"), "{body}");
    assert!(body.contains("rel=\"next\""), "{body}");
}

#[tokio::test]
async fn document_page_renders_card_body_tags_and_canonical() {
    let server = mount(&one_publication()).await;
    let (status, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub1/d3"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("<title>Third Post — Ross Writes — eaten.at</title>"),
        "{body}"
    );
    assert!(
        body.contains("<link rel=\"canonical\" href=\"https://ross.eaten.at/2026/09/third-post\">"),
        "{body}"
    );
    assert!(body.contains("class=\"place-name\">Third Place<"), "{body}");
    assert!(
        body.contains("<time datetime=\"2026-09-06\">September 6, 2026</time><span class=\"visit-meal\">Dinner</span><span class=\"price\" aria-label=\"price band 2 of 4\">$$</span>"),
        "{body}"
    );
    assert!(
        body.contains("class=\"place-address\">1 Example St<"),
        "{body}"
    );
    assert!(
        body.contains("<span class=\"rating-marks\" aria-hidden=\"true\">++</span> <span class=\"rating-word\">Recommended</span>"),
        "{body}"
    );
    assert!(body.contains("<h2>Heading</h2>"), "{body}");
    assert!(body.contains("<em>Third Place</em>"), "{body}");
    assert!(
        body.contains(">Official site<"),
        "known service label: {body}"
    );
    assert!(
        body.contains(">example.com<"),
        "unknown service labelled by host: {body}"
    );
    assert!(
        body.contains("href=\"https://www.google.com/maps/place/?q=place_id:g1\" rel=\"ugc nofollow noopener\">Google Maps</a>"),
        "a known id becomes a map link: {body}"
    );
    assert!(body.contains("longform"), "{body}");
}

#[tokio::test]
async fn document_that_is_not_a_visit_renders_without_card() {
    let server = mount(&one_publication()).await;
    let (status, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub1/d2"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains("visit-card"), "{body}");
    assert!(body.contains("Just a blog post."), "{body}");
}

#[tokio::test]
async fn document_from_another_publication_is_404_under_this_one() {
    let mut repo = one_publication();
    repo.publications
        .push(("pub2", publication("Side Blog", "https://side.example")));
    let server = mount(&repo).await;
    let (status, _, _) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub2/d3"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn error_statuses_and_pages() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, StaticDns::new());
    let (status, _, body) = get(&state, "/at/nope/").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body.contains("<main"), "{body}");
    let (status, _, _) = get(&state, &format!("/at/{OTHER_DID}/")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = get(&state, &format!("/at/{DID}/nopub/")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = get(&state, &format!("/at/{DID}/pub1/nodoc")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn did_without_pds_is_502() {
    let server = mount(&Repo {
        no_pds: true,
        ..Repo::default()
    })
    .await;
    let (status, _, _) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/"),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
}

#[tokio::test]
async fn warm_publication_page_makes_no_upstream_requests() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, StaticDns::new());
    let (status, _, _) = get(&state, &format!("/at/{DID}/pub1/")).await;
    assert_eq!(status, StatusCode::OK);
    let before = server.received_requests().await.unwrap().len();
    for _ in 0..3 {
        let (status, _, _) = get(&state, &format!("/at/{DID}/pub1/")).await;
        assert_eq!(status, StatusCode::OK);
    }
    let after = server.received_requests().await.unwrap().len();
    assert_eq!(before, after, "warm pages hit upstream");
}

#[tokio::test]
async fn stylesheet_is_served_with_cache_headers() {
    let server = mount(&Repo::default()).await;
    let state = state_for(&server, StaticDns::new());
    let hashed = eaten_at_web::assets::css_path();
    let response = router(state.clone())
        .oneshot(Request::get(&hashed).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/css; charset=utf-8"
    );
    assert!(response.headers()[header::CACHE_CONTROL]
        .to_str()
        .unwrap()
        .contains("immutable"));
    let (status, _, _) = get(&state, "/static/app.css").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = get(&state, "/static/other.css").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, _, landing) = get(&state, "/").await;
    assert!(
        landing.contains(&format!("<link rel=\"stylesheet\" href=\"{hashed}\">")),
        "{landing}"
    );
}

#[tokio::test]
async fn publication_theme_is_clamped_and_emitted_as_numbers() {
    let mut repo = one_publication();
    // Pale gray text on white: fails contrast and must be darkened.
    repo.publications[0].1["basicTheme"] = json!({
        "background": {"r": 255, "g": 255, "b": 255},
        "foreground": {"r": 200, "g": 200, "b": 200},
        "accent": {"r": 0, "g": 100, "b": 160},
        "accentForeground": {"r": 255, "g": 255, "b": 255}
    });
    let server = mount(&repo).await;
    let state = state_for(&server, StaticDns::new());
    for uri in [format!("/at/{DID}/pub1/"), format!("/at/{DID}/pub1/d3")] {
        let (status, _, body) = get(&state, &uri).await;
        assert_eq!(status, StatusCode::OK);
        assert!(body.contains("data-theme=\"publication\""), "{body}");
        let start = body.find(":root[data-theme]{").unwrap();
        let rule = &body[start..body[start..].find('}').unwrap() + start];
        assert!(rule.contains("--theme-bg: 255 255 255;"), "{rule}");
        assert!(
            !rule.contains("--theme-fg: 200 200 200;"),
            "foreground must be clamped: {rule}"
        );
        assert!(rule.contains("--theme-accent: 0 100 160;"), "{rule}");
    }
}

#[tokio::test]
async fn malformed_theme_reads_as_no_theme() {
    let mut repo = one_publication();
    repo.publications[0].1["basicTheme"] = json!({"background": {"r": 999, "g": 0, "b": 0}});
    let server = mount(&repo).await;
    let (status, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub1/"),
    )
    .await;
    // The publication itself must not fail to load because of its theme.
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("data-theme"), "{body}");
}

fn png_bytes(w: u32, h: u32) -> Vec<u8> {
    use image::{DynamicImage, ImageFormat, RgbImage};
    let img = RgbImage::from_pixel(w, h, image::Rgb([200, 30, 30]));
    let mut out = std::io::Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(img)
        .write_to(&mut out, ImageFormat::Png)
        .unwrap();
    out.into_inner()
}

fn visit_doc_with_cover(pub_rkey: &str, mime: &str) -> Value {
    let mut doc = visit_doc(pub_rkey, "Covered", "Covered Place", &[]);
    doc["coverImage"] =
        json!({"$type": "blob", "ref": {"$link": "bafkcover"}, "mimeType": mime, "size": 100});
    doc
}

async fn mount_blob(server: &MockServer, content_type: &str, bytes: Vec<u8>) {
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.sync.getBlob"))
        .and(query_param("cid", "bafkcover"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(bytes, content_type))
        .mount(server)
        .await;
}

async fn get_image(state: &AppState, uri: &str) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let response = router(state.clone())
        .oneshot(Request::get(uri).body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, headers, body)
}

fn jpeg_dimensions(bytes: &[u8]) -> (u32, u32) {
    use image::GenericImageView;
    assert_eq!(&bytes[..2], &[0xff, 0xd8], "not a JPEG");
    image::load_from_memory(bytes).unwrap().dimensions()
}

#[tokio::test]
async fn cover_proxy_serves_blob_as_jpeg_with_safe_headers() {
    let mut repo = one_publication();
    repo.documents
        .insert(0, ("cov".into(), visit_doc_with_cover("pub1", "image/png")));
    let server = mount(&repo).await;
    mount_blob(&server, "image/png", png_bytes(1000, 1000)).await;
    let state = state_for(&server, StaticDns::new());
    let (status, headers, body) = get_image(&state, &format!("/img/{DID}/cov")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(
        headers[header::CONTENT_DISPOSITION],
        "inline; filename=\"cover.jpg\""
    );
    assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
    assert!(headers[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap()
        .contains("sandbox"));
    assert_eq!(jpeg_dimensions(&body), (800, 800));

    let (status, _, og) = get_image(&state, &format!("/img/{DID}/cov?size=og")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(jpeg_dimensions(&og), (1200, 630));

    // The card and listing point at the proxy, never at the PDS.
    let (_, _, page) = get(&state, &format!("/at/{DID}/pub1/cov")).await;
    assert!(page.contains(&format!("src=\"/img/{DID}/cov\"")), "{page}");
    assert!(!page.contains("getBlob"), "{page}");
}

#[tokio::test]
async fn svg_blob_is_refused_and_chain_falls_through_to_placeholder() {
    let mut repo = one_publication();
    repo.documents.insert(
        0,
        ("cov".into(), visit_doc_with_cover("pub1", "image/svg+xml")),
    );
    let server = mount(&repo).await;
    mount_blob(
        &server,
        "image/svg+xml",
        b"<svg xmlns='http://www.w3.org/2000/svg'><script>1</script></svg>".to_vec(),
    )
    .await;
    let state = state_for(&server, StaticDns::new());
    let (status, _, body) = get_image(&state, &format!("/img/{DID}/cov")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(jpeg_dimensions(&body), (600, 600), "placeholder size");
    // Deterministic: the same document yields the same bytes.
    let (_, _, again) = get_image(&state, &format!("/img/{DID}/cov")).await;
    assert_eq!(body, again);
}

#[tokio::test]
async fn mislabelled_blob_is_decoded_by_content_not_declared_type() {
    // Declared PNG, actually an SVG: sniffing must reject it.
    let mut repo = one_publication();
    repo.documents
        .insert(0, ("cov".into(), visit_doc_with_cover("pub1", "image/png")));
    let server = mount(&repo).await;
    mount_blob(
        &server,
        "image/png",
        b"<svg xmlns='http://www.w3.org/2000/svg'/>".to_vec(),
    )
    .await;
    let state = state_for(&server, StaticDns::new());
    let (status, _, body) = get_image(&state, &format!("/img/{DID}/cov")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        jpeg_dimensions(&body),
        (600, 600),
        "fell through to placeholder"
    );
}

#[tokio::test]
async fn cover_proxy_404s_for_missing_or_non_visit_documents() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, StaticDns::new());
    let (status, _, _) = get_image(&state, &format!("/img/{DID}/nope")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = get_image(&state, &format!("/img/{DID}/d2")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn tag_pages_filter_within_the_publication_and_match_loosely() {
    let mut repo = one_publication();
    repo.documents = vec![
        (
            "t3".into(),
            visit_doc("pub1", "Loud One", "Loud Place", &["Longform", "long read"]),
        ),
        (
            "t2".into(),
            visit_doc("pub1", "Quiet One", "Quiet Place", &["short"]),
        ),
        (
            "t1".into(),
            visit_doc("pub1", "Old One", "Old Place", &["longform"]),
        ),
        ("x1".into(), plain_doc("pub1", "Blog post")),
    ];
    let server = mount(&repo).await;
    let state = state_for(&server, StaticDns::new());

    let (status, _, body) = get(&state, &format!("/at/{DID}/pub1/tagged/LONGFORM")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("Loud One") && body.contains("Old One"),
        "{body}"
    );
    assert!(!body.contains("Quiet One"), "{body}");
    assert!(
        body.contains("Tagged “Longform”"),
        "display form from the first match: {body}"
    );
    assert!(body.contains("in this publication only"), "{body}");

    let (status, _, body) = get(&state, &format!("/at/{DID}/pub1/tagged/long%20%20read")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("Loud One") && !body.contains("Old One"),
        "{body}"
    );

    let (status, _, body) = get(&state, &format!("/at/{DID}/pub1/tagged/nothing-here")).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "unknown tag is an empty state, not a 404"
    );
    assert!(
        body.contains("Nothing in this publication is tagged"),
        "{body}"
    );

    let (status, _, body) = get(&state, &format!("/at/{DID}/pub1/tagged/%23longform")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        !body.contains("Loud One"),
        "a leading # never matches: {body}"
    );
}

#[tokio::test]
async fn tag_links_appear_on_document_and_publication_pages() {
    let mut repo = one_publication();
    repo.documents = vec![
        (
            "t3".into(),
            visit_doc("pub1", "Loud One", "Loud Place", &["Longform", "long read"]),
        ),
        (
            "t1".into(),
            visit_doc("pub1", "Old One", "Old Place", &["longform"]),
        ),
    ];
    let server = mount(&repo).await;
    let state = state_for(&server, StaticDns::new());
    let tag_href = format!("/at/{DID}/pub1/tagged/Longform");
    let (_, _, doc) = get(&state, &format!("/at/{DID}/pub1/t3")).await;
    assert!(doc.contains(&format!("href=\"{tag_href}\"")), "{doc}");
    assert!(
        doc.contains(&format!("/at/{DID}/pub1/tagged/long%20read")),
        "{doc}"
    );
    assert!(doc.contains("in this publication"), "{doc}");
    let (_, _, pub_page) = get(&state, &format!("/at/{DID}/pub1/")).await;
    assert_eq!(
        pub_page.matches("class=\"tag\"").count(),
        2,
        "distinct tags, first spelling: {pub_page}"
    );
    assert!(
        pub_page.contains(">Longform<") && !pub_page.contains(">longform<"),
        "{pub_page}"
    );
}

/// The `<head>` of a page, with the hashed stylesheet path made stable.
fn head_of(body: &str) -> String {
    let start = body.find("<head>").unwrap();
    let end = body.find("</head>").unwrap() + "</head>".len();
    let re = regex_lite::Regex::new(r"app\.[0-9a-f]{8}\.css").unwrap();
    re.replace_all(&body[start..end], "app.HASH.css")
        .replace("><", ">\n<")
}

#[tokio::test]
async fn document_head_hosted_publication() {
    let mut repo = one_publication();
    repo.documents[0].1["updatedAt"] = Value::String("2026-09-08T09:00:00.000Z".into());
    repo.documents[0].1["description"] = Value::String("  The author's own excerpt.  ".into());
    let server = mount(&repo).await;
    let (_, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub1/d3"),
    )
    .await;
    insta::assert_snapshot!(head_of(&body));
}

#[tokio::test]
async fn document_head_byo_domain_points_canonical_away_and_derives_description() {
    let mut repo = one_publication();
    repo.publications[0].1["url"] = Value::String("https://records.example.net".into());
    let server = mount(&repo).await;
    let (_, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub1/d3"),
    )
    .await;
    let head = head_of(&body);
    assert!(
        head.contains(
            "<link rel=\"canonical\" href=\"https://records.example.net/2026/09/third-post\">"
        ),
        "{head}"
    );
    assert!(
        head.contains(
            "<meta property=\"og:url\" content=\"https://records.example.net/2026/09/third-post\">"
        ),
        "{head}"
    );
    assert!(
        head.contains("content=\"A write-up of Third Place.\""),
        "derived excerpt: {head}"
    );
    assert!(!head.contains("modified_time"), "{head}");
    assert!(head.contains("<meta property=\"og:image\" content=\"https://eaten.at/img/did:plc:re3ebnp5v7ffagz6rb6xfei4/d3?size=og\">"), "{head}");
    assert!(
        head.contains("<meta property=\"og:title\" content=\"Third Post\">"),
        "{head}"
    );
    insta::assert_snapshot!(head);
}

#[tokio::test]
async fn document_without_path_uses_site_route_as_canonical() {
    let mut repo = one_publication();
    repo.documents[0].1.as_object_mut().unwrap().remove("path");
    let server = mount(&repo).await;
    let (_, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub1/d3"),
    )
    .await;
    assert!(
        body.contains(&format!(
            "<link rel=\"canonical\" href=\"https://eaten.at/at/{DID}/pub1/d3\">"
        )),
        "{body}"
    );
}

#[tokio::test]
async fn publication_head_and_icon() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, StaticDns::new());
    let (_, _, body) = get(&state, &format!("/at/{DID}/pub1/")).await;
    let head = head_of(&body);
    assert!(
        head.contains("<meta property=\"og:type\" content=\"website\">"),
        "{head}"
    );
    assert!(
        head.contains("<link rel=\"canonical\" href=\"https://ross.eaten.at\">"),
        "{head}"
    );
    assert!(
        head.contains(&format!(
            "content=\"https://eaten.at/img/{DID}/pub1?kind=icon\""
        )),
        "{head}"
    );
    insta::assert_snapshot!(head);
    let (status, headers, bytes) = get_image(&state, &format!("/img/{DID}/pub1?kind=icon")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(jpeg_dimensions(&bytes), (1200, 630));
}

#[tokio::test]
async fn feed_lists_visit_documents_with_canonical_links() {
    let mut repo = one_publication();
    repo.documents[0].1["title"] = Value::String("Third & <Place>".into());
    let server = mount(&repo).await;
    let state = state_for(&server, StaticDns::new());
    let response = router(state.clone())
        .oneshot(
            Request::get(format!("/at/{DID}/pub1/feed.xml"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/rss+xml; charset=utf-8"
    );
    let body = String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(body.matches("<item>").count(), 2, "{body}");
    assert!(
        body.contains("<link>https://ross.eaten.at/2026/09/third-post</link>"),
        "{body}"
    );
    assert!(
        body.contains("<title>Third &amp; &lt;Place&gt;</title>"),
        "{body}"
    );
    assert!(
        body.contains("<description>A write-up of Third Place.</description>"),
        "{body}"
    );
    assert!(
        body.contains(&format!("url=\"https://eaten.at/img/{DID}/d3?size=og\"")),
        "{body}"
    );
    assert!(
        body.contains(&format!(
            "href=\"https://eaten.at/at/{DID}/pub1/feed.xml\" rel=\"self\""
        )),
        "{body}"
    );
    assert!(!body.contains("Plain Post"), "{body}");
    insta::assert_snapshot!(body);
}

#[tokio::test]
async fn feed_for_empty_publication_is_valid() {
    let mut repo = one_publication();
    repo.documents.clear();
    let server = mount(&repo).await;
    let (status, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub1/feed.xml"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("<channel>") && !body.contains("<item>"),
        "{body}"
    );
}

async fn get_with_cookie(state: &AppState, uri: &str, cookie: &str) -> (StatusCode, String) {
    let response = router(state.clone())
        .oneshot(
            Request::get(uri)
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

#[tokio::test]
async fn labeled_document_shows_interstitial_until_acknowledged() {
    let mut repo = one_publication();
    repo.documents[0].1["labels"] = json!({"$type": "com.atproto.label.defs#selfLabels",
                                           "values": [{"val": "graphic-media"}, {"val": "spoilers"}]});
    let server = mount(&repo).await;
    let state = state_for(&server, StaticDns::new());
    let uri = format!("/at/{DID}/pub1/d3");

    let (status, _, body) = get(&state, &uri).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("Before you read this"), "{body}");
    assert!(
        body.contains("<strong>graphic media</strong> and <strong>spoilers</strong>"),
        "{body}"
    );
    assert!(
        body.contains(&format!("name=\"return\" value=\"{uri}\"")),
        "{body}"
    );
    assert!(
        !body.contains("A write-up of"),
        "content must not leak: {body}"
    );
    assert!(
        body.contains("<button type=\"submit\">"),
        "keyboard-operable control: {body}"
    );

    let (status, body) = get_with_cookie(&state, &uri, "ea_show_labeled=1").await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("A write-up of"), "{body}");
    assert!(!body.contains("Before you read this"), "{body}");

    // Unlabeled documents are never gated.
    let (_, _, body) = get(&state, &format!("/at/{DID}/pub1/d1")).await;
    assert!(!body.contains("Before you read this"), "{body}");
}

#[tokio::test]
async fn labeled_publication_gates_its_pages_and_cover_proxy_is_not_gated() {
    let mut repo = one_publication();
    repo.publications[0].1["labels"] = json!({"values": [{"val": "porn"}]});
    let server = mount(&repo).await;
    let state = state_for(&server, StaticDns::new());
    let (status, _, body) = get(&state, &format!("/at/{DID}/pub1/")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("adult content"), "{body}");
    assert!(!body.contains("Third Post"), "{body}");
    let (_, _, body) = get(&state, &format!("/at/{DID}/pub1/d3")).await;
    assert!(
        body.contains("Before you read this"),
        "publication label gates documents too: {body}"
    );
    let (status, headers, _) = get_image(&state, &format!("/img/{DID}/d3")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/jpeg");
}

#[tokio::test]
async fn malformed_labels_read_as_unlabeled() {
    let mut repo = one_publication();
    repo.documents[0].1["labels"] = json!({"values": "not-a-list"});
    let server = mount(&repo).await;
    let (status, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub1/d3"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("A write-up of"), "{body}");
}

#[tokio::test]
async fn acknowledging_sets_a_session_cookie_and_returns_only_to_local_paths() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, StaticDns::new());
    let post = |body: &'static str, proto: Option<&'static str>| {
        let mut req = Request::post("/labels/continue")
            .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
        if let Some(p) = proto {
            req = req.header("x-forwarded-proto", p);
        }
        req.body(Body::from(body)).unwrap()
    };
    let response = router(state.clone())
        .oneshot(post("return=%2Fat%2Fdid%3Aplc%3Ax%2Fp%2Fd", None))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/at/did:plc:x/p/d");
    let cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(cookie, "ea_show_labeled=1; Path=/; HttpOnly; SameSite=Lax");

    let response = router(state.clone())
        .oneshot(post("return=https%3A%2F%2Fevil.example%2F", Some("https")))
        .await
        .unwrap();
    assert_eq!(
        response.headers()[header::LOCATION],
        "/",
        "open redirect refused"
    );
    assert!(response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .ends_with("; Secure"));
    let response = router(state)
        .oneshot(post("return=%2F%2Fevil.example%2F", None))
        .await
        .unwrap();
    assert_eq!(response.headers()[header::LOCATION], "/");
}

#[tokio::test]
async fn security_headers_on_every_route_family() {
    let mut repo = one_publication();
    repo.publications[0].1["basicTheme"] = json!({
        "background": {"r": 255, "g": 255, "b": 255}, "foreground": {"r": 0, "g": 0, "b": 0},
        "accent": {"r": 0, "g": 100, "b": 160}, "accentForeground": {"r": 255, "g": 255, "b": 255}
    });
    let server = mount(&repo).await;
    let state = state_for(&server, StaticDns::new());
    let routes = [
        "/".to_owned(),
        format!("/at/{DID}/pub1/"),
        format!("/at/{DID}/pub1/d3"),
        format!("/at/{DID}/pub1/tagged/longform"),
        format!("/at/{DID}/pub1/feed.xml"),
        eaten_at_web::assets::css_path(),
        "/at/nope/".to_owned(),
        format!("/at/{DID}/pub1/missing"),
    ];
    for uri in routes {
        let response = router(state.clone())
            .oneshot(Request::get(&uri).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let headers = response.headers().clone();
        let csp = headers[header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .to_owned();
        assert!(
            csp.starts_with("default-src 'self'; script-src 'nonce-"),
            "{uri}: {csp}"
        );
        assert!(!csp.contains("unsafe-inline"), "{uri}: {csp}");
        assert_eq!(
            headers[header::REFERRER_POLICY],
            "strict-origin-when-cross-origin",
            "{uri}"
        );
        assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff", "{uri}");
        assert_eq!(headers[header::X_FRAME_OPTIONS], "DENY", "{uri}");
        assert!(headers.contains_key("permissions-policy"), "{uri}");

        let nonce = csp
            .split("'nonce-")
            .nth(1)
            .unwrap()
            .split('\'')
            .next()
            .unwrap()
            .to_owned();
        assert_eq!(nonce.len(), 32, "{uri}");
        let body =
            String::from_utf8_lossy(&response.into_body().collect().await.unwrap().to_bytes())
                .into_owned();
        // Every inline style or script carries this request's nonce.
        for tag in ["<style", "<script"] {
            for occurrence in body.match_indices(tag) {
                let snippet = &body[occurrence.0..occurrence.0 + 60];
                assert!(
                    snippet.contains(&format!("nonce=\"{nonce}\"")),
                    "{uri}: {snippet}"
                );
            }
        }
    }
    // The document page really did carry the themed style block. No page
    // ships a script until the editor island (C3.5); the loop above will
    // cover it when it does.
    let (_, _, doc) = get(&state, &format!("/at/{DID}/pub1/d3")).await;
    assert!(doc.contains("<style nonce="), "{doc}");
    assert!(!doc.contains("<script"), "{doc}");
}

#[tokio::test]
async fn nonces_differ_per_request_and_image_proxy_keeps_its_own_csp() {
    let mut repo = one_publication();
    repo.documents
        .insert(0, ("cov".into(), visit_doc_with_cover("pub1", "image/png")));
    let server = mount(&repo).await;
    mount_blob(&server, "image/png", png_bytes(10, 10)).await;
    let state = state_for(&server, StaticDns::new());
    let mut seen = std::collections::HashSet::new();
    for _ in 0..3 {
        let response = router(state.clone())
            .oneshot(Request::get("/").body(Body::empty()).unwrap())
            .await
            .unwrap();
        seen.insert(
            response.headers()[header::CONTENT_SECURITY_POLICY]
                .to_str()
                .unwrap()
                .to_owned(),
        );
    }
    assert_eq!(seen.len(), 3);
    let (_, headers, _) = get_image(&state, &format!("/img/{DID}/cov")).await;
    assert_eq!(
        headers[header::CONTENT_SECURITY_POLICY],
        "default-src 'none'; sandbox"
    );
}

// ---- the editor (C3.2) ----

/// A browser session for `DID`, as the cookie header to send.
async fn signed_in(state: &AppState) -> String {
    let did = eaten_at_atproto::identity::Did::parse(DID).unwrap();
    let token = state.sessions().create(&did).await.unwrap();
    format!("ea_session={token}")
}

/// A multipart body with text fields and, optionally, one file.
fn multipart(fields: &[(&str, &str)], file: Option<(&str, &str, &[u8])>) -> (String, Vec<u8>) {
    const BOUNDARY: &str = "----eaten-at-test-boundary";
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!("--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n")
                .as_bytes(),
        );
    }
    if let Some((name, filename, bytes)) = file {
        body.extend_from_slice(
            format!(
                "--{BOUNDARY}\r\nContent-Disposition: form-data; name=\"{name}\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
            )
            .as_bytes(),
        );
        body.extend_from_slice(bytes);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{BOUNDARY}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={BOUNDARY}"), body)
}

async fn post_editor(
    state: &AppState,
    uri: &str,
    cookie: &str,
    fields: &[(&str, &str)],
    file: Option<(&str, &str, &[u8])>,
) -> (StatusCode, String) {
    let (content_type, body) = multipart(fields, file);
    let response = router(state.clone())
        .oneshot(
            Request::post(uri)
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, content_type)
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8_lossy(&body).into_owned())
}

async fn get_signed(
    state: &AppState,
    uri: &str,
    cookie: &str,
) -> (StatusCode, Option<String>, String) {
    let response = router(state.clone())
        .oneshot(
            Request::get(uri)
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let location = response
        .headers()
        .get(header::LOCATION)
        .map(|v| v.to_str().unwrap().to_owned());
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        location,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

fn good_fields<'a>() -> Vec<(&'a str, &'a str)> {
    vec![
        ("title", "A room with the lights off"),
        ("body", "Forty-six *minutes*.\n\nNine notes."),
        ("place_name", "Promises"),
        ("place_price", "2"),
        ("visited_on", "2026-09-08"),
        ("meal", "dinner"),
        ("rating", "3"),
        ("id_service_0", "googlePlace"),
        ("id_value_0", "g1"),
        ("link_url_0", "https://example.com/official"),
        ("link_service_0", "officialSite"),
        ("link_service_other_0", ""),
        ("link_label_0", ""),
        ("tags", "#notes, Short"),
        (
            "publication",
            "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1",
        ),
        ("action", "preview"),
    ]
}

#[tokio::test]
async fn editor_requires_sign_in_and_prefills_the_publication() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, dns_for_handle());
    let (status, location, _) = get(&state, "/write").await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(location.as_deref(), Some("/login?return_to=%2Fwrite"));

    let cookie = signed_in(&state).await;
    let (status, _, body) = get_signed(&state, "/write", &cookie).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("enctype=\"multipart/form-data\""), "{body}");
    assert!(
        body.contains(&format!(
            "<option value=\"at://{DID}/site.standard.publication/pub1\" selected>Ross Writes</option>"
        )),
        "the only publication is preselected: {body}"
    );
    assert!(body.contains("name=\"link_url_0\""), "{body}");
    assert!(body.contains("value=\"preview\""), "{body}");
    // A new write-up opens straight on the form: no kicker, no heading.
    assert!(!body.contains("<h1"), "{body}");
    assert!(!body.contains("A new write-up"), "{body}");
}

#[tokio::test]
async fn editor_rows_grow_and_shrink_without_javascript() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = signed_in(&state).await;
    let mut fields = good_fields();
    fields.retain(|(k, _)| *k != "action");
    fields.push(("action", "add_link"));
    let (status, body) = post_editor(&state, "/write", &cookie, &fields, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("name=\"link_url_1\""), "{body}");
    assert!(body.contains("value=\"remove_link:1\""), "{body}");
    assert!(
        body.contains("value=\"Promises\""),
        "typed values survive: {body}"
    );
    assert!(
        body.contains("<option value=\"officialSite\" selected>"),
        "the chosen service stays selected: {body}"
    );
    assert!(
        !body.contains("class=\"preview\""),
        "a structural action does not validate"
    );
}

#[tokio::test]
async fn editor_reports_problems_beside_fields_and_previews_a_good_draft() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = signed_in(&state).await;

    let mut fields = good_fields();
    fields.retain(|(k, _)| *k != "title" && *k != "place_name" && *k != "visited_on");
    fields.push(("title", "   "));
    fields.push(("place_name", " "));
    fields.push(("visited_on", "yesterday"));
    let (status, body) = post_editor(&state, "/write", &cookie, &fields, None).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    // A blank title is not a problem (D29); a blank place name is.
    assert!(!body.contains("id=\"title-error\""), "{body}");
    assert!(body.contains("id=\"place_name-error\""), "{body}");
    assert!(body.contains("id=\"visited_on-error\""), "{body}");
    assert!(
        body.contains("aria-describedby=\"place_name-error\""),
        "{body}"
    );
    assert!(body.contains("2 things to fix below"), "{body}");

    let (status, body) = post_editor(&state, "/write", &cookie, &good_fields(), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("class=\"preview\""), "{body}");
    assert!(
        body.contains("<em>minutes</em>"),
        "markdown rendered: {body}"
    );
    assert!(body.contains("class=\"place-name\">Promises<"), "{body}");
    assert!(
        body.contains("<span class=\"rating-marks\" aria-hidden=\"true\">+++</span> <span class=\"rating-word\">Strongly Recommended</span>"),
        "{body}"
    );
    assert!(body.contains(">Official site</a>"), "{body}");
    assert!(
        body.contains(">notes</a>") && body.contains(">Short</a>"),
        "{body}"
    );
    assert!(
        body.contains("value=\"A room with the lights off\""),
        "the form is still there: {body}"
    );

    let too_big = vec![0u8; 5 * 1024 * 1024 + 1];
    let (status, body) = post_editor(
        &state,
        "/write",
        &cookie,
        &good_fields(),
        Some(("cover", "big.png", &too_big)),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{}",
        &body[..body.len().min(400)]
    );
    assert!(body.contains("id=\"cover-error\""), "{body}");
    let (status, body) = post_editor(
        &state,
        "/write",
        &cookie,
        &good_fields(),
        Some(("cover", "x.txt", b"not an image")),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert!(body.contains("isn't an image"), "{body}");
}

#[tokio::test]
async fn editing_prefills_from_the_document_and_keeps_foreign_values() {
    let mut repo = one_publication();
    let mut foreign = visit_doc("pub1", "Foreign Post", "Foreign Place", &["Tape"]);
    foreign["content"]["place"]["urls"] =
        json!([{"url": "https://x.example/a", "service": "bc", "rank": 1}]);
    foreign["content"]["meal"] = json!("tea");
    repo.documents.push(("d9".into(), foreign));
    let server = mount(&repo).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = signed_in(&state).await;

    let (status, _, body) = get_signed(&state, "/write/d9", &cookie).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("value=\"Foreign Post\""), "{body}");
    assert!(body.contains("value=\"Foreign Place\""), "{body}");
    assert!(body.contains("value=\"2026-09-06\""), "{body}");
    assert!(body.contains("<option value=\"other\" selected>"), "{body}");
    assert!(body.contains("value=\"bc\""), "{body}");
    assert!(body.contains("value=\"tea\""), "{body}");
    assert!(
        body.contains("value=\"2\" checked"),
        "the rating is preselected: {body}"
    );
    assert!(body.contains("value=\"Tape\""), "{body}");
    assert!(body.contains("action=\"/write/d9\""), "{body}");
    assert!(body.contains("A write-up of *Foreign Place*."), "{body}");
    // Editing opens with the write-up's own title as the heading.
    assert!(
        body.contains("<p class=\"kicker\">Edit</p><h1>Foreign Post</h1>"),
        "{body}"
    );

    let fields = [
        ("title", "Foreign Post"),
        ("body", "A write-up of *Foreign Place*."),
        ("place_name", "Foreign Place"),
        ("visited_on", "2026-09-06"),
        ("link_url_0", "https://x.example/a"),
        ("link_service_0", "other"),
        ("link_service_other_0", "bc"),
        ("tags", "Tape"),
        (
            "publication",
            "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1",
        ),
        ("action", "preview"),
    ];
    let (status, body) = post_editor(&state, "/write/d9", &cookie, &fields, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("class=\"preview\""), "{body}");
    assert!(
        body.contains(">x.example</a>"),
        "an unknown service is labelled by host: {body}"
    );

    let (status, _, body) = get_signed(&state, "/write/d2", &cookie).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "not a visit: {body}");
    let (status, _, _) = get_signed(&state, "/write/nope", &cookie).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

// ---- publishing (C3.3) ----

/// The signed-in author's OAuth session, and the PDS's write endpoints.
async fn mount_writes(server: &MockServer) {
    let uri = server.uri();
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-authorization-server"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "issuer": uri,
            "authorization_endpoint": format!("{uri}/oauth/authorize"),
            "token_endpoint": format!("{uri}/oauth/token"),
            "pushed_authorization_request_endpoint": format!("{uri}/oauth/par"),
            "response_types_supported": ["code"],
            "grant_types_supported": ["authorization_code", "refresh_token"],
            "code_challenge_methods_supported": ["S256"],
            "token_endpoint_auth_methods_supported": ["none"],
            "dpop_signing_alg_values_supported": ["ES256"],
            "scopes_supported": ["atproto"]
        })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.createRecord"))
        .and(wiremock::matchers::body_string_contains(
            "\"collection\":\"site.standard.publication\"",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "uri": format!("at://{DID}/site.standard.publication/newpub"), "cid": "bafypub"
        })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.createRecord"))
        .and(wiremock::matchers::body_string_contains(
            "\"collection\":\"site.standard.document\"",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "uri": format!("at://{DID}/site.standard.document/newdoc"), "cid": "bafydoc"
        })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.putRecord"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "uri": format!("at://{DID}/x/y"), "cid": "bafyput"
        })))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.deleteRecord"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.uploadBlob"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "blob": {"$type": "blob", "ref": {"$link": "bafyblob"}, "mimeType": "image/png", "size": 70}
        })))
        .mount(server)
        .await;
}

async fn author_session(state: &AppState, server: &MockServer) -> String {
    let did = eaten_at_atproto::identity::Did::parse(DID).unwrap();
    eaten_at_atproto::oauth::testing::seed_session(
        state.oauth_store(),
        &did,
        &server.uri(),
        &server.uri(),
        "tok1",
    )
    .await;
    signed_in(state).await
}

/// The XRPC writes the PDS received, in order: (method, JSON body).
async fn repo_writes(server: &MockServer) -> Vec<(String, Value)> {
    server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .filter(|r| r.method == "POST" && r.url.path().starts_with("/xrpc/com.atproto.repo."))
        .map(|r| {
            let nsid = r.url.path().trim_start_matches("/xrpc/").to_owned();
            assert_eq!(
                r.headers.get("authorization").unwrap().to_str().unwrap(),
                "DPoP tok1"
            );
            assert!(r.headers.contains_key("dpop"));
            let body = serde_json::from_slice(&r.body).unwrap_or(Value::Null);
            (nsid, body)
        })
        .collect()
}

fn this_month() -> String {
    let now = jiff::Timestamp::now().to_zoned(jiff::tz::TimeZone::UTC);
    format!("/{:04}/{:02}", now.year(), now.month())
}

#[tokio::test]
async fn first_publish_creates_the_publication_and_preferences_then_the_document() {
    let server = mount(&Repo::default()).await;
    mount_writes(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = author_session(&state, &server).await;

    let mut fields = good_fields();
    fields.retain(|(k, _)| !matches!(*k, "publication" | "action"));
    fields.push(("publication", "new"));
    fields.push(("new_publication_name", "Liner Notes"));
    fields.push(("new_publication_url", "https://notes.alice.test/"));
    fields.push(("action", "publish"));
    let (status, body) = post_editor(&state, "/write", &cookie, &fields, None).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");

    let writes = repo_writes(&server).await;
    let names: Vec<&str> = writes.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "com.atproto.repo.createRecord",
            "com.atproto.repo.putRecord",
            "com.atproto.repo.createRecord"
        ]
    );
    let publication = &writes[0].1;
    assert_eq!(publication["collection"], "site.standard.publication");
    assert_eq!(publication["record"]["url"], "https://notes.alice.test");
    assert_eq!(publication["record"]["name"], "Liner Notes");
    let prefs = &writes[1].1;
    assert_eq!(prefs["collection"], "at.eaten.preferences");
    assert_eq!(prefs["rkey"], "self");
    assert_eq!(
        prefs["record"]["defaultPublication"],
        format!("at://{DID}/site.standard.publication/newpub")
    );
    let doc = &writes[2].1["record"];
    assert_eq!(doc["$type"], "site.standard.document");
    assert_eq!(
        doc["site"],
        format!("at://{DID}/site.standard.publication/newpub")
    );
    assert_eq!(
        doc["path"],
        format!("{}/a-room-with-the-lights-off", this_month())
    );
    assert_eq!(doc["content"]["$type"], "at.eaten.visit");
    assert_eq!(
        doc["content"]["body"]["text"]["markdown"],
        "Forty-six *minutes*.\n\nNine notes."
    );
    assert_eq!(
        doc["textContent"],
        "Promises · 2026-09-08 · Strongly Recommended\n\nForty-six minutes.\n\nNine notes."
    );
    assert_eq!(doc["content"]["place"]["name"], "Promises");
    assert_eq!(doc["content"]["place"]["price"], 2);
    assert_eq!(doc["content"]["place"]["ids"][0]["id"], "g1");
    assert_eq!(
        doc["content"]["place"]["urls"][0]["service"],
        "officialSite"
    );
    assert_eq!(doc["content"]["visitedOn"], "2026-09-08");
    assert_eq!(doc["content"]["meal"], "dinner");
    assert_eq!(doc["content"]["rating"], 3);
    assert!(doc.get("links").is_none(), "{doc}");
    assert_eq!(doc["tags"], json!(["notes", "Short"]));
    assert!(doc.get("description").is_none());
    assert!(doc.get("updatedAt").is_none());
}

#[tokio::test]
async fn publishing_to_an_existing_publication_writes_only_the_document() {
    let mut repo = one_publication();
    repo.publications
        .push(("pub2", publication("Second", "https://two.alice.test")));
    repo.preferences = Some(json!({
        "$type": "at.eaten.preferences",
        "defaultPublication": format!("at://{DID}/site.standard.publication/pub1"),
        "createdAt": "2026-01-01T00:00:00.000Z"
    }));
    let server = mount(&repo).await;
    mount_writes(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = author_session(&state, &server).await;

    let mut fields = good_fields();
    fields.retain(|(k, _)| !matches!(*k, "title" | "action"));
    fields.push(("title", "Third Post"));
    fields.push(("action", "publish"));
    let (status, body) = post_editor(&state, "/write", &cookie, &fields, None).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    let writes = repo_writes(&server).await;
    assert_eq!(writes.len(), 1, "{writes:?}");
    let doc = &writes[0].1["record"];
    assert_eq!(
        doc["site"],
        format!("at://{DID}/site.standard.publication/pub1")
    );
    assert_eq!(
        doc["path"],
        format!("{}/third-post-2", this_month()),
        "the seeded Third Post already holds /third-post this month"
    );
}

#[tokio::test]
async fn editing_replaces_the_record_and_deleting_removes_it() {
    let mut repo = one_publication();
    repo.preferences = Some(json!({
        "$type": "at.eaten.preferences",
        "createdAt": "2026-01-01T00:00:00.000Z"
    }));
    let server = mount(&repo).await;
    mount_writes(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = author_session(&state, &server).await;

    let fields = [
        ("title", "Third Post, revisited"),
        ("body", "New words."),
        ("place_name", "Third Place"),
        ("visited_on", "2026-09-06"),
        ("link_url_0", "https://example.com/official"),
        ("link_service_0", "officialSite"),
        ("tags", "longform"),
        (
            "publication",
            "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1",
        ),
        ("action", "publish"),
    ];
    let (status, body) = post_editor(&state, "/write/d3", &cookie, &fields, None).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    let writes = repo_writes(&server).await;
    assert_eq!(writes.len(), 1);
    let (name, put) = &writes[0];
    assert_eq!(name, "com.atproto.repo.putRecord");
    assert_eq!(put["collection"], "site.standard.document");
    assert_eq!(put["rkey"], "d3");
    let doc = &put["record"];
    assert_eq!(doc["title"], "Third Post, revisited");
    assert_eq!(doc["path"], "/2026/09/third-post", "the path never changes");
    assert_eq!(doc["publishedAt"], "2026-09-07T12:00:00.000Z");
    assert!(doc.get("updatedAt").is_some(), "{doc}");
    assert_eq!(
        doc["content"]["place"]["urls"][0]["service"],
        "officialSite"
    );
    assert_eq!(
        doc["content"]["place"]["urls"][0]["url"],
        "https://example.com/official"
    );
    assert_eq!(doc["content"]["visitedOn"], "2026-09-06");
    assert!(doc["content"].get("rating").is_none(), "unrated now: {doc}");

    let (status, _, page) = get_signed(&state, "/write/d3/delete", &cookie).await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("Delete “Third Post”?"), "{page}");
    let (status, body) = post_editor(&state, "/write/d3/delete", &cookie, &[], None).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    let writes = repo_writes(&server).await;
    let (name, del) = writes.last().unwrap();
    assert_eq!(name, "com.atproto.repo.deleteRecord");
    assert_eq!(del["rkey"], "d3");
}

#[tokio::test]
async fn a_cover_is_uploaded_and_referenced() {
    let server = mount(&one_publication()).await;
    mount_writes(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = author_session(&state, &server).await;

    let mut png = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(4, 4, image::Rgb([200, 30, 30])))
        .write_to(&mut png, image::ImageFormat::Png)
        .unwrap();
    let png = png.into_inner();
    let (status, body) = post_editor(
        &state,
        "/write",
        &cookie,
        &good_fields(),
        Some(("cover", "cover.png", &png)),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("src=\"data:image/png;base64,"),
        "the preview shows the new cover: {body}"
    );

    let mut fields = good_fields();
    fields.retain(|(k, _)| *k != "action");
    fields.push(("action", "publish"));
    let (status, _) = post_editor(
        &state,
        "/write",
        &cookie,
        &fields,
        Some(("cover", "cover.png", &png)),
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let requests = server.received_requests().await.unwrap();
    let upload = requests
        .iter()
        .find(|r| r.url.path() == "/xrpc/com.atproto.repo.uploadBlob")
        .expect("the cover was uploaded");
    assert_eq!(upload.headers.get("content-type").unwrap(), "image/png");
    assert_eq!(upload.body, png);
    let doc = repo_writes(&server).await.pop().unwrap().1;
    assert_eq!(doc["record"]["coverImage"]["ref"]["$link"], "bafyblob");
}

#[tokio::test]
async fn the_author_sees_an_edit_link_on_their_document() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = signed_in(&state).await;
    let (_, _, anonymous) = get(&state, &format!("/at/{DID}/pub1/d3")).await;
    assert!(!anonymous.contains("href=\"/write/d3\""));
    let (_, _, mine) = get_signed(&state, &format!("/at/{DID}/pub1/d3"), &cookie).await;
    assert!(mine.contains("<a href=\"/write/d3\">edit</a>"), "{mine}");
}

// ---- the tags field (C3.4) ----

#[tokio::test]
async fn the_tags_field_offers_no_suggestions_of_its_own() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = signed_in(&state).await;

    // Tags are the author's own vocabulary: no datalist.
    let (_, _, body) = get_signed(&state, "/write", &cookie).await;
    assert!(body.contains("name=\"tags\""), "{body}");
    assert!(!body.contains("<datalist"), "{body}");
    assert!(!body.contains("list=\"genres\""), "{body}");
}

// ---- the editor island (C3.5) ----

#[tokio::test]
async fn the_editor_carries_one_nonced_script_and_nothing_else_changes() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = signed_in(&state).await;
    let response = router(state.clone())
        .oneshot(
            Request::get("/write")
                .header(header::COOKIE, &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let csp = response.headers()[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap()
        .to_owned();
    let nonce = csp
        .split("script-src 'nonce-")
        .nth(1)
        .and_then(|rest| rest.split('\'').next())
        .unwrap()
        .to_owned();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8_lossy(&body).into_owned();
    let script = format!(
        "<script nonce=\"{nonce}\">{}</script>",
        eaten_at_web::assets::EDITOR_SCRIPT
    );
    assert_eq!(body.matches("<script").count(), 1, "{body}");
    assert!(
        body.contains(&script),
        "the island is inlined under the page's nonce"
    );
    let without = body.replace(&script, "");
    assert!(!without.contains("<script"));
    assert!(
        without.contains("<form class=\"editor\""),
        "the form stands on its own"
    );
    assert!(
        !without.contains("class=\"notice restore\""),
        "the banner is the script's to add"
    );
}

// ---- settings and hosted subdomains (C3.6) ----

async fn get_host(state: &AppState, host: &str, uri: &str) -> (StatusCode, Option<String>, String) {
    let response = router(state.clone())
        .oneshot(
            Request::get(uri)
                .header(header::HOST, host)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let location = response
        .headers()
        .get(header::LOCATION)
        .map(|v| v.to_str().unwrap().to_owned());
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        location,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

async fn post_form_signed(
    state: &AppState,
    uri: &str,
    cookie: &str,
    form: &str,
) -> (StatusCode, Option<String>, String) {
    let response = router(state.clone())
        .oneshot(
            Request::post(uri)
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(form.to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let location = response
        .headers()
        .get(header::LOCATION)
        .map(|v| v.to_str().unwrap().to_owned());
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        location,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

#[tokio::test]
async fn settings_claims_a_subdomain_and_rewrites_the_publication_url() {
    let mut repo = one_publication();
    repo.publications
        .push(("pub2", publication("Second", "https://two.alice.test")));
    let server = mount(&repo).await;
    mount_writes(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = author_session(&state, &server).await;

    let (status, _, body) = get_signed(&state, "/settings", &cookie).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Served by you at ross.eaten.at"), "{body}");
    assert!(body.contains("action=\"/settings/pub1/hosting\""), "{body}");
    assert!(
        body.contains("<span class=\"meta\">.eaten.at</span>"),
        "{body}"
    );

    for (form, message) in [
        (
            "mode=hosted&name=-bad-",
            "not starting or ending with a hyphen",
        ),
        ("mode=hosted&name=www", "reserved"),
        ("mode=own&url=http://insecure.example", "https address"),
        ("mode=", "Choose where"),
    ] {
        let (status, _, body) =
            post_form_signed(&state, "/settings/pub1/hosting", &cookie, form).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{form}: {body}");
        assert!(body.contains(message), "{form}: {body}");
    }

    let (status, location, body) = post_form_signed(
        &state,
        "/settings/pub1/hosting",
        &cookie,
        "mode=hosted&name=Records",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    assert_eq!(location.as_deref(), Some("/settings"));
    let writes = repo_writes(&server).await;
    let (name, put) = writes.last().unwrap();
    assert_eq!(name, "com.atproto.repo.putRecord");
    assert_eq!(put["collection"], "site.standard.publication");
    assert_eq!(put["rkey"], "pub1");
    assert_eq!(put["record"]["url"], "https://records.eaten.at");
    assert_eq!(
        put["record"]["name"], "Ross Writes",
        "the rest of the record is kept"
    );
    assert_eq!(put["record"]["$type"], "site.standard.publication");

    // The second publication cannot take the same name.
    let (status, _, body) = post_form_signed(
        &state,
        "/settings/pub2/hosting",
        &cookie,
        "mode=hosted&name=records",
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(body.contains("that name is taken"), "{body}");
    assert!(body.contains("Hosted here at records.eaten.at"), "{body}");
}

#[tokio::test]
async fn a_claimed_subdomain_is_served_as_the_publication_origin() {
    let server = mount(&one_publication()).await;
    let state = state_for(&server, dns_for_handle());
    let did = eaten_at_atproto::identity::Did::parse(DID).unwrap();
    let uri = eaten_at_atproto::at_uri::AtUri::parse(&format!(
        "at://{DID}/site.standard.publication/pub1"
    ))
    .unwrap();
    state.claims().claim("ross", &uri, &did).await.unwrap();

    let (status, _, body) = get_host(&state, "ross.eaten.at", "/").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("class=\"nameplate-name\">Ross Writes<"),
        "{body}"
    );

    let (status, _, body) = get_host(&state, "ross.eaten.at", "/2026/09/third-post").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("<h1 class=\"doc-title\">Third Post</h1>"),
        "{body}"
    );

    let (status, _, body) = get_host(&state, "ross.eaten.at", "/feed.xml").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("<rss"), "{body}");

    let (status, _, body) = get_host(&state, "ross.eaten.at", "/tagged/longform").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body.contains("Third Post"), "{body}");

    let (status, _, body) = get_host(
        &state,
        "ross.eaten.at",
        "/.well-known/site.standard.publication",
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let doc: Value = serde_json::from_str(&body).unwrap();
    assert_eq!(doc["$type"], "site.standard.publication");
    assert_eq!(doc["name"], "Ross Writes");
    assert_eq!(doc["url"], "https://ross.eaten.at");
    assert_eq!(doc["description"], "Ross Writes description");

    let (status, _, _) = get_host(&state, "ross.eaten.at", "/static/app.css").await;
    assert_ne!(status, StatusCode::NOT_FOUND, "assets pass through");
    let (status, _, _) = get_host(&state, "ross.eaten.at", "/2026/09/nope").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = get_host(&state, "nobody.eaten.at", "/").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, _) = get_host(&state, "deep.ross.eaten.at", "/").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "only one level of subdomain");
    let (status, _, _) = get_host(&state, "eaten.at", "/").await;
    assert_eq!(status, StatusCode::OK, "the bare host is the site");

    state
        .claims()
        .record_move("old.eaten.at", "https://ross.example/")
        .await
        .unwrap();
    let (status, location, _) = get_host(&state, "old.eaten.at", "/2026/09/x?a=b").await;
    assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        location.as_deref(),
        Some("https://ross.example/2026/09/x?a=b")
    );
}

#[tokio::test]
async fn moving_to_an_own_domain_releases_the_claim_and_redirects_the_old_host() {
    let server = mount(&one_publication()).await;
    mount_writes(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = author_session(&state, &server).await;
    let did = eaten_at_atproto::identity::Did::parse(DID).unwrap();
    let uri = eaten_at_atproto::at_uri::AtUri::parse(&format!(
        "at://{DID}/site.standard.publication/pub1"
    ))
    .unwrap();
    state.claims().claim("ross", &uri, &did).await.unwrap();

    let (status, _, body) = post_form_signed(
        &state,
        "/settings/pub1/hosting",
        &cookie,
        "mode=own&url=https%3A%2F%2Fross.example%2F",
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    let (_, put) = repo_writes(&server).await.pop().unwrap();
    assert_eq!(put["record"]["url"], "https://ross.example");
    assert!(state
        .claims()
        .for_publication(&uri)
        .await
        .unwrap()
        .is_none());
    let (status, location, _) = get_host(&state, "ross.eaten.at", "/").await;
    assert_eq!(status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(location.as_deref(), Some("https://ross.example/"));
}

// ---- Bluesky comment threads (C4.1) ----

const POST_URI: &str = "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/app.bsky.feed.post/3kroot";

/// A document whose comment thread is a Bluesky post.
fn commented_repo() -> Repo {
    let mut doc = visit_doc("pub1", "Third Post", "Third Place", &[]);
    doc["bskyPostRef"] = json!({"uri": POST_URI, "cid": "bafyroot"});
    Repo {
        publications: vec![("pub1", publication("Ross Writes", "https://ross.eaten.at"))],
        documents: vec![("d3".into(), doc)],
        ..Repo::default()
    }
}

fn thread_mock() -> wiremock::MockBuilder {
    Mock::given(method("GET"))
        .and(path("/xrpc/app.bsky.feed.getPostThread"))
        .and(query_param("uri", POST_URI))
}

#[tokio::test]
async fn document_shows_its_bluesky_thread_as_plain_text() {
    let server = mount(&commented_repo()).await;
    let thread: Value = serde_json::from_str(include_str!("fixtures/bsky-thread.json")).unwrap();
    thread_mock()
        .and(query_param("parentHeight", "0"))
        .respond_with(ResponseTemplate::new(200).set_body_json(thread))
        .expect(1)
        .mount(&server)
        .await;
    let state = state_for(&server, StaticDns::new());
    let (status, _, body) = get(&state, &format!("/at/{DID}/pub1/d3")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("<h2 class=\"kicker\" id=\"comments-heading\">Comments on Bluesky</h2>"),
        "{body}"
    );
    // Reading order: Bob, Alice's answer nested, Bob's answer nested
    // further, then Carol. The hidden, blocked, and deleted replies are
    // not there, nor is the reply to the hidden one.
    let order: Vec<usize> = ["Loved *this*", "Thanks Bob!", "Any time.", "Same here."]
        .iter()
        .map(|needle| {
            body.find(needle)
                .unwrap_or_else(|| panic!("{needle} missing: {body}"))
        })
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{order:?}");
    assert!(!body.contains("Buy followers"), "{body}");
    assert!(!body.contains("Reported."), "{body}");
    assert!(
        body.contains("<li class=\"comment depth-1\">")
            && body.contains("<li class=\"comment depth-2\">"),
        "{body}"
    );
    // Plain text: markdown literal, tags escaped, the line break kept.
    assert!(
        body.contains("Loved *this* &lt;b&gt;record&lt;/b&gt;\nand the write-up."),
        "{body}"
    );
    assert!(
        body.contains("<a class=\"comment-author\" href=\"https://bsky.app/profile/bob.test\" rel=\"ugc nofollow noopener\">Bob</a><span class=\"comment-handle\">@bob.test</span>"),
        "{body}"
    );
    assert!(
        body.contains("href=\"https://bsky.app/profile/bob.test/post/3kbob1\""),
        "{body}"
    );
    assert!(
        body.contains(&format!("<a href=\"https://bsky.app/profile/{DID}/post/3kroot\" rel=\"noopener\">Reply on Bluesky →</a>")),
        "{body}"
    );
    // The footer's link is still there too.
    assert!(body.contains("Comments on Bluesky →"), "{body}");

    // A second visit within the cache window costs no AppView call
    // (the mock expects exactly one).
    let (status, _, again) = get(&state, &format!("/at/{DID}/pub1/d3")).await;
    assert_eq!(status, StatusCode::OK);
    assert!(again.contains("Thanks Bob!"), "{again}");
}

#[tokio::test]
async fn deleted_or_unreadable_post_leaves_only_the_link() {
    // Deleted: the AppView answers 400 NotFound.
    let server = mount(&commented_repo()).await;
    thread_mock()
        .respond_with(
            ResponseTemplate::new(400)
                .set_body_json(json!({"error": "NotFound", "message": "Post not found"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let state = state_for(&server, StaticDns::new());
    let (status, _, body) = get(&state, &format!("/at/{DID}/pub1/d3")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("class=\"comments\""), "{body}");
    assert!(
        body.contains(&format!("href=\"https://bsky.app/profile/{DID}/post/3kroot\" rel=\"noopener\">Comments on Bluesky →</a>")),
        "{body}"
    );
    // The absence is cached too.
    let (status, _, _) = get(&state, &format!("/at/{DID}/pub1/d3")).await;
    assert_eq!(status, StatusCode::OK);

    // Blocked: a blockedPost at the root.
    let server = mount(&commented_repo()).await;
    thread_mock()
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "thread": {"$type": "app.bsky.feed.defs#blockedPost", "uri": POST_URI,
                       "blocked": true, "author": {"did": DID}}
        })))
        .mount(&server)
        .await;
    let (status, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub1/d3"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("class=\"comments\""), "{body}");
    assert!(body.contains("Comments on Bluesky →"), "{body}");

    // Unreachable: a 500 is not cached, and the page is still fine.
    let server = mount(&commented_repo()).await;
    thread_mock()
        .respond_with(ResponseTemplate::new(500))
        .expect(2)
        .mount(&server)
        .await;
    let state = state_for(&server, StaticDns::new());
    for _ in 0..2 {
        let (status, _, body) = get(&state, &format!("/at/{DID}/pub1/d3")).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert!(!body.contains("class=\"comments\""), "{body}");
        assert!(body.contains("Comments on Bluesky →"), "{body}");
    }
}

#[tokio::test]
async fn document_without_a_post_reference_never_asks_the_appview() {
    let server = mount(&one_publication()).await;
    Mock::given(method("GET"))
        .and(path("/xrpc/app.bsky.feed.getPostThread"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({})))
        .expect(0)
        .mount(&server)
        .await;
    let (status, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub1/d3"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(!body.contains("Comments on Bluesky"), "{body}");
}

#[tokio::test]
async fn post_with_no_replies_invites_the_first() {
    let server = mount(&commented_repo()).await;
    thread_mock()
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "thread": {"$type": "app.bsky.feed.defs#threadViewPost",
                       "post": {"uri": POST_URI, "cid": "bafyroot",
                                "author": {"did": DID, "handle": HANDLE},
                                "record": {"text": "root", "createdAt": "2026-09-07T12:05:00.000Z"},
                                "indexedAt": "2026-09-07T12:05:01.000Z"},
                       "replies": []}
        })))
        .mount(&server)
        .await;
    let (status, _, body) = get(
        &state_for(&server, StaticDns::new()),
        &format!("/at/{DID}/pub1/d3"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body.contains("<p class=\"empty comments-empty\">No comments yet.</p>"),
        "{body}"
    );
    assert!(body.contains(">Reply on Bluesky →<"), "{body}");
}

// ---- Bluesky crossposting (C4.2) ----

const POST_SCOPE: &str = eaten_at::auth::CROSSPOST_SCOPE;

/// A signed-in author whose grant includes the crosspost scope.
async fn posting_author_session(state: &AppState, server: &MockServer) -> String {
    let did = eaten_at_atproto::identity::Did::parse(DID).unwrap();
    eaten_at_atproto::oauth::testing::seed_session_with_scope(
        state.oauth_store(),
        &did,
        &server.uri(),
        &server.uri(),
        "tok1",
        &eaten_at::auth::all_scopes().join(" "),
    )
    .await;
    signed_in(state).await
}

/// A form-urlencoded POST with the session cookie.
async fn post_form(
    state: &AppState,
    uri: &str,
    cookie: &str,
    fields: &[(&str, &str)],
) -> (StatusCode, Option<String>, String) {
    let body = fields
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                eaten_at_web::layout::urlencoding(k),
                eaten_at_web::layout::urlencoding(v)
            )
        })
        .collect::<Vec<_>>()
        .join("&");
    let response = router(state.clone())
        .oneshot(
            Request::post(uri)
                .header(header::COOKIE, cookie)
                .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
                .body(Body::from(body))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let location = response
        .headers()
        .get(header::LOCATION)
        .map(|v| v.to_str().unwrap().to_owned());
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        location,
        String::from_utf8_lossy(&body).into_owned(),
    )
}

/// The document a publish creates, as the PDS hands it back at the key
/// the createRecord mock mints.
fn new_document() -> Value {
    visit_doc("pub1", "A room with the lights off", "Promises", &["notes"])
}

/// `getRecord` for the freshly created document; ahead of the catch-all
/// not-found mock.
async fn mount_new_document(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.getRecord"))
        .and(query_param("collection", "site.standard.document"))
        .and(query_param("rkey", "newdoc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(record(
            "site.standard.document",
            "newdoc",
            &new_document(),
        )))
        .with_priority(2)
        .mount(server)
        .await;
}

fn post_uri() -> String {
    format!("at://{DID}/app.bsky.feed.post/newdoc")
}

/// `createRecord` for a Bluesky post succeeds.
async fn mount_post_created(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.createRecord"))
        .and(wiremock::matchers::body_string_contains(
            "app.bsky.feed.post",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(json!({"uri": post_uri(), "cid": "bafypost"})),
        )
        .mount(server)
        .await;
}

/// `getRecord` finds the post an earlier attempt created at the
/// document's key.
async fn mount_post_exists(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.getRecord"))
        .and(query_param("collection", "app.bsky.feed.post"))
        .and(query_param("rkey", "newdoc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "uri": post_uri(), "cid": "bafypost",
            "value": {"$type": "app.bsky.feed.post", "text": "earlier", "createdAt": "2026-09-09T15:00:00.000Z"}
        })))
        .with_priority(1)
        .mount(server)
        .await;
}

fn publish_with_crosspost(text: &str) -> Vec<(&str, &str)> {
    let mut fields = good_fields();
    fields.retain(|(k, _)| *k != "action");
    fields.push(("crosspost", "1"));
    fields.push(("post_text", text));
    fields.push(("action", "publish"));
    fields
}

fn post_writes(writes: &[(String, Value)]) -> Vec<&Value> {
    writes
        .iter()
        .filter(|(n, body)| {
            n == "com.atproto.repo.createRecord" && body["collection"] == "app.bsky.feed.post"
        })
        .map(|(_, body)| body)
        .collect()
}

#[tokio::test]
async fn crosspost_writes_document_then_post_then_the_reference() {
    let server = mount(&one_publication()).await;
    mount_writes(&server).await;
    mount_new_document(&server).await;
    mount_post_created(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = posting_author_session(&state, &server).await;

    let (status, body) = post_editor(
        &state,
        "/write",
        &cookie,
        &publish_with_crosspost("Listen to this one"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    assert!(body.is_empty() || !body.contains("crosspost"), "{body}");

    let writes = repo_writes(&server).await;
    let names: Vec<&str> = writes.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(
        names,
        [
            "com.atproto.repo.putRecord",    // first publish: the preferences
            "com.atproto.repo.createRecord", // the document
            "com.atproto.repo.uploadBlob",   // the card's thumb
            "com.atproto.repo.createRecord", // the post
            "com.atproto.repo.putRecord",    // the document, now with bskyPostRef
        ],
        "{writes:?}"
    );
    let prefs = &writes[0].1;
    assert_eq!(prefs["collection"], "at.eaten.preferences");
    assert_eq!(prefs["record"]["crosspostToBluesky"], true);
    assert_eq!(
        prefs["record"]["defaultPublication"],
        format!("at://{DID}/site.standard.publication/pub1"),
        "one write carries both"
    );
    assert_eq!(writes[1].1["collection"], "site.standard.document");

    let post = &writes[3].1;
    assert_eq!(post["collection"], "app.bsky.feed.post");
    assert_eq!(post["rkey"], "newdoc", "the post takes the document's key");
    let record = &post["record"];
    assert_eq!(record["$type"], "app.bsky.feed.post");
    assert_eq!(record["text"], "Listen to this one");
    assert!(record["createdAt"].is_string());
    assert_eq!(record["embed"]["$type"], "app.bsky.embed.external");
    let external = &record["embed"]["external"];
    assert_eq!(
        external["uri"],
        "https://ross.eaten.at/2026/09/a-room-with-the-lights-off"
    );
    assert_eq!(external["title"], "A room with the lights off");
    assert_eq!(
        external["description"], "A write-up of Promises.",
        "the summary: first paragraph, frozen into the card (plan §7.5)"
    );
    assert_eq!(external["thumb"]["ref"]["$link"], "bafyblob");

    let reference = &writes[4].1;
    assert_eq!(reference["collection"], "site.standard.document");
    assert_eq!(reference["rkey"], "newdoc");
    let mut expected = new_document();
    expected["bskyPostRef"] = json!({"uri": post_uri(), "cid": "bafypost"});
    assert_eq!(
        reference["record"], expected,
        "the reference is the only change; updatedAt is not stamped"
    );
}

#[tokio::test]
async fn retry_after_a_failed_reference_reuses_the_post() {
    let server = mount(&one_publication()).await;
    // The first putRecord carrying bskyPostRef fails.
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.putRecord"))
        .and(wiremock::matchers::body_string_contains("bskyPostRef"))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    mount_writes(&server).await;
    mount_new_document(&server).await;
    mount_post_created(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = posting_author_session(&state, &server).await;

    let (status, body) = post_editor(
        &state,
        "/write",
        &cookie,
        &publish_with_crosspost("Listen"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    // The document is published; the page offers a retry with the text kept.
    let (status, location, _) = get_signed(
        &state,
        "/write/newdoc/crosspost?text=Listen&failed=1",
        &cookie,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{location:?}");
    let (_, _, page) = get_signed(
        &state,
        "/write/newdoc/crosspost?text=Listen&failed=1",
        &cookie,
    )
    .await;
    assert!(page.contains("Bluesky didn't accept the post"), "{page}");
    assert!(page.contains("value=\"Listen\""), "{page}");
    assert!(page.contains(">Post to Bluesky<"), "{page}");

    // On retry the post from the first attempt is found, not remade.
    mount_post_exists(&server).await;
    let (status, location, body) = post_form(
        &state,
        "/write/newdoc/crosspost",
        &cookie,
        &[("post_text", "Listen")],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    assert_eq!(
        location.as_deref(),
        Some(&*format!("/at/{DID}/pub1/newdoc"))
    );
    let writes = repo_writes(&server).await;
    assert_eq!(post_writes(&writes).len(), 1, "{writes:?}");
    let (name, last) = writes.last().unwrap();
    assert_eq!(name, "com.atproto.repo.putRecord");
    assert_eq!(last["record"]["bskyPostRef"]["uri"], post_uri());
}

#[tokio::test]
async fn retry_after_a_failed_post_creates_exactly_one() {
    let server = mount(&one_publication()).await;
    Mock::given(method("POST"))
        .and(path("/xrpc/com.atproto.repo.createRecord"))
        .and(wiremock::matchers::body_string_contains(
            "app.bsky.feed.post",
        ))
        .respond_with(ResponseTemplate::new(500))
        .up_to_n_times(1)
        .with_priority(1)
        .mount(&server)
        .await;
    mount_writes(&server).await;
    mount_new_document(&server).await;
    mount_post_created(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = posting_author_session(&state, &server).await;

    let (status, body) = post_editor(
        &state,
        "/write",
        &cookie,
        &publish_with_crosspost("Listen"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    let writes = repo_writes(&server).await;
    assert!(
        !writes
            .iter()
            .any(|(_, b)| b["record"].get("bskyPostRef").is_some()),
        "no reference without a post: {writes:?}"
    );

    let (status, location, body) = post_form(
        &state,
        "/write/newdoc/crosspost",
        &cookie,
        &[("post_text", "Listen")],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    assert_eq!(
        location.as_deref(),
        Some(&*format!("/at/{DID}/pub1/newdoc"))
    );
    let writes = repo_writes(&server).await;
    let posts = post_writes(&writes);
    assert_eq!(posts.len(), 2, "one refused, one accepted: {writes:?}");
    assert!(posts.iter().all(|p| p["rkey"] == "newdoc"));
    let (name, last) = writes.last().unwrap();
    assert_eq!(name, "com.atproto.repo.putRecord");
    assert_eq!(last["record"]["bskyPostRef"]["cid"], "bafypost");

    // Once referenced, another submit writes nothing more.
    let referenced = {
        let mut doc = new_document();
        doc["bskyPostRef"] = json!({"uri": post_uri(), "cid": "bafypost"});
        doc
    };
    Mock::given(method("GET"))
        .and(path("/xrpc/com.atproto.repo.getRecord"))
        .and(query_param("collection", "site.standard.document"))
        .and(query_param("rkey", "newdoc"))
        .respond_with(ResponseTemplate::new(200).set_body_json(record(
            "site.standard.document",
            "newdoc",
            &referenced,
        )))
        .with_priority(1)
        .mount(&server)
        .await;
    let before = repo_writes(&server).await.len();
    let (status, _, _) = post_form(
        &state,
        "/write/newdoc/crosspost",
        &cookie,
        &[("post_text", "Again")],
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(repo_writes(&server).await.len(), before);
    let (_, _, page) = get_signed(&state, "/write/newdoc/crosspost", &cookie).await;
    assert!(page.contains("is on Bluesky"), "{page}");
    assert!(page.contains("See the thread on Bluesky"), "{page}");
}

#[tokio::test]
async fn crosspost_toggle_defaults_from_preferences() {
    let server = mount(&Repo {
        preferences: Some(
            json!({"crosspostToBluesky": true, "createdAt": "2026-09-01T00:00:00.000Z"}),
        ),
        ..one_publication()
    })
    .await;
    mount_writes(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = posting_author_session(&state, &server).await;
    let (status, _, page) = get_signed(&state, "/write", &cookie).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        page.contains("name=\"crosspost\" type=\"checkbox\" value=\"1\" checked"),
        "{page}"
    );
    assert!(page.contains("placeholder=\"\""), "no place yet: {page}");

    let server = mount(&one_publication()).await;
    mount_writes(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = posting_author_session(&state, &server).await;
    let (_, _, page) = get_signed(&state, "/write", &cookie).await;
    assert!(
        page.contains("name=\"crosspost\" type=\"checkbox\" value=\"1\">"),
        "{page}"
    );
    // A document that already names a post shows the link, not the toggle.
    let mut doc = visit_doc("pub1", "Third Post", "Third Place", &[]);
    doc["bskyPostRef"] =
        json!({"uri": format!("at://{DID}/app.bsky.feed.post/3kpost"), "cid": "bafy"});
    let server = mount(&Repo {
        documents: vec![("d3".into(), doc)],
        ..one_publication()
    })
    .await;
    mount_writes(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = posting_author_session(&state, &server).await;
    let (_, _, page) = get_signed(&state, "/write/d3", &cookie).await;
    assert!(!page.contains("name=\"crosspost\""), "{page}");
    assert!(
        page.contains(&format!(
            "Posted to Bluesky: <a href=\"https://bsky.app/profile/{DID}/post/3kpost\""
        )),
        "{page}"
    );
}

/// Percent-decode a form-urlencoded value.
fn form_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < bytes.len() => {
                out.push(u8::from_str_radix(&s[i + 1..i + 3], 16).unwrap());
                i += 2;
            }
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8(out).unwrap()
}

#[tokio::test]
async fn without_permission_publish_hands_over_to_the_crosspost_page_which_asks() {
    let server = mount(&one_publication()).await;
    mount_writes(&server).await;
    mount_new_document(&server).await;
    mount_post_created(&server).await;
    Mock::given(method("GET"))
        .and(path("/.well-known/oauth-protected-resource"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "resource": server.uri(), "authorization_servers": [server.uri()], "scopes_supported": []
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/oauth/par"))
        .respond_with(ResponseTemplate::new(201).set_body_json(
            json!({"request_uri": "urn:ietf:params:oauth:request_uri:abc", "expires_in": 60}),
        ))
        .mount(&server)
        .await;
    let state = state_for(&server, dns_for_handle());
    // The sign-in grant only.
    let cookie = author_session(&state, &server).await;
    let (_, _, editor) = get_signed(&state, "/write", &cookie).await;
    assert!(editor.contains("ask you to allow posting"), "{editor}");

    let (status, body) = post_editor(
        &state,
        "/write",
        &cookie,
        &publish_with_crosspost("Listen to this"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    assert!(post_writes(&repo_writes(&server).await).is_empty());
    let (status, _, page) = get_signed(
        &state,
        "/write/newdoc/crosspost?text=Listen%20to%20this",
        &cookie,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(page.contains("needs your account's permission"), "{page}");
    assert!(page.contains("action=\"/login/bluesky\""), "{page}");
    assert!(
        page.contains(
            "name=\"return_to\" value=\"/write/newdoc/crosspost?text=Listen%20to%20this\""
        ),
        "{page}"
    );

    // Allowing starts an authorization for the DID with the larger scope set.
    let (status, _, page) = post_form(
        &state,
        "/login/bluesky",
        &cookie,
        &[(
            "return_to",
            "/write/newdoc/crosspost?text=Listen%20to%20this",
        )],
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{page}");
    assert!(page.contains("http-equiv=\"refresh\""), "{page}");
    let par = server
        .received_requests()
        .await
        .unwrap()
        .into_iter()
        .find(|r| r.url.path() == "/oauth/par")
        .expect("a pushed authorization request");
    let body = form_decode(&String::from_utf8_lossy(&par.body));
    assert!(body.contains(POST_SCOPE), "{body}");
    assert!(body.contains("repo:site.standard.document"), "{body}");
    assert!(body.contains(&format!("login_hint={DID}")), "{body}");

    // A plain sign-in by an account that never opted in asks for the
    // sign-in set only; once the stored grant includes posting, signing
    // in again asks for the full set so the grant is not dropped.
    let par_scopes = |requests: Vec<wiremock::Request>| -> Vec<String> {
        requests
            .into_iter()
            .filter(|r| r.url.path() == "/oauth/par")
            .map(|r| form_decode(&String::from_utf8_lossy(&r.body)))
            .collect()
    };
    let before = par_scopes(server.received_requests().await.unwrap()).len();
    let (status, _, _) = post_form(&state, "/login", "", &[("handle", HANDLE)]).await;
    assert_eq!(status, StatusCode::OK);
    let bodies = par_scopes(server.received_requests().await.unwrap());
    assert_eq!(bodies.len(), before + 1);
    assert!(!bodies[before].contains(POST_SCOPE), "{}", bodies[before]);
    let _ = posting_author_session(&state, &server).await;
    let (status, _, _) = post_form(&state, "/login", "", &[("handle", HANDLE)]).await;
    assert_eq!(status, StatusCode::OK);
    let bodies = par_scopes(server.received_requests().await.unwrap());
    assert_eq!(bodies.len(), before + 2);
    assert!(
        bodies[before + 1].contains(POST_SCOPE),
        "{}",
        bodies[before + 1]
    );

    // The metadata document declares the crosspost scope too.
    let (status, _, metadata) = get(&state, "/client-metadata.json").await;
    assert_eq!(status, StatusCode::OK);
    let metadata: Value = serde_json::from_str(&metadata).unwrap();
    assert!(
        metadata["scope"].as_str().unwrap().contains(POST_SCOPE),
        "{metadata}"
    );
}

#[tokio::test]
async fn delete_offers_to_delete_the_post_when_it_may() {
    let mut doc = visit_doc("pub1", "Third Post", "Third Place", &[]);
    doc["bskyPostRef"] =
        json!({"uri": format!("at://{DID}/app.bsky.feed.post/3kpost"), "cid": "bafy"});
    let repo = || Repo {
        documents: vec![("d3".into(), doc.clone())],
        ..one_publication()
    };

    let server = mount(&repo()).await;
    mount_writes(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = posting_author_session(&state, &server).await;
    let (_, _, page) = get_signed(&state, "/write/d3/delete", &cookie).await;
    assert!(
        page.contains("name=\"delete_post\" type=\"checkbox\" value=\"1\" checked"),
        "{page}"
    );
    let (status, location, body) =
        post_form(&state, "/write/d3/delete", &cookie, &[("delete_post", "1")]).await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    assert_eq!(location.as_deref(), Some(&*format!("/at/{DID}/pub1/")));
    let deletes: Vec<(String, String)> = repo_writes(&server)
        .await
        .into_iter()
        .filter(|(n, _)| n == "com.atproto.repo.deleteRecord")
        .map(|(_, b)| {
            (
                b["collection"].as_str().unwrap().to_owned(),
                b["rkey"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    assert_eq!(
        deletes,
        [
            ("app.bsky.feed.post".to_owned(), "3kpost".to_owned()),
            ("site.standard.document".to_owned(), "d3".to_owned())
        ]
    );

    // Unticked: the post stays.
    let server = mount(&repo()).await;
    mount_writes(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = posting_author_session(&state, &server).await;
    let (status, _, _) = post_form(&state, "/write/d3/delete", &cookie, &[]).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let writes = repo_writes(&server).await;
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(writes[0].1["collection"], "site.standard.document");

    // Without the scope the page says the post stays, and the box is not there.
    let server = mount(&repo()).await;
    mount_writes(&server).await;
    let state = state_for(&server, dns_for_handle());
    let cookie = author_session(&state, &server).await;
    let (_, _, page) = get_signed(&state, "/write/d3/delete", &cookie).await;
    assert!(!page.contains("name=\"delete_post\""), "{page}");
    assert!(page.contains("may not delete posts"), "{page}");
    let (status, _, _) =
        post_form(&state, "/write/d3/delete", &cookie, &[("delete_post", "1")]).await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let writes = repo_writes(&server).await;
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert_eq!(writes[0].1["collection"], "site.standard.document");
}
