//! Hosted subdomains (plan §5.5, D8): a publication may claim
//! `{name}.eaten.at`, and requests arriving under that host are
//! served as the publication's own origin. Claims live in SQLite, keyed
//! by publication; the claim rewrites `publication.url`, which is what
//! makes the canonical links and the `.well-known` verification agree.
//!
//! Routing is a URI rewrite: a request for `/` on a claimed host becomes
//! the publication's site route, a request for a document's path becomes
//! its site route, and so on. The handlers stay the same.

use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header::HOST;
use axum::http::{StatusCode, Uri};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use eaten_at_atproto::at_uri::AtUri;
use eaten_at_atproto::identity::{Did, Handle, Identity};
use rusqlite::{params, OptionalExtension};

use crate::cache::Clock;
use crate::db::{Database, DbError};
use crate::error::AppError;
use crate::paths;
use crate::state::AppState;

/// Names that can never be claimed: they are, or may become, the site's own.
pub const RESERVED_NAMES: &[&str] = &[
    "www", "api", "static", "img", "cdn", "assets", "mail", "smtp", "imap", "admin", "app",
    "login", "oauth", "auth", "feed", "feeds", "rss", "m", "dev", "staging", "status", "help",
    "docs", "blog", "about", "settings", "write", "at", "did", "plc", "eaten", "eaten-at", "ns",
    "ns1", "ns2", "mx", "ftp", "git", "root",
];
/// Longest DNS label.
const MAX_LABEL_LEN: usize = 63;
/// How many pages of a repo's documents are read to find one by path.
const MAX_PATH_PAGES: usize = 25;

/// A claimed subdomain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Claim {
    pub name: String,
    pub publication_uri: AtUri,
    pub did: Did,
}

/// Why a name cannot be claimed.
#[derive(Debug, thiserror::Error)]
pub enum ClaimError {
    #[error(
        "use 1 to 63 lowercase letters, digits, or hyphens, not starting or ending with a hyphen"
    )]
    Invalid,
    #[error("that name is reserved")]
    Reserved,
    #[error("that name is taken")]
    Taken,
    #[error(transparent)]
    Db(#[from] DbError),
}

impl PartialEq for ClaimError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::Invalid, Self::Invalid)
                | (Self::Reserved, Self::Reserved)
                | (Self::Taken, Self::Taken)
        )
    }
}

/// Most numbered variants tried when a suggested label is taken.
const MAX_SUFFIX: u32 = 99;

/// The subdomain label suggested for an account (plan 08): the first
/// label of its handle (`alice` from `alice.bsky.social`, `rosslebeau`
/// from `rosslebeau.com`), or the DID's own id when it has no verified
/// handle, made into a valid label. Reserved or taken names are the
/// caller's problem; see [`AppState::claim_free`].
pub fn suggest_name(did: &Did, handle: Option<&Handle>) -> String {
    let raw = match handle {
        Some(handle) => handle
            .as_str()
            .split('.')
            .next()
            .unwrap_or_default()
            .to_owned(),
        None => did.as_str().trim_start_matches("did:").replace(':', "-"),
    };
    let mut name: String = raw
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    name.truncate(MAX_LABEL_LEN);
    let name = name.trim_matches('-').to_owned();
    if name.is_empty() {
        "author".to_owned()
    } else {
        name
    }
}

/// A subdomain label as the user typed it, normalised and checked.
pub fn validate_name(raw: &str) -> Result<String, ClaimError> {
    let name = raw.trim().to_ascii_lowercase();
    let ok_chars = name
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if name.is_empty()
        || name.len() > MAX_LABEL_LEN
        || !ok_chars
        || name.starts_with('-')
        || name.ends_with('-')
    {
        return Err(ClaimError::Invalid);
    }
    if RESERVED_NAMES.contains(&name.as_str()) {
        return Err(ClaimError::Reserved);
    }
    Ok(name)
}

/// The claims table.
#[derive(Debug, Clone)]
pub struct Claims {
    db: Database,
    clock: Arc<dyn Clock>,
}

impl Claims {
    pub fn new(db: Database, clock: Arc<dyn Clock>) -> Self {
        Self { db, clock }
    }

