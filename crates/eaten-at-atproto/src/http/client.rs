//! The guarded client itself.

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use bytes::Bytes;
use reqwest::header::{HeaderMap, HeaderValue, CONTENT_LENGTH, LOCATION};
use reqwest::StatusCode;
use tokio::sync::Semaphore;
use url::{Host, Url};

use super::hosts::{GuardedResolver, HostResolver};
use super::policy::Policy;

/// Errors from the guarded client. HTTP error statuses are *not* errors
/// here; the caller sees the status and decides.
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("refusing to fetch {url}: {reason}")]
    Blocked { url: Url, reason: String },
    #[error("too many redirects fetching {url}")]
    TooManyRedirects { url: Url },
    #[error("response from {url} exceeds {limit} bytes")]
    TooLarge { url: Url, limit: usize },
    #[error("request to {url} timed out")]
    Timeout { url: Url },
    #[error("request to {url} failed: {source}")]
    Transport {
        url: Url,
        #[source]
        source: reqwest::Error,
    },
    #[error("could not build HTTP client: {0}")]
    Build(#[source] reqwest::Error),
}

/// A request assembled by a caller that speaks its own protocol (OAuth
/// form posts carrying `DPoP` proofs). The policy applies exactly as it does
/// to every other request; only the wire format is the caller's.
#[derive(Debug, Clone)]
pub struct RawRequest {
    pub method: reqwest::Method,
    pub url: Url,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

/// What to send.
enum Method {
    Get,
    PostJson {
        body: Vec<u8>,
        bearer: Option<String>,
    },
    Raw {
        method: reqwest::Method,
        headers: HeaderMap,
        body: Vec<u8>,
    },
}

impl Method {
    /// Only idempotent reads follow redirects. A write that redirects is a
    /// misconfiguration, not something to retry elsewhere with the same
    /// credentials.
    fn follows_redirects(&self) -> bool {
        match self {
            Self::Get => true,
            Self::PostJson { .. } => false,
            Self::Raw { method, .. } => *method == reqwest::Method::GET,
        }
    }
}

/// A fully read response.
#[derive(Debug, Clone)]
pub struct Response {
    pub status: StatusCode,
    pub headers: HeaderMap,
    /// The URL that produced the body, after any redirects.
    pub url: Url,
    pub body: Bytes,
}

impl Response {
    /// Body as text, lossily decoded.
    pub fn text(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.body)
    }

    /// Body as JSON.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::from_slice(&self.body)
    }

    /// The `Content-Type` header, if present and ASCII.
    pub fn content_type(&self) -> Option<&str> {
        self.headers
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
    }
}

/// HTTP client that enforces [`Policy`] on every request and redirect hop.
/// Cheap to clone; clones share connection pools and concurrency limits.
#[derive(Debug, Clone)]
pub struct GuardedClient {
    inner: reqwest::Client,
    policy: Arc<Policy>,
    limits: Arc<Mutex<HashMap<String, Arc<Semaphore>>>>,
}

