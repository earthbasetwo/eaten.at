//! Guarded client behaviour against a mock server.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use eaten_at_atproto::http::{GuardedClient, HttpError, Policy, StaticHosts};
use reqwest::StatusCode;
use url::Url;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn client_with(policy: Policy, hosts: StaticHosts) -> GuardedClient {
    GuardedClient::new(policy, Arc::new(hosts), "eaten-at-tests").unwrap()
}

fn client() -> GuardedClient {
    client_with(Policy::for_tests(), StaticHosts::new())
}

fn url(server: &MockServer, p: &str) -> Url {
    Url::parse(&format!("{}{p}", server.uri())).unwrap()
}

#[tokio::test]
async fn returns_status_headers_and_body_without_treating_4xx_as_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("hello")
                .insert_header("content-type", "text/plain"),
        )
        .mount(&server)
        .await;
    let response = client().get(url(&server, "/ok")).await.unwrap();
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.text(), "hello");
    assert_eq!(response.content_type(), Some("text/plain"));

    let missing = client().get(url(&server, "/missing")).await.unwrap();
    assert_eq!(missing.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn follows_redirects_and_reports_final_url() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/start"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/middle"))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/middle"))
        .respond_with(
            ResponseTemplate::new(308)
                .insert_header("location", &format!("{}/end#frag", server.uri())),
        )
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/end"))
        .respond_with(ResponseTemplate::new(200).set_body_string("done"))
        .mount(&server)
        .await;
    let response = client().get(url(&server, "/start")).await.unwrap();
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.text(), "done");
    assert_eq!(response.url, url(&server, "/end"));
}

#[tokio::test]
async fn redirect_loop_hits_the_cap() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/loop"))
        .respond_with(ResponseTemplate::new(302).insert_header("location", "/loop"))
        .mount(&server)
        .await;
    let err = client().get(url(&server, "/loop")).await.unwrap_err();
    assert!(matches!(err, HttpError::TooManyRedirects { .. }), "{err}");
}

#[tokio::test]
async fn redirect_to_private_address_is_refused() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/bounce"))
        .respond_with(
            ResponseTemplate::new(302)
                .insert_header("location", "http://169.254.169.254/latest/meta-data/"),
        )
        .mount(&server)
        .await;
    let err = client().get(url(&server, "/bounce")).await.unwrap_err();
    assert!(
        matches!(&err, HttpError::Blocked { url, .. } if url.host_str() == Some("169.254.169.254")),
        "{err}"
    );
}

#[tokio::test]
async fn declared_and_streamed_oversize_bodies_are_refused() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/big"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(vec![b'x'; 1024]))
        .mount(&server)
        .await;
    let err = client()
        .get_limited(url(&server, "/big"), 512)
        .await
        .unwrap_err();
    assert!(
        matches!(err, HttpError::TooLarge { limit: 512, .. }),
        "{err}"
    );

    let ok = client()
        .get_limited(url(&server, "/big"), 1024)
        .await
        .unwrap();
    assert_eq!(ok.body.len(), 1024);
}

#[tokio::test]
async fn slow_server_times_out() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_secs(3)))
        .mount(&server)
        .await;
    let policy = Policy {
        timeout: Duration::from_millis(300),
        ..Policy::for_tests()
    };
    let err = client_with(policy, StaticHosts::new())
        .get(url(&server, "/slow"))
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Timeout { .. }), "{err}");
}

#[tokio::test]
async fn host_resolving_to_private_address_is_blocked_at_connect_time() {
    let hosts = StaticHosts::new().with("private.test", "10.0.0.1:80".parse().unwrap());
    let err = client_with(Policy::for_tests(), hosts)
        .get(Url::parse("http://private.test/").unwrap())
        .await
        .unwrap_err();
    assert!(
        matches!(&err, HttpError::Blocked { reason, .. } if reason.contains("non-public")),
        "{err}"
    );
}

#[tokio::test]
async fn host_with_a_public_and_a_private_address_uses_only_the_public_one() {
    // The "public" address is the mock server here, admitted by the test
    // policy's loopback allowance; the private one must be filtered out
    // rather than failing the whole lookup.
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string("via mock"))
        .mount(&server)
        .await;
    let hosts = StaticHosts::new()
        .with("mixed.test", "10.0.0.1:80".parse().unwrap())
        .with("mixed.test", *server.address());
    let response = client_with(Policy::for_tests(), hosts)
        .get(Url::parse("http://mixed.test/").unwrap())
        .await
        .unwrap();
    assert_eq!(response.text(), "via mock");
}

#[tokio::test]
async fn unresolvable_host_is_a_transport_error() {
    let err = client()
        .get(Url::parse("http://nowhere.test/").unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, HttpError::Transport { .. }), "{err}");
}

#[tokio::test]
async fn production_policy_refuses_http_loopback_and_credentials() {
    let c = client_with(Policy::production(), StaticHosts::new());
    for bad in [
        "http://example.com/",
        "https://127.0.0.1/",
        "https://localhost/",
        "https://foo.localhost/",
        "https://[::1]/",
        "https://user:pw@example.com/",
        "https://10.0.0.1/",
        "ftp://example.com/",
    ] {
        let err = c.get(Url::parse(bad).unwrap()).await.unwrap_err();
        assert!(matches!(err, HttpError::Blocked { .. }), "{bad}: {err}");
    }
}

#[tokio::test]
async fn per_host_concurrency_is_limited() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_delay(Duration::from_millis(150)))
        .mount(&server)
        .await;
    let policy = Policy {
        max_per_host: 1,
        ..Policy::for_tests()
    };
    let c = client_with(policy, StaticHosts::new());
    let started = Instant::now();
    let (a, b, d) = tokio::join!(
        c.get(url(&server, "/a")),
        c.get(url(&server, "/b")),
        c.get(url(&server, "/c"))
    );
    assert!(a.is_ok() && b.is_ok() && d.is_ok());
    assert!(
        started.elapsed() >= Duration::from_millis(450),
        "{:?}",
        started.elapsed()
    );
}

#[test]
fn static_hosts_pin_ports() {
    let addr: SocketAddr = "127.0.0.1:4321".parse().unwrap();
    let hosts = StaticHosts::new().with("a.test", addr);
    let rt = tokio::runtime::Runtime::new().unwrap();
    let found = rt.block_on(async {
        eaten_at_atproto::http::HostResolver::lookup(&hosts, "a.test")
            .await
            .unwrap()
    });
    assert_eq!(found, vec![addr]);
}

#[tokio::test]
async fn host_overrides_parse_and_fall_back() {
    use eaten_at_atproto::http::HostResolver;
    let hosts =
        StaticHosts::parse_overrides("alice.test=127.0.0.1:2583,eaten.test = 127.0.0.1:2583")
            .unwrap();
    assert_eq!(
        hosts.lookup("alice.test").await.unwrap(),
        vec!["127.0.0.1:2583".parse::<SocketAddr>().unwrap()]
    );
    assert!(hosts.lookup("other.test").await.is_err());
    assert!(StaticHosts::parse_overrides("alice.test=nope").is_err());
    assert!(StaticHosts::parse_overrides("alice.test").is_err());
    let layered = hosts.or_else(Arc::new(
        StaticHosts::new().with("other.test", "127.0.0.1:1".parse().unwrap()),
    ));
    assert_eq!(
        layered.lookup("other.test").await.unwrap(),
        vec!["127.0.0.1:1".parse::<SocketAddr>().unwrap()]
    );
}
