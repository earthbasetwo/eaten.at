//! Process configuration from the environment, including the development
//! mode that lets the app talk to a local atproto stack.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use eaten_at_atproto::http::{GuardedClient, HostResolver, Policy, StaticHosts, SystemHosts};
use eaten_at_atproto::identity::{DnsResolver, IdentityConfig, StaticDns, SystemDns};
use eaten_at_atproto::oauth::SigningKey;
use url::Url;

use crate::bsky::BskyConfig;
use crate::cache::{Cache, SystemClock};
use crate::state::{AppConfig, AppState, USER_AGENT};

/// Names of every environment variable the app reads.
pub mod env {
    pub const LISTEN: &str = "EATEN_AT_LISTEN";
    pub const DB: &str = "EATEN_AT_DB";
    pub const PUBLIC_URL: &str = "EATEN_AT_PUBLIC_URL";
    pub const PLC_DIRECTORY: &str = "EATEN_AT_PLC_DIRECTORY";
    /// `1` enables the development policy: plain HTTP and loopback allowed.
    pub const DEV_INSECURE: &str = "EATEN_AT_DEV_INSECURE";
    /// `host=ip:port,…` overrides for name resolution.
    pub const DEV_HOSTS: &str = "EATEN_AT_DEV_HOSTS";
    /// `name=value,…` fixed TXT answers, e.g. `_lexicon.eaten.at=did=did:plc:…`.
    pub const DEV_DNS_TXT: &str = "EATEN_AT_DEV_DNS_TXT";
    /// Path of the OAuth signing key (a private JWK). Created on first
    /// start if absent. Without it the app is a public OAuth client.
    pub const OAUTH_KEY_FILE: &str = "EATEN_AT_OAUTH_KEY_FILE";
    /// The Bluesky `AppView` that comment threads are read from. Defaults
    /// to the public one.
    pub const BSKY_APPVIEW: &str = "EATEN_AT_BSKY_APPVIEW";
}

/// Everything the process needs to start.
#[derive(Debug, Clone)]
pub struct Settings {
    pub listen: SocketAddr,
    pub db: PathBuf,
    pub public_url: Url,
    pub plc_directory: Option<Url>,
    pub oauth_key_file: Option<PathBuf>,
    pub bsky_appview: Option<Url>,
    pub dev: Option<Dev>,
}

/// Development-mode overrides. Never set these in production: the
/// insecure policy disables the SSRF guards.
#[derive(Debug, Clone)]
pub struct Dev {
    pub insecure: bool,
    pub hosts: StaticHosts,
    pub dns: StaticDns,
}

impl Settings {
    /// Read the environment. Only malformed values are errors; absent ones
    /// take defaults.
    pub fn from_env() -> anyhow::Result<Self> {
        let var = |name: &str| std::env::var(name).ok().filter(|v| !v.trim().is_empty());
        let listen_raw = var(env::LISTEN).unwrap_or_else(|| "127.0.0.1:3000".to_owned());
        let listen: SocketAddr = listen_raw.parse().with_context(|| {
            format!(
                "{}={listen_raw:?} is not a valid socket address",
                env::LISTEN
            )
        })?;
        let db = PathBuf::from(var(env::DB).unwrap_or_else(|| "eaten-at.db".to_owned()));
        let public_url = match var(env::PUBLIC_URL) {
            Some(raw) => Url::parse(&raw)
                .with_context(|| format!("{}={raw:?} is not a valid URL", env::PUBLIC_URL))?,
            None => Url::parse(&format!("http://{listen}"))
                .with_context(|| format!("cannot derive a public URL from {listen}"))?,
        };
        let plc_directory =
            match var(env::PLC_DIRECTORY) {
                Some(raw) => Some(Url::parse(&raw).with_context(|| {
                    format!("{}={raw:?} is not a valid URL", env::PLC_DIRECTORY)
                })?),
                None => None,
            };

        let bsky_appview = match var(env::BSKY_APPVIEW) {
            Some(raw) => Some(
                Url::parse(&raw)
                    .with_context(|| format!("{}={raw:?} is not a valid URL", env::BSKY_APPVIEW))?,
            ),
            None => None,
        };
        let insecure =
            var(env::DEV_INSECURE).is_some_and(|v| v == "1" || v.eq_ignore_ascii_case("true"));
        let hosts = match var(env::DEV_HOSTS) {
            Some(spec) => Some(StaticHosts::parse_overrides(&spec).map_err(anyhow::Error::msg)?),
            None => None,
        };
        let dns = match var(env::DEV_DNS_TXT) {
            Some(spec) => Some(StaticDns::parse_overrides(&spec).map_err(anyhow::Error::msg)?),
            None => None,
        };
        let dev = if insecure || hosts.is_some() || dns.is_some() {
            Some(Dev {
                insecure,
                hosts: hosts.unwrap_or_default(),
                dns: dns.unwrap_or_default(),
            })
        } else {
            None
        };
        Ok(Self {
            listen,
            db,
            public_url,
            plc_directory,
            oauth_key_file: var(env::OAUTH_KEY_FILE).map(PathBuf::from),
            bsky_appview,
            dev,
        })
    }