impl GuardedClient {
    /// Build a client. `user_agent` is sent on every request.
    pub fn new(
        policy: Policy,
        hosts: Arc<dyn HostResolver>,
        user_agent: &str,
    ) -> Result<Self, HttpError> {
        let policy = Arc::new(policy);
        let inner = reqwest::Client::builder()
            .user_agent(user_agent)
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(policy.connect_timeout)
            .timeout(policy.timeout)
            .dns_resolver(Arc::new(GuardedResolver::new(hosts, Arc::clone(&policy))))
            .build()
            .map_err(HttpError::Build)?;
        Ok(Self {
            inner,
            policy,
            limits: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    /// The URL scheme this client can use for URLs it builds itself:
    /// `https`, or `http` when the policy allows it (tests).
    pub fn scheme(&self) -> &'static str {
        if self.policy.allow_http {
            "http"
        } else {
            "https"
        }
    }

    /// `GET url`, following redirects, with the policy's default body cap.
    pub async fn get(&self, url: Url) -> Result<Response, HttpError> {
        self.get_limited(url, self.policy.max_body_bytes).await
    }

    /// `GET url` with an explicit body cap (clamped to the policy's).
    pub async fn get_limited(&self, url: Url, max_bytes: usize) -> Result<Response, HttpError> {
        self.send(Method::Get, url, max_bytes).await
    }

    /// `POST url` with a JSON body and, optionally, a bearer token. Redirects
    /// are not followed: a POST that redirects is a misconfiguration, not
    /// something to retry elsewhere with the same credentials.
    pub async fn post_json<T: serde::Serialize + ?Sized>(
        &self,
        url: Url,
        body: &T,
        bearer: Option<&str>,
    ) -> Result<Response, HttpError> {
        let body = serde_json::to_vec(body).map_err(|e| HttpError::Blocked {
            url: url.clone(),
            reason: format!("request body is not serializable: {e}"),
        })?;
        self.send(
            Method::PostJson {
                body,
                bearer: bearer.map(str::to_owned),
            },
            url,
            self.policy.max_body_bytes,
        )
        .await
    }

    /// Send a request the caller built, with its own method, headers, and
    /// body. Headers that describe the connection rather than the request
    /// (`Host`, `Content-Length`, `Transfer-Encoding`) are dropped; the
    /// client sets those itself.
    pub async fn send_raw(&self, request: RawRequest) -> Result<Response, HttpError> {
        let mut headers = request.headers;
        for name in [
            reqwest::header::HOST,
            CONTENT_LENGTH,
            reqwest::header::TRANSFER_ENCODING,
        ] {
            headers.remove(name);
        }
        self.send(
            Method::Raw {
                method: request.method,
                headers,
                body: request.body,
            },
            request.url,
            self.policy.max_body_bytes,
        )
        .await
    }

    async fn send(
        &self,
        method: Method,
        url: Url,
        max_bytes: usize,
    ) -> Result<Response, HttpError> {
        let max_bytes = max_bytes.min(self.policy.max_body_bytes);
        let mut url = url;
        let mut hops = 0u8;
        loop {
            self.check_url(&url)?;
            let _permit = self.permit_for(&url).await;
            let request = match &method {
                Method::Get => self.inner.get(url.clone()),
                Method::PostJson { body, bearer } => {
                    let mut req = self
                        .inner
                        .post(url.clone())
                        .header(reqwest::header::CONTENT_TYPE, "application/json")
                        .body(body.clone());
                    if let Some(token) = bearer {
                        req = req.bearer_auth(token);
                    }
                    req
                }
                Method::Raw {
                    method,
                    headers,
                    body,
                } => self
                    .inner
                    .request(method.clone(), url.clone())
                    .headers(headers.clone())
                    .body(body.clone()),
            };
            let response = request
                .send()
                .await
                .map_err(|source| map_error(&url, source))?;

            if !method.follows_redirects()
                && redirect_target(&url, response.status(), response.headers()).is_some()
            {
                return Err(HttpError::Blocked {
                    url,
                    reason: "a write was redirected".to_owned(),
                });
            }

            if let Some(next) = redirect_target(&url, response.status(), response.headers()) {
                hops += 1;
                if hops > self.policy.max_redirects {
                    return Err(HttpError::TooManyRedirects { url });
                }
                url = next;
                continue;
            }

            let status = response.status();
            let headers = response.headers().clone();
            let body = read_body(&url, response, max_bytes).await?;
            return Ok(Response {
                status,
                headers,
                url,
                body,
            });
        }
    }

    /// Reject URLs the policy forbids before any connection is attempted.
    fn check_url(&self, url: &Url) -> Result<(), HttpError> {
        let blocked = |reason: &str| HttpError::Blocked {
            url: url.clone(),
            reason: reason.to_owned(),
        };
        match url.scheme() {
            "https" => {}
            "http" if self.policy.allow_http => {}
            other => return Err(blocked(&format!("scheme {other} is not allowed"))),
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(blocked("credentials in URL"));
        }
        match url.host() {
            None => return Err(blocked("no host")),
            Some(Host::Ipv4(ip)) => self.check_ip(url, IpAddr::V4(ip))?,
            Some(Host::Ipv6(ip)) => self.check_ip(url, IpAddr::V6(ip))?,
            Some(Host::Domain(name)) => {
                // Names that resolve to loopback by convention, without DNS.
                let is_localhost =
                    name.eq_ignore_ascii_case("localhost") || name.ends_with(".localhost");
                if is_localhost && !self.policy.allow_loopback {
                    return Err(blocked("localhost is not allowed"));
                }
            }
        }
        Ok(())
    }

    fn check_ip(&self, url: &Url, ip: IpAddr) -> Result<(), HttpError> {
        if self.policy.allows_ip(ip) {
            Ok(())
        } else {
            Err(HttpError::Blocked {
                url: url.clone(),
                reason: format!("address {ip} is not public"),
            })
        }
    }

    async fn permit_for(&self, url: &Url) -> tokio::sync::OwnedSemaphorePermit {
        let host = url.host_str().unwrap_or_default().to_owned();
        let semaphore = {
            let mut limits = self.limits.lock().expect("limits mutex poisoned");
            Arc::clone(
                limits
                    .entry(host)
                    .or_insert_with(|| Arc::new(Semaphore::new(self.policy.max_per_host))),
            )
        };
        semaphore
            .acquire_owned()
            .await
            .expect("semaphore is never closed")
    }
}

/// Where a redirect response points, resolved against the request URL.
fn redirect_target(current: &Url, status: StatusCode, headers: &HeaderMap) -> Option<Url> {
    if !matches!(
        status,
        StatusCode::MOVED_PERMANENTLY
            | StatusCode::FOUND
            | StatusCode::SEE_OTHER
            | StatusCode::TEMPORARY_REDIRECT
            | StatusCode::PERMANENT_REDIRECT
    ) {
        return None;
    }
    let location = headers
        .get(LOCATION)
        .and_then(|v: &HeaderValue| v.to_str().ok())?;
    let mut next = current.join(location).ok()?;
    // A redirect never carries the previous fragment; nothing here uses
    // fragments, so drop it rather than reason about it.
    next.set_fragment(None);
    Some(next)
}

/// Read a body, refusing early on a declared length over the cap and
/// stopping as soon as the cap is exceeded while streaming.
async fn read_body(
    url: &Url,
    mut response: reqwest::Response,
    max_bytes: usize,
) -> Result<Bytes, HttpError> {
    let too_large = || HttpError::TooLarge {
        url: url.clone(),
        limit: max_bytes,
    };
    if let Some(declared) = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
    {
        if declared > max_bytes {
            return Err(too_large());
        }
    }
    let mut buf = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|source| map_error(url, source))?
    {
        if buf.len() + chunk.len() > max_bytes {
            return Err(too_large());
        }
        buf.extend_from_slice(&chunk);
    }
    Ok(Bytes::from(buf))
}

fn map_error(url: &Url, source: reqwest::Error) -> HttpError {
    if source.is_timeout() {
        return HttpError::Timeout { url: url.clone() };
    }
    // A blocked host surfaces from the resolver as a connect error; report
    // it as a policy refusal rather than a transport failure.
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(&source);
    while let Some(err) = cause {
        if let Some(blocked) = err.downcast_ref::<super::hosts::BlockedHost>() {
            return HttpError::Blocked {
                url: url.clone(),
                reason: blocked.0.clone(),
            };
        }
        cause = err.source();
    }
    HttpError::Transport {
        url: url.clone(),
        source,
    }
}