    /// Claim `name` for a publication, replacing any claim that
    /// publication held. Fails with [`ClaimError::Taken`] when another
    /// publication holds the name.
    pub async fn claim(
        &self,
        name: &str,
        publication: &AtUri,
        did: &Did,
    ) -> Result<(), ClaimError> {
        let name = validate_name(name)?;
        let uri = publication.as_str().to_owned();
        let did = did.as_str().to_owned();
        let now = self.clock.now();
        let outcome = self
            .db
            .run(move |conn| {
                let holder: Option<String> = conn
                    .query_row(
                        "SELECT publication_uri FROM subdomain_claims WHERE name = ?1",
                        params![name],
                        |row| row.get(0),
                    )
                    .optional()?;
                if holder.is_some_and(|h| h != uri) {
                    return Ok(false);
                }
                conn.execute(
                    "DELETE FROM subdomain_claims WHERE publication_uri = ?1",
                    params![uri],
                )?;
                conn.execute(
                    "INSERT OR REPLACE INTO subdomain_claims (name, publication_uri, did, created_at)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![name, uri, did, now],
                )?;
                Ok(true)
            })
            .await?;
        if outcome {
            Ok(())
        } else {
            Err(ClaimError::Taken)
        }
    }

    /// Drop whatever claim a publication holds.
    pub async fn release(&self, publication: &AtUri) -> Result<(), DbError> {
        let uri = publication.as_str().to_owned();
        self.db
            .run(move |conn| {
                conn.execute(
                    "DELETE FROM subdomain_claims WHERE publication_uri = ?1",
                    params![uri],
                )
                .map(|_| ())
            })
            .await
    }

    /// The claim for a name, if any.
    pub async fn by_name(&self, name: &str) -> Result<Option<Claim>, DbError> {
        let name = name.to_ascii_lowercase();
        self.db
            .run(move |conn| {
                conn.query_row(
                    "SELECT name, publication_uri, did FROM subdomain_claims WHERE name = ?1",
                    params![name],
                    row_to_claim,
                )
                .optional()
            })
            .await
            .map(Option::flatten)
    }

    /// The claim a publication holds, if any.
    pub async fn for_publication(&self, publication: &AtUri) -> Result<Option<Claim>, DbError> {
        let uri = publication.as_str().to_owned();
        self.db
            .run(move |conn| {
                conn.query_row(
                    "SELECT name, publication_uri, did FROM subdomain_claims WHERE publication_uri = ?1",
                    params![uri],
                    row_to_claim,
                )
                .optional()
            })
            .await
            .map(Option::flatten)
    }

    /// Remember that `old_host` moved to `new_origin`, so links to it can
    /// be redirected for good.
    pub async fn record_move(&self, old_host: &str, new_origin: &str) -> Result<(), DbError> {
        let old_host = old_host.to_ascii_lowercase();
        let new_origin = new_origin.trim_end_matches('/').to_owned();
        let now = self.clock.now();
        self.db
            .run(move |conn| {
                conn.execute(
                    "INSERT OR REPLACE INTO origin_moves (old_host, new_origin, moved_at)
                     VALUES (?1, ?2, ?3)",
                    params![old_host, new_origin, now],
                )
                .map(|_| ())
            })
            .await
    }

    /// Where a host moved to, if it did.
    pub async fn moved_to(&self, host: &str) -> Result<Option<String>, DbError> {
        let host = host.to_ascii_lowercase();
        self.db
            .run(move |conn| {
                conn.query_row(
                    "SELECT new_origin FROM origin_moves WHERE old_host = ?1",
                    params![host],
                    |row| row.get::<_, String>(0),
                )
                .optional()
            })
            .await
    }
}

fn row_to_claim(row: &rusqlite::Row<'_>) -> rusqlite::Result<Option<Claim>> {
    let name: String = row.get(0)?;
    let uri: String = row.get(1)?;
    let did: String = row.get(2)?;
    Ok(match (AtUri::parse(&uri), Did::parse(&did)) {
        (Ok(publication_uri), Ok(did)) => Some(Claim {
            name,
            publication_uri,
            did,
        }),
        _ => None,
    })
}

impl AppState {
    /// Claim `base` for `publication`, or failing that the first of
    /// `base-2`, `base-3`, … that is neither taken nor reserved. Returns
    /// the name claimed.
    pub async fn claim_free(
        &self,
        base: &str,
        publication: &AtUri,
        did: &Did,
    ) -> Result<String, ClaimError> {
        for n in 1..=MAX_SUFFIX {
            let candidate = if n == 1 {
                base.to_owned()
            } else {
                let suffix = format!("-{n}");
                let mut stem = base.to_owned();
                stem.truncate(MAX_LABEL_LEN - suffix.len());
                format!("{}{suffix}", stem.trim_end_matches('-'))
            };
            match self.claims().claim(&candidate, publication, did).await {
                Ok(()) => return Ok(candidate),
                Err(ClaimError::Taken | ClaimError::Reserved) => {}
                Err(err) => return Err(err),
            }
        }
        Err(ClaimError::Taken)
    }

