//! DNS lookups behind a trait so tests can substitute fixed answers.

use std::future::Future;
use std::pin::Pin;

use hickory_resolver::proto::rr::RData;
use hickory_resolver::TokioResolver;

/// A boxed future returned by [`DnsResolver`] methods.
pub type DnsFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Failure to perform a DNS query. "No such record" is not an error; it is
/// an empty answer.
#[derive(Debug, Clone, thiserror::Error)]
#[error("DNS lookup failed: {0}")]
pub struct DnsError(pub String);

/// The one DNS operation identity resolution needs.
pub trait DnsResolver: Send + Sync + std::fmt::Debug {
    /// Return every TXT record at `name`, each with its character-strings
    /// concatenated. An absent name yields an empty vector.
    fn lookup_txt<'a>(&'a self, name: &'a str) -> DnsFuture<'a, Result<Vec<String>, DnsError>>;
}

/// [`DnsResolver`] backed by hickory using the system's resolver configuration.
#[derive(Debug, Clone)]
pub struct SystemDns {
    inner: TokioResolver,
}

impl SystemDns {
    /// Build a resolver from `/etc/resolv.conf` (or the platform equivalent).
    pub fn from_system_conf() -> Result<Self, DnsError> {
        let inner = TokioResolver::builder_tokio()
            .map_err(|e| DnsError(e.to_string()))?
            .build()
            .map_err(|e| DnsError(e.to_string()))?;
        Ok(Self { inner })
    }
}

impl DnsResolver for SystemDns {
    fn lookup_txt<'a>(&'a self, name: &'a str) -> DnsFuture<'a, Result<Vec<String>, DnsError>> {
        Box::pin(async move {
            // A trailing dot marks the name as fully qualified so hickory
            // does not append search domains from the local configuration.
            let fqdn = format!("{name}.");
            let lookup = match self.inner.txt_lookup(fqdn).await {
                Ok(lookup) => lookup,
                Err(e) if e.is_no_records_found() || e.is_nx_domain() => return Ok(Vec::new()),
                Err(e) => return Err(DnsError(e.to_string())),
            };
            let values = lookup
                .answers()
                .iter()
                .filter_map(|record| match &record.data {
                    RData::TXT(txt) => Some(
                        txt.txt_data
                            .iter()
                            .map(|chunk| String::from_utf8_lossy(chunk))
                            .collect::<String>(),
                    ),
                    _ => None,
                })
                .collect();
            Ok(values)
        })
    }
}

/// A resolver that answers from a fixed table, optionally falling back to
/// another resolver for names it does not know. Used by tests and by the
/// development mode, where a few names stand in for DNS records that do
/// not exist yet.
#[derive(Debug, Clone, Default)]
pub struct StaticDns {
    records: std::collections::HashMap<String, Vec<String>>,
    fail: bool,
    fallback: Option<std::sync::Arc<dyn DnsResolver>>,
}

impl StaticDns {
    /// Create an empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Add TXT values for `name`.
    #[must_use]
    pub fn with_txt(mut self, name: &str, values: &[&str]) -> Self {
        self.records
            .entry(name.to_owned())
            .or_default()
            .extend(values.iter().map(|v| (*v).to_owned()));
        self
    }

    /// Make every lookup fail, to exercise fallback paths.
    #[must_use]
    pub fn failing(mut self) -> Self {
        self.fail = true;
        self
    }

    /// Consult `fallback` for names not in the table.
    #[must_use]
    pub fn or_else(mut self, fallback: std::sync::Arc<dyn DnsResolver>) -> Self {
        self.fallback = Some(fallback);
        self
    }

    /// Parse `name=value` pairs separated by commas or newlines into a
    /// table, e.g. `_lexicon.eaten.test=did=did:plc:abc`. The first `=`
    /// splits name from value, so values may themselves contain `=`.
    pub fn parse_overrides(spec: &str) -> Result<Self, String> {
        let mut dns = Self::new();
        for entry in spec
            .split([',', '\n'])
            .map(str::trim)
            .filter(|e| !e.is_empty())
        {
            let (name, value) = entry
                .split_once('=')
                .ok_or_else(|| format!("DNS override {entry:?} is not name=value"))?;
            dns = dns.with_txt(name.trim(), &[value.trim()]);
        }
        Ok(dns)
    }
}

impl DnsResolver for StaticDns {
    fn lookup_txt<'a>(&'a self, name: &'a str) -> DnsFuture<'a, Result<Vec<String>, DnsError>> {
        Box::pin(async move {
            if self.fail {
                return Err(DnsError("simulated failure".into()));
            }
            match (self.records.get(name), &self.fallback) {
                (Some(values), _) => Ok(values.clone()),
                (None, Some(fallback)) => fallback.lookup_txt(name).await,
                (None, None) => Ok(Vec::new()),
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn overrides_parse_and_fall_back() {
        let dns = StaticDns::parse_overrides(
            "_lexicon.eaten.test=did=did:plc:abc, _atproto.alice.test = did=did:plc:def\n",
        )
        .unwrap();
        assert_eq!(
            dns.lookup_txt("_lexicon.eaten.test").await.unwrap(),
            vec!["did=did:plc:abc"]
        );
        assert_eq!(
            dns.lookup_txt("_atproto.alice.test").await.unwrap(),
            vec!["did=did:plc:def"]
        );
        assert!(dns.lookup_txt("other.test").await.unwrap().is_empty());
        assert!(StaticDns::parse_overrides("no-equals").is_err());

        let fallback = StaticDns::new().with_txt("other.test", &["from-fallback"]);
        let layered = dns.or_else(std::sync::Arc::new(fallback));
        assert_eq!(
            layered.lookup_txt("other.test").await.unwrap(),
            vec!["from-fallback"]
        );
        assert_eq!(
            layered.lookup_txt("_lexicon.eaten.test").await.unwrap(),
            vec!["did=did:plc:abc"]
        );
    }
}
