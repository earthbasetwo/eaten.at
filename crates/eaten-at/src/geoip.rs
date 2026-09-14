//! Where a request comes from, by its IP (plan 12, D44): the point the
//! editor's place search looks near. A local database in the `MaxMind`
//! DB format with the `GeoIP2` City layout (DB-IP's IP-to-City Lite, or
//! any file with that layout) is read into memory at startup; nothing
//! leaves the server to find out where an author is.
//!
//! City-level at best, and weaker on phones (carrier networks pool
//! traffic; iCloud Private Relay egresses from Apple's addresses). The
//! search's 25-mile radius absorbs most of that, and a place can always
//! be entered by hand.

use std::fmt;
use std::net::IpAddr;
use std::path::Path;
use std::sync::Arc;

use anyhow::Context as _;
use axum::extract::{ConnectInfo, FromRequestParts};
use axum::http::request::Parts;
use axum::http::HeaderMap;
use maxminddb::{geoip2, Reader};

use crate::places::Point;

/// Where the request is from, as far as the database can say.
#[derive(Debug, Clone, PartialEq)]
pub struct Located {
    pub point: Point,
    /// The city's English name, when the database has one.
    pub city: Option<String>,
}

/// The IP-to-location source.
#[derive(Clone)]
pub struct GeoIp {
    source: Source,
}

#[derive(Clone)]
enum Source {
    /// No database configured: nothing locates.
    None,
    /// A database file, read into memory.
    Database(Arc<Reader<Vec<u8>>>),
    /// Every request is here. Development only.
    Fixed(Point),
}

impl fmt::Debug for GeoIp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.source {
            Source::None => f.write_str("GeoIp::None"),
            Source::Database(_) => f.write_str("GeoIp::Database"),
            Source::Fixed(point) => write!(f, "GeoIp::Fixed({point:?})"),
        }
    }
}

impl Default for GeoIp {
    fn default() -> Self {
        Self::none()
    }
}

impl GeoIp {
    /// No database: every lookup answers nothing.
    pub fn none() -> Self {
        Self {
            source: Source::None,
        }
    }

    /// Every request located at `point`. For the local network, whose
    /// addresses are loopback and would otherwise locate to nothing.
    pub fn fixed(point: Point) -> Self {
        Self {
            source: Source::Fixed(point),
        }
    }

    /// Read a database file into memory. A missing or unreadable file
    /// is an error: a configured path that does not work should stop
    /// the process, not silently turn search off.
    pub fn open(path: &Path) -> anyhow::Result<Self> {
        let reader = Reader::open_readfile(path)
            .with_context(|| format!("could not read the IP database {}", path.display()))?;
        tracing::info!(
            path = %path.display(),
            kind = %reader.metadata().database_type,
            built = reader.metadata().build_epoch,
            "IP database loaded"
        );
        Ok(Self {
            source: Source::Database(Arc::new(reader)),
        })
    }

    /// Whether lookups can answer at all.
    pub fn enabled(&self) -> bool {
        !matches!(self.source, Source::None)
    }

    /// Where `ip` is, if the database knows and the address is one that
    /// can be somewhere (not loopback, private, or link-local).
    pub fn locate(&self, ip: IpAddr) -> Option<Located> {
        match &self.source {
            Source::None => None,
            Source::Fixed(point) => Some(Located {
                point: *point,
                city: None,
            }),
            Source::Database(reader) => {
                if !is_routable(ip) {
                    return None;
                }
                let city = match reader.lookup(ip).and_then(|r| r.decode::<geoip2::City>()) {
                    Ok(Some(city)) => city,
                    Ok(None) => return None,
                    Err(err) => {
                        tracing::debug!(%ip, %err, "IP lookup failed");
                        return None;
                    }
                };
                let (Some(lat), Some(lon)) = (city.location.latitude, city.location.longitude)
                else {
                    return None;
                };
                let point = Point::parse(&lat.to_string(), &lon.to_string())?;
                Some(Located {
                    point,
                    city: city.city.names.english.map(str::to_owned),
                })
            }
        }
    }
}

/// Whether an address could be somewhere on the map: not loopback,
/// unspecified, link-local, or a private range.
fn is_routable(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            !(v4.is_loopback()
                || v4.is_unspecified()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_broadcast()
                || v4.octets()[0] == 100 && (64..=127).contains(&v4.octets()[1]))
        }
        IpAddr::V6(v6) => {
            if let Some(v4) = v6.to_ipv4_mapped() {
                return is_routable(IpAddr::V4(v4));
            }
            !(v6.is_loopback()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80)
        }
    }
}

