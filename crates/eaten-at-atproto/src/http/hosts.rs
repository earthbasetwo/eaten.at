//! Hostname → address lookup, filtered by policy at connection time.
//!
//! Filtering here rather than before the request closes the DNS-rebinding
//! window: the addresses the connector actually dials are the ones checked.

use std::future::Future;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::Arc;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use super::policy::Policy;

/// Boxed future returned by [`HostResolver::lookup`].
pub type LookupFuture<'a> =
    Pin<Box<dyn Future<Output = std::io::Result<Vec<SocketAddr>>> + Send + 'a>>;

/// Resolves a hostname to socket addresses. Port is `0` unless the resolver
/// has a reason to pin one (tests pointing a name at a mock server do).
pub trait HostResolver: Send + Sync + std::fmt::Debug {
    fn lookup<'a>(&'a self, host: &'a str) -> LookupFuture<'a>;
}

/// [`HostResolver`] using the operating system's resolver.
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemHosts;

impl HostResolver for SystemHosts {
    fn lookup<'a>(&'a self, host: &'a str) -> LookupFuture<'a> {
        Box::pin(async move {
            tokio::net::lookup_host((host, 0))
                .await
                .map(Iterator::collect)
        })
    }
}

/// [`HostResolver`] answering from a fixed table, optionally falling back
/// to another resolver. Without a fallback, unknown names fail to resolve.
#[derive(Debug, Clone, Default)]
pub struct StaticHosts {
    table: std::collections::HashMap<String, Vec<SocketAddr>>,
    fallback: Option<Arc<dyn HostResolver>>,
}

impl StaticHosts {
    pub fn new() -> Self {
        Self::default()
    }

    /// Point `host` at `addr`, port included.
    #[must_use]
    pub fn with(mut self, host: &str, addr: SocketAddr) -> Self {
        self.table.entry(host.to_owned()).or_default().push(addr);
        self
    }

    /// Consult `fallback` for hosts not in the table.
    #[must_use]
    pub fn or_else(mut self, fallback: Arc<dyn HostResolver>) -> Self {
        self.fallback = Some(fallback);
        self
    }

    /// Parse `host=ip:port` pairs separated by commas or newlines.
    pub fn parse_overrides(spec: &str) -> Result<Self, String> {
        let mut hosts = Self::new();
        for entry in spec
            .split([',', '\n'])
            .map(str::trim)
            .filter(|e| !e.is_empty())
        {
            let (host, addr) = entry
                .split_once('=')
                .ok_or_else(|| format!("host override {entry:?} is not host=ip:port"))?;
            let addr: SocketAddr = addr
                .trim()
                .parse()
                .map_err(|e| format!("host override {entry:?}: {e}"))?;
            hosts = hosts.with(host.trim(), addr);
        }
        Ok(hosts)
    }
}

impl HostResolver for StaticHosts {
    fn lookup<'a>(&'a self, host: &'a str) -> LookupFuture<'a> {
        Box::pin(async move {
            match (self.table.get(host), &self.fallback) {
                (Some(addrs), _) => Ok(addrs.clone()),
                (None, Some(fallback)) => fallback.lookup(host).await,
                (None, None) => Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    format!("no static entry for {host}"),
                )),
            }
        })
    }
}

/// The resolver handed to reqwest: looks up via the inner resolver, then
/// drops every address the policy forbids.
#[derive(Debug, Clone)]
pub(super) struct GuardedResolver {
    inner: Arc<dyn HostResolver>,
    policy: Arc<Policy>,
}

impl GuardedResolver {
    pub(super) fn new(inner: Arc<dyn HostResolver>, policy: Arc<Policy>) -> Self {
        Self { inner, policy }
    }
}

impl Resolve for GuardedResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let inner = Arc::clone(&self.inner);
        let policy = Arc::clone(&self.policy);
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let addrs = inner.lookup(&host).await?;
            let total = addrs.len();
            let allowed: Vec<SocketAddr> = addrs
                .into_iter()
                .filter(|addr| policy.allows_ip(addr.ip()))
                .collect();
            if allowed.is_empty() {
                let reason = if total == 0 {
                    format!("{host} has no addresses")
                } else {
                    format!("{host} resolves only to non-public addresses")
                };
                return Err(
                    Box::new(BlockedHost(reason)) as Box<dyn std::error::Error + Send + Sync>
                );
            }
            Ok(Box::new(allowed.into_iter()) as Addrs)
        })
    }
}

/// Error surfaced through reqwest when a host has no permitted addresses.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct BlockedHost(pub(super) String);