    /// The outbound HTTP policy: production unless dev mode says otherwise.
    pub fn policy(&self) -> Policy {
        if self.dev.as_ref().is_some_and(|d| d.insecure) {
            Policy::for_tests()
        } else {
            Policy::production()
        }
    }

    /// Name resolution for the guarded client: dev overrides first, then
    /// the system resolver.
    pub fn hosts(&self) -> Arc<dyn HostResolver> {
        match &self.dev {
            Some(dev) => Arc::new(dev.hosts.clone().or_else(Arc::new(SystemHosts))),
            None => Arc::new(SystemHosts),
        }
    }

    /// TXT lookups for identity and lexicon resolution: dev overrides
    /// first, then the system resolver.
    pub fn dns(&self) -> anyhow::Result<Arc<dyn DnsResolver>> {
        let system: Arc<dyn DnsResolver> = Arc::new(SystemDns::from_system_conf()?);
        Ok(match &self.dev {
            Some(dev) => Arc::new(dev.dns.clone().or_else(system)),
            None => system,
        })
    }

    pub fn http(&self) -> anyhow::Result<GuardedClient> {
        Ok(GuardedClient::new(self.policy(), self.hosts(), USER_AGENT)?)
    }

    pub fn identity_config(&self) -> IdentityConfig {
        match &self.plc_directory {
            Some(plc) => IdentityConfig {
                plc_directory: plc.clone(),
            },
            None => IdentityConfig::default(),
        }
    }

    /// Build the application state. Logs loudly when the insecure policy
    /// is active so it cannot go unnoticed in a log.
    pub fn build_state(&self) -> anyhow::Result<AppState> {
        if self.dev.as_ref().is_some_and(|d| d.insecure) {
            tracing::warn!(
                "{} is set: plain HTTP and loopback upstreams are allowed. Never run this way in production.",
                env::DEV_INSECURE
            );
        }
        let cache = Cache::open(&self.db, Arc::new(SystemClock))
            .with_context(|| format!("could not open cache database {}", self.db.display()))?;
        let config = AppConfig {
            identity: self.identity_config(),
            public_url: self.public_url.clone(),
            oauth_signing_key: self.oauth_signing_key()?,
            bsky: match &self.bsky_appview {
                Some(appview) => BskyConfig {
                    appview: appview.clone(),
                },
                None => BskyConfig::default(),
            },
        };
        AppState::new(self.http()?, self.dns()?, config, cache)
    }

    /// The OAuth signing key from its file, created if the file does not
    /// exist yet. `None` when no file is configured.
    fn oauth_signing_key(&self) -> anyhow::Result<Option<SigningKey>> {
        let Some(path) = &self.oauth_key_file else {
            if self.dev.is_none() {
                tracing::warn!(
                    "{} is not set: the app is a public OAuth client and sessions will be shorter",
                    env::OAUTH_KEY_FILE
                );
            }
            return Ok(None);
        };
        if path.exists() {
            let json = std::fs::read_to_string(path)
                .with_context(|| format!("could not read {}", path.display()))?;
            let key = SigningKey::from_json(&json)
                .with_context(|| format!("{} is not a usable signing key", path.display()))?;
            tracing::info!(kid = key.kid(), path = %path.display(), "OAuth signing key loaded");
            return Ok(Some(key));
        }
        let key = SigningKey::generate();
        write_secret(path, &key.to_json())
            .with_context(|| format!("could not write {}", path.display()))?;
        tracing::info!(kid = key.kid(), path = %path.display(), "OAuth signing key generated");
        Ok(Some(key))
    }
}

/// Write `contents` to a new file readable by the owner only.
fn write_secret(path: &Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(contents.as_bytes())?;
    file.write_all(b"\n")
}