/// The address a request came from: the first hop in `X-Forwarded-For`
/// when the app's reverse proxy set one (it already trusts
/// `X-Forwarded-Proto` from the same place), else the peer address.
/// `None` for a request with neither, as a test's is.
///
/// A spoofed header can only move the place search, never anything
/// the author owns, so the first hop is taken as is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientIp(pub Option<IpAddr>);

impl ClientIp {
    pub fn from_parts(headers: &HeaderMap, peer: Option<IpAddr>) -> Self {
        let forwarded = headers
            .get("x-forwarded-for")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(',').next())
            .map(str::trim)
            .and_then(|first| first.parse::<IpAddr>().ok());
        Self(forwarded.or(peer))
    }
}

impl<S: Send + Sync> FromRequestParts<S> for ClientIp {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        let peer = parts
            .extensions
            .get::<ConnectInfo<std::net::SocketAddr>>()
            .map(|info| info.0.ip());
        Ok(Self::from_parts(&parts.headers, peer))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    /// `MaxMind`'s own test database (MIT/Apache-2.0), the same format
    /// production uses.
    fn fixture() -> GeoIp {
        GeoIp::open(Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/GeoIP2-City-Test.mmdb"
        )))
        .unwrap()
    }

    #[test]
    fn a_known_address_locates_to_its_city_and_an_unknown_one_to_nothing() {
        let geo = fixture();
        assert!(geo.enabled());
        let london = geo.locate("81.2.69.160".parse().unwrap()).unwrap();
        assert_eq!(london.city.as_deref(), Some("London"));
        assert!((london.point.lat - 51.5142).abs() < 1e-6, "{london:?}");
        assert!((london.point.lon + 0.0931).abs() < 1e-6, "{london:?}");
        let boxford = geo.locate("2.125.160.216".parse().unwrap()).unwrap();
        assert_eq!(boxford.city.as_deref(), Some("Boxford"));
        assert_eq!(
            geo.locate("8.8.8.8".parse().unwrap()),
            None,
            "not in the fixture"
        );
        // Addresses that cannot be anywhere are not even looked up.
        for local in [
            "127.0.0.1",
            "10.1.2.3",
            "192.168.1.1",
            "172.16.0.1",
            "::1",
            "fe80::1",
            "fd00::1",
            "100.64.0.1",
            "0.0.0.0",
        ] {
            assert_eq!(geo.locate(local.parse().unwrap()), None, "{local}");
        }
        // A mapped address is judged as its IPv4 self.
        let mapped: IpAddr = "::ffff:81.2.69.160".parse().unwrap();
        assert_eq!(
            geo.locate(mapped).map(|l| l.city),
            Some(Some("London".into()))
        );
    }

    #[test]
    fn none_and_fixed_sources() {
        assert!(!GeoIp::none().enabled());
        assert_eq!(GeoIp::none().locate("81.2.69.160".parse().unwrap()), None);
        let fixed = GeoIp::fixed(Point {
            lat: 40.6888,
            lon: -73.9799,
        });
        assert!(fixed.enabled());
        let here = fixed.locate("127.0.0.1".parse().unwrap()).unwrap();
        assert_eq!(here.city, None);
        assert!((here.point.lat - 40.6888).abs() < 1e-9);
    }

    #[test]
    fn a_missing_database_is_an_error() {
        assert!(GeoIp::open(Path::new("/nonexistent/geo.mmdb")).is_err());
    }

    #[test]
    fn the_client_ip_is_the_first_forwarded_hop_else_the_peer() {
        let peer: IpAddr = "203.0.113.9".parse().unwrap();
        let mut headers = HeaderMap::new();
        assert_eq!(ClientIp::from_parts(&headers, None), ClientIp(None));
        assert_eq!(
            ClientIp::from_parts(&headers, Some(peer)),
            ClientIp(Some(peer))
        );
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_static("81.2.69.160, 10.0.0.1"),
        );
        assert_eq!(
            ClientIp::from_parts(&headers, Some(peer)),
            ClientIp(Some("81.2.69.160".parse().unwrap()))
        );
        headers.insert("x-forwarded-for", HeaderValue::from_static("not an ip"));
        assert_eq!(
            ClientIp::from_parts(&headers, Some(peer)),
            ClientIp(Some(peer))
        );
    }
}