    /// The host of a hosted subdomain: `{name}.{our host}`.
    pub fn hosted_host(&self, name: &str) -> String {
        format!("{name}.{}", self.public_host())
    }

    /// The origin written into `publication.url` for a hosted subdomain.
    /// Always https: hosting needs TLS to be worth anything.
    pub fn hosted_origin(&self, name: &str) -> String {
        format!("https://{}", self.hosted_host(name))
    }

    /// Whether this deployment can host subdomains at all: only under a
    /// real domain name, not an address or `localhost`.
    pub fn can_host_subdomains(&self) -> bool {
        let host = self.public_host();
        let bare = host.split(':').next().unwrap_or_default();
        bare.contains('.')
            && bare.parse::<std::net::IpAddr>().is_err()
            && !bare.ends_with(".localhost")
            && bare != "localhost"
    }

    /// The subdomain label in `host`, when `host` is one level under our
    /// own host (ports must match).
    pub fn subdomain_of(&self, host: &str) -> Option<String> {
        let host = host.to_ascii_lowercase();
        let suffix = format!(".{}", self.public_host().to_ascii_lowercase());
        let name = host.strip_suffix(&suffix)?;
        (!name.is_empty() && !name.contains('.')).then(|| name.to_owned())
    }

    /// The record key of the document at `path` in a publication.
    pub async fn document_by_path(
        &self,
        identity: &Identity,
        publication: &AtUri,
        path: &str,
    ) -> Result<Option<String>, AppError> {
        let mut cursor: Option<String> = None;
        for _ in 0..MAX_PATH_PAGES {
            let page = self.documents(identity, cursor.as_deref()).await?;
            if let Some(found) = page.records.iter().find(|r| {
                r.value.site == publication.as_str() && r.value.path.as_deref() == Some(path)
            }) {
                return Ok(Some(found.rkey().to_owned()));
            }
            match page.cursor {
                Some(next) if !page.records.is_empty() => cursor = Some(next),
                _ => break,
            }
        }
        Ok(None)
    }
}

/// Paths that keep their meaning on every host.
fn passes_through(path: &str) -> bool {
    path == "/healthz"
        || path.starts_with("/static/")
        || path.starts_with("/img/")
        || path.starts_with("/at/")
        || path.starts_with("/@")
        || path.starts_with("/login")
        || path.starts_with("/oauth/")
        || path == "/logout"
        || path == "/client-metadata.json"
        || path.starts_with("/write")
        || path.starts_with("/settings")
        || path == "/labels/continue"
        || path == "/lookup"
}

