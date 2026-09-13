//! Shared application state handed to every handler.

use std::path::Path;
use std::sync::Arc;

use eaten_at_atproto::http::{GuardedClient, Policy, SystemHosts};
use eaten_at_atproto::identity::{
    DnsResolver, Identity, IdentityConfig, IdentityResolver, SystemDns,
};
use eaten_at_atproto::oauth::{ClientConfig, OAuthClient, SigningKey};
use eaten_at_atproto::repo::RepoClient;
use eaten_at_web::APP_NAME;

use crate::auth::{self, SessionCookie, SqliteOAuthStore, WebSessions};
use crate::bsky::BskyConfig;
use crate::cache::{Cache, SystemClock};
use crate::hosting::Claims;
use crate::places::PlacesConfig;

/// `User-Agent` sent to every upstream.
pub const USER_AGENT: &str = concat!(
    "eaten-at/",
    env!("CARGO_PKG_VERSION"),
    " (+https://eaten.at)"
);

/// Cloneable handle to everything a request handler needs.
#[derive(Debug, Clone)]
pub struct AppState {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    identity: IdentityResolver,
    http: GuardedClient,
    cache: Cache,
    bsky: BskyConfig,
    public_url: url::Url,
    oauth: OAuthClient,
    oauth_store: SqliteOAuthStore,
    sessions: WebSessions,
    cookie: SessionCookie,
    claims: Claims,
    places: PlacesConfig,
}

/// Where upstreams live and where we are. Defaults are production;
/// tests point upstreams at mock servers.
#[derive(Debug, Clone)]
pub struct AppConfig {
    pub identity: IdentityConfig,
    /// The Bluesky `AppView` comment threads are read from.
    pub bsky: BskyConfig,
    /// Our own origin, for absolute URLs in meta tags and feeds.
    /// No trailing slash.
    pub public_url: url::Url,
    /// The OAuth client's key. With one the app is a confidential client;
    /// without one it is public, which works but earns shorter sessions.
    pub oauth_signing_key: Option<SigningKey>,
    /// Where place search goes, and the key that enables it.
    pub places: PlacesConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            identity: IdentityConfig::default(),
            bsky: BskyConfig::default(),
            public_url: url::Url::parse("https://eaten.at").expect("constant"),
            oauth_signing_key: None,
            places: PlacesConfig::default(),
        }
    }
}

impl AppState {
    /// Assemble state from its parts. Tests use this with mock backends.
    /// Fails only if the public URL cannot describe an OAuth client.
    pub fn new(
        http: GuardedClient,
        dns: Arc<dyn DnsResolver>,
        config: AppConfig,
        cache: Cache,
    ) -> anyhow::Result<Self> {
        let identity = IdentityResolver::new(http.clone(), dns, config.identity);
        let db = cache.database().clone();
        let clock = Arc::clone(cache.clock());
        let oauth_store = SqliteOAuthStore::new(db.clone(), Arc::clone(&clock));
        let oauth = OAuthClient::new(
            ClientConfig {
                public_url: config.public_url.clone(),
                client_name: APP_NAME.to_owned(),
                scopes: auth::all_scopes(),
                signing_key: config.oauth_signing_key,
            },
            http.clone(),
            Arc::new(identity.clone()),
            Arc::new(oauth_store.clone()),
        )?;
        Ok(Self {
            inner: Arc::new(Inner {
                identity,
                http,
                cache,
                bsky: config.bsky,
                cookie: SessionCookie::for_origin(&config.public_url),
                public_url: config.public_url,
                oauth,
                oauth_store,
                claims: Claims::new(db.clone(), Arc::clone(&clock)),
                sessions: WebSessions::new(db, clock),
                places: config.places,
            }),
        })
    }

    /// Absolute URL on our own origin for a site-route path.
    pub fn absolute(&self, path: &str) -> String {
        format!(
            "{}{path}",
            self.inner.public_url.as_str().trim_end_matches('/')
        )
    }

    /// `host[:port]` of our own origin, as a `Host` header would carry it.
    pub fn public_host(&self) -> String {
        let url = &self.inner.public_url;
        match (url.host_str(), url.port()) {
            (Some(host), Some(port)) => format!("{host}:{port}"),
            (Some(host), None) => host.to_owned(),
            (None, _) => String::new(),
        }
    }

    /// Production state: system DNS, public PLC directory, production
    /// HTTP policy (HTTPS only, public addresses only).
    pub fn from_env(db_path: &Path, public_url: url::Url) -> anyhow::Result<Self> {
        let http = GuardedClient::new(Policy::production(), Arc::new(SystemHosts), USER_AGENT)?;
        let dns = Arc::new(SystemDns::from_system_conf()?);
        let cache = Cache::open(db_path, Arc::new(SystemClock))?;
        let config = AppConfig {
            public_url,
            ..AppConfig::default()
        };
        Self::new(http, dns, config, cache)
    }

    pub fn cache(&self) -> &Cache {
        &self.inner.cache
    }

    pub fn http(&self) -> &GuardedClient {
        &self.inner.http
    }

    pub(crate) fn bsky(&self) -> &BskyConfig {
        &self.inner.bsky
    }

    pub fn identity(&self) -> &IdentityResolver {
        &self.inner.identity
    }

    /// The OAuth client for signing in and acting as a signed-in user.
    pub fn oauth(&self) -> &OAuthClient {
        &self.inner.oauth
    }

    /// The OAuth client's storage, for housekeeping.
    pub fn oauth_store(&self) -> &SqliteOAuthStore {
        &self.inner.oauth_store
    }

    /// Browser sessions.
    pub fn sessions(&self) -> &WebSessions {
        &self.inner.sessions
    }

    /// How the session cookie is set for this origin.
    pub fn cookie(&self) -> &SessionCookie {
        &self.inner.cookie
    }

    /// Hosted subdomain claims.
    pub fn claims(&self) -> &Claims {
        &self.inner.claims
    }

    /// Place search configuration.
    pub(crate) fn places(&self) -> &PlacesConfig {
        &self.inner.places
    }

    /// A repo client for the PDS of a resolved identity.
    pub fn repo_for(&self, identity: &Identity) -> RepoClient {
        RepoClient::new(self.inner.http.clone(), identity.pds.clone())
    }
}