/// Middleware: serve a claimed subdomain as its publication's origin, send
/// a moved host on, and refuse an unclaimed one.
pub async fn by_host(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .unwrap_or_default();
    let under_us = host
        .to_ascii_lowercase()
        .ends_with(&format!(".{}", state.public_host().to_ascii_lowercase()));
    if !under_us {
        return next.run(request).await;
    }
    let Some(name) = state.subdomain_of(&host) else {
        return AppError::NotFound(format!("nothing is hosted at {host}")).into_response();
    };
    let path = request.uri().path().to_owned();
    if passes_through(&path) {
        return next.run(request).await;
    }
    let claim = match state.claims().by_name(&name).await {
        Ok(claim) => claim,
        Err(err) => {
            tracing::warn!(%err, "claim lookup failed");
            return AppError::Upstream(err.to_string()).into_response();
        }
    };
    let Some(claim) = claim else {
        if let Ok(Some(origin)) = state.claims().moved_to(&host).await {
            let query = request
                .uri()
                .query()
                .map(|q| format!("?{q}"))
                .unwrap_or_default();
            return Redirect::permanent(&format!("{origin}{path}{query}")).into_response();
        }
        return AppError::NotFound(format!("nothing is hosted at {host}")).into_response();
    };
    let did = &claim.did;
    let pub_rkey = claim.publication_uri.rkey();
    let target = match path.as_str() {
        "/" => Some(paths::publication(did, pub_rkey)),
        "/feed.xml" => Some(paths::feed(did, pub_rkey)),
        "/.well-known/site.standard.publication" => Some(format!(
            "{}publication.json",
            paths::publication(did, pub_rkey)
        )),
        other => {
            if let Some(tag) = other.strip_prefix("/tagged/") {
                Some(format!("{}tagged/{tag}", paths::publication(did, pub_rkey)))
            } else {
                match state.identity_for(did).await {
                    Ok(Some(identity)) => state
                        .document_by_path(&identity, &claim.publication_uri, other)
                        .await
                        .ok()
                        .flatten()
                        .map(|rkey| paths::document(did, pub_rkey, &rkey)),
                    _ => None,
                }
            }
        }
    };
    let Some(target) = target else {
        return AppError::NotFound(format!("no page at {host}{path}")).into_response();
    };
    let rewritten = match request.uri().query() {
        Some(query) => format!("{target}?{query}"),
        None => target,
    };
    match rewritten.parse::<Uri>() {
        Ok(uri) => *request.uri_mut() = uri,
        Err(_) => return StatusCode::NOT_FOUND.into_response(),
    }
    next.run(request).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::ManualClock;

    #[test]
    fn a_label_is_suggested_from_the_handle_or_the_did() {
        let did = Did::parse("did:plc:re3ebnp5v7ffagz6rb6xfei4").unwrap();
        let handle = |s: &str| Handle::parse(s).unwrap();
        assert_eq!(
            suggest_name(&did, Some(&handle("alice.bsky.social"))),
            "alice"
        );
        assert_eq!(
            suggest_name(&did, Some(&handle("rosslebeau.com"))),
            "rosslebeau"
        );
        assert_eq!(suggest_name(&did, None), "plc-re3ebnp5v7ffagz6rb6xfei4");
        // A reserved word is suggested as is; claiming decides what to do.
        assert_eq!(suggest_name(&did, Some(&handle("www.example.com"))), "www");
        assert!(validate_name(&suggest_name(&did, None)).is_ok());
    }

    #[test]
    fn names_follow_dns_label_rules() {
        assert_eq!(validate_name(" Ross ").unwrap(), "ross");
        assert_eq!(
            validate_name("heavy-rotation-2").unwrap(),
            "heavy-rotation-2"
        );
        for bad in [
            "",
            "-ross",
            "ross-",
            "ro ss",
            "ro.ss",
            "röss",
            &"a".repeat(64),
        ] {
            assert_eq!(validate_name(bad), Err(ClaimError::Invalid), "{bad:?}");
        }
        for reserved in ["www", "API", "static", "img", "settings"] {
            assert_eq!(
                validate_name(reserved),
                Err(ClaimError::Reserved),
                "{reserved}"
            );
        }
        assert_eq!(validate_name("test").unwrap(), "test");
    }

    #[tokio::test]
    async fn claims_are_unique_and_one_per_publication() {
        let claims = Claims::new(
            Database::in_memory().unwrap(),
            Arc::new(ManualClock::starting_at(1)),
        );
        let did = Did::parse("did:plc:re3ebnp5v7ffagz6rb6xfei4").unwrap();
        let a = AtUri::parse("at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/a")
            .unwrap();
        let b = AtUri::parse("at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/b")
            .unwrap();
        claims.claim("ross", &a, &did).await.unwrap();
        assert_eq!(claims.claim("ross", &b, &did).await, Err(ClaimError::Taken));
        claims.claim("ross", &a, &did).await.unwrap();
        claims.claim("records", &a, &did).await.unwrap();
        assert!(
            claims.by_name("ross").await.unwrap().is_none(),
            "one name per publication"
        );
        let held = claims.for_publication(&a).await.unwrap().unwrap();
        assert_eq!(held.name, "records");
        assert_eq!(held.did, did);
        assert_eq!(
            claims.by_name("RECORDS").await.unwrap().map(|c| c.name),
            Some("records".into())
        );
        claims.release(&a).await.unwrap();
        assert!(claims.for_publication(&a).await.unwrap().is_none());
        claims.claim("ross", &b, &did).await.unwrap();

        claims
            .record_move("ross.eaten.at", "https://own.example/")
            .await
            .unwrap();
        assert_eq!(
            claims.moved_to("Ross.eaten.at").await.unwrap().as_deref(),
            Some("https://own.example")
        );
        assert_eq!(claims.moved_to("x.eaten.at").await.unwrap(), None);
    }

    #[test]
    fn pass_through_paths() {
        assert!(passes_through("/static/app.css"));
        assert!(passes_through("/at/did:plc:x/y/"));
        assert!(passes_through("/write/abc"));
        assert!(!passes_through("/"));
        assert!(!passes_through("/2026/09/slug"));
        assert!(!passes_through("/.well-known/site.standard.publication"));
    }
}
