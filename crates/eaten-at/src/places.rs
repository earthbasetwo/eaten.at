//! Place search through the Open Places API, which serves Overture Maps
//! places (D34). The one upstream that is not atproto.
//!
//! The API is proximity search only: every query needs a point, and there
//! is no lookup by id. So everything the record keeps about a place (its
//! GERS id, name, address, coordinates) is captured from a search hit at
//! pick time. Results are cached for an hour so re-rendering a results
//! page never spends quota; the terms allow storing results.

use std::fmt;

use eaten_at_atproto::http::{HttpError, RawRequest};
use eaten_at_atproto::lexicon::{LatE6, LonE6};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION};
use serde::{Deserialize, Serialize};
use url::Url;

use crate::cache::Namespace;
use crate::state::AppState;

/// Where the production API lives.
pub const DEFAULT_BASE_URL: &str = "https://api.openplacesapi.com";
/// How many hits one search shows. The API caps text search at 20.
pub const RESULT_LIMIT: u8 = 10;
/// Every `place_id` the API issues starts with this; the rest is the
/// Overture GERS id (D36, verified 2026-09-13 against release 2026-08-19).
const ID_PREFIX: &str = "overture:";
/// Shortest query the API accepts, in characters.
pub const MIN_QUERY_CHARS: usize = 2;
/// Longest query the API accepts, in characters.
pub const MAX_QUERY_CHARS: usize = 128;

/// How to reach the API. Without a key, search is disabled and the
/// editor offers manual entry only.
#[derive(Clone)]
pub struct PlacesConfig {
    pub base_url: Url,
    pub api_key: Option<String>,
}

impl Default for PlacesConfig {
    fn default() -> Self {
        Self {
            base_url: Url::parse(DEFAULT_BASE_URL).expect("constant"),
            api_key: None,
        }
    }
}

impl fmt::Debug for PlacesConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlacesConfig")
            .field("base_url", &self.base_url.as_str())
            .field("api_key", &self.api_key.as_ref().map(|_| "<set>"))
            .finish()
    }
}

/// A point on the map, in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub lat: f64,
    pub lon: f64,
}

impl Point {
    /// A point from two decimal strings, if both are finite and in range.
    pub fn parse(lat: &str, lon: &str) -> Option<Self> {
        let lat: f64 = lat.trim().parse().ok()?;
        let lon: f64 = lon.trim().parse().ok()?;
        (lat.is_finite()
            && lon.is_finite()
            && (-90.0..=90.0).contains(&lat)
            && (-180.0..=180.0).contains(&lon))
        .then_some(Self { lat, lon })
    }

    /// A point from a place's stored coordinates.
    pub fn from_e6(lat: LatE6, lon: LonE6) -> Self {
        Self {
            lat: lat.degrees(),
            lon: lon.degrees(),
        }
    }

    /// The point to about a hundred metres, for cache keys.
    fn cache_key(self) -> String {
        format!("{:.3},{:.3}", self.lat, self.lon)
    }
}

/// One search result, with what the record keeps and what the results
/// list shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Hit {
    /// The Overture GERS id, without the API's `overture:` prefix.
    pub gers_id: String,
    pub name: String,
    /// One line, assembled from the API's address parts.
    pub address: Option<String>,
    pub lat_e6: i32,
    pub lon_e6: i32,
    pub distance_mi: f64,
    /// The primary category, as words (`coffee shop`).
    pub category: Option<String>,
    /// The place's website, as the API has it (may be plain `http`).
    pub website: Option<String>,
}

/// Why a search returned nothing usable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchError {
    /// No API key is configured.
    Disabled,
    /// The API rejected the query; the message is for the author.
    Query(String),
    /// The API could not be used: quota, rate limit, outage, or a bad
    /// key. Details are in the log, never on the page.
    Unavailable,
}

impl fmt::Display for SearchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("Place search is not set up on this site."),
            Self::Query(message) => f.write_str(message),
            Self::Unavailable => {
                f.write_str("Place search isn't answering right now. Try again in a moment, or enter the place by hand.")
            }
        }
    }
}

impl std::error::Error for SearchError {}

impl AppState {
    /// Whether a search can be offered at all.
    pub fn places_enabled(&self) -> bool {
        self.places().api_key.is_some()
    }

    /// Places called `q` near `near`, cached by query and point.
    pub async fn search_places(&self, q: &str, near: Point) -> Result<Vec<Hit>, SearchError> {
        let config = self.places();
        let Some(key) = &config.api_key else {
            return Err(SearchError::Disabled);
        };
        let q = normalize_query(q);
        let chars = q.chars().count();
        if chars < MIN_QUERY_CHARS {
            return Err(SearchError::Query(
                "Type a bit more of the place's name.".to_owned(),
            ));
        }
        if chars > MAX_QUERY_CHARS {
            return Err(SearchError::Query(
                "That's too long for a search.".to_owned(),
            ));
        }
        let cache_key = format!("{q}|{}", near.cache_key());
        let hits = self
            .cache()
            .get_or_fetch(Namespace::PlaceSearch, &cache_key, || async {
                let hits = fetch(self, &config.base_url, key, &q, near).await?;
                Ok::<_, SearchError>(Some(hits))
            })
            .await?;
        Ok(hits.unwrap_or_default())
    }
}

/// The query as the cache keys it: trimmed, lower-cased, one space
/// between words. The API folds case and diacritics itself.
fn normalize_query(q: &str) -> String {
    q.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

async fn fetch(
    state: &AppState,
    base_url: &Url,
    key: &str,
    q: &str,
    near: Point,
) -> Result<Vec<Hit>, SearchError> {
    let mut url = base_url
        .join("/v1/places")
        .map_err(|_| SearchError::Unavailable)?;
    url.query_pairs_mut()
        .append_pair("q", q)
        .append_pair("lat", &near.lat.to_string())
        .append_pair("lon", &near.lon.to_string())
        .append_pair("limit", &RESULT_LIMIT.to_string());
    let mut headers = HeaderMap::new();
    headers.insert(ACCEPT, HeaderValue::from_static("application/json"));
    let bearer = HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| {
        tracing::error!("the Open Places API key is not a valid header value");
        SearchError::Unavailable
    })?;
    headers.insert(AUTHORIZATION, bearer);
    let response = match state
        .http()
        .send_raw(RawRequest {
            method: reqwest::Method::GET,
            url,
            headers,
            body: Vec::new(),
        })
        .await
    {
        Ok(response) => response,
        Err(err) => {
            tracing::warn!(error = %err, "place search request failed");
            return Err(SearchError::Unavailable);
        }
    };
    let request_id = response
        .headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-")
        .to_owned();
    let status = response.status;
    if status.is_success() {
        let body: SearchResponse = response.json().map_err(|err| {
            tracing::warn!(%err, request_id, "place search response did not decode");
            SearchError::Unavailable
        })?;
        return Ok(body.hits());
    }
    let envelope: Option<ErrorEnvelope> = response.json().ok();
    let code = envelope
        .as_ref()
        .map_or_else(|| "-".to_owned(), |e| e.error.code.clone());
    let quota_remaining = response
        .headers
        .get("x-quota-remaining")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("-");
    match status.as_u16() {
        400 => {
            let message = envelope
                .map(|e| e.error.message)
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| "That search could not be run.".to_owned());
            tracing::debug!(%status, code, request_id, "place search rejected");
            Err(SearchError::Query(message))
        }
        401 | 403 => {
            tracing::error!(%status, code, request_id, "the Open Places API rejected the key");
            Err(SearchError::Unavailable)
        }
        402 | 429 => {
            tracing::warn!(%status, code, request_id, quota_remaining, "place search over quota or rate limit");
            Err(SearchError::Unavailable)
        }
        _ => {
            tracing::warn!(%status, code, request_id, "place search failed");
            Err(SearchError::Unavailable)
        }
    }
}

impl From<HttpError> for SearchError {
    fn from(_: HttpError) -> Self {
        Self::Unavailable
    }
}

/// The API's search response. Each result is decoded on its own so one
/// malformed entry drops that entry, not the page.
#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<serde_json::Value>,
}

impl SearchResponse {
    fn hits(self) -> Vec<Hit> {
        self.results
            .into_iter()
            .filter_map(|value| match serde_json::from_value::<WirePlace>(value) {
                Ok(place) => place.into_hit(),
                Err(err) => {
                    tracing::debug!(%err, "skipping a malformed place result");
                    None
                }
            })
            .collect()
    }
}

#[derive(Debug, Deserialize)]
struct WirePlace {
    place_id: String,
    name: String,
    lat: f64,
    lon: f64,
    #[serde(default)]
    distance_mi: Option<f64>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    address: Option<WireAddress>,
    #[serde(default)]
    website: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct WireAddress {
    #[serde(default)]
    formatted: Option<String>,
    #[serde(default)]
    street: Option<String>,
    #[serde(default)]
    locality: Option<String>,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    postal_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ErrorEnvelope {
    error: ErrorBody,
}

#[derive(Debug, Deserialize)]
struct ErrorBody {
    #[serde(default)]
    code: String,
    #[serde(default)]
    message: String,
}

impl WirePlace {
    /// The hit, or `None` when the coordinates or the id are unusable.
    fn into_hit(self) -> Option<Hit> {
        let gers_id = self
            .place_id
            .strip_prefix(ID_PREFIX)
            .unwrap_or(&self.place_id)
            .trim()
            .to_owned();
        let name = self.name.trim().to_owned();
        if gers_id.is_empty() || name.is_empty() {
            return None;
        }
        let lat_e6 = to_e6(self.lat).and_then(LatE6::new)?.value();
        let lon_e6 = to_e6(self.lon).and_then(LonE6::new)?.value();
        Some(Hit {
            gers_id,
            name,
            address: self.address.as_ref().and_then(one_line),
            lat_e6,
            lon_e6,
            distance_mi: self.distance_mi.unwrap_or(0.0).max(0.0),
            category: self
                .category
                .as_deref()
                .map(str::trim)
                .filter(|c| !c.is_empty())
                .map(|c| c.replace('_', " ")),
            website: self
                .website
                .as_deref()
                .map(str::trim)
                .filter(|w| !w.is_empty())
                .map(str::to_owned),
        })
    }
}

/// Degrees to microdegrees, rounded; `None` for anything not finite or
/// far out of range.
fn to_e6(degrees: f64) -> Option<i32> {
    if !degrees.is_finite() {
        return None;
    }
    let scaled = (degrees * 1_000_000.0).round();
    if scaled.abs() > f64::from(i32::MAX) {
        return None;
    }
    // Bounded above, so the cast cannot truncate or wrap.
    #[allow(clippy::cast_possible_truncation)]
    Some(scaled as i32)
}

/// One display line from the API's address parts: the street line, the
/// locality, and a region code with the postal code. `formatted` is
/// usually just the street; when it already names the locality, nothing
/// is repeated after it.
fn one_line(address: &WireAddress) -> Option<String> {
    let clean = |s: &Option<String>| {
        s.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
    };
    let street = clean(&address.street).or_else(|| clean(&address.formatted));
    let locality = clean(&address.locality);
    let region = clean(&address.region)
        .filter(|r| r.len() == 2 && r.chars().all(|c| c.is_ascii_alphabetic()))
        .map(|r| r.to_ascii_uppercase());
    let postal = clean(&address.postal_code);
    let mut parts: Vec<String> = Vec::new();
    if let Some(street) = &street {
        parts.push(street.clone());
    }
    let repeats_locality = match (&street, &locality) {
        (Some(street), Some(locality)) => street.to_lowercase().contains(&locality.to_lowercase()),
        _ => false,
    };
    if !repeats_locality {
        if let Some(locality) = locality {
            parts.push(locality);
        }
        match (region, postal) {
            (Some(region), Some(postal)) => parts.push(format!("{region} {postal}")),
            (Some(region), None) => parts.push(region),
            (None, Some(postal)) => parts.push(postal),
            (None, None) => {}
        }
    }
    (!parts.is_empty()).then(|| parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(json: serde_json::Value) -> Option<Hit> {
        serde_json::from_value::<WirePlace>(json)
            .unwrap()
            .into_hit()
    }

    #[test]
    fn a_hit_keeps_the_gers_id_and_assembles_the_address() {
        let hit = place(serde_json::json!({
            "place_id": "overture:76f1250d-8e38-40b3-a021-bfe1c16b4e1c",
            "name": " Devocion ",
            "lat": 40.701_607, "lon": -73.986_565, "distance_mi": 0.95,
            "category": "coffee_shop",
            "address": {"formatted": "105 York St", "street": "105 York St", "locality": "Brooklyn",
                        "region": "ny", "postal_code": "11201-2597", "country_code": "US"},
            "website": "https://www.devocion.com/pages/coffee-shop-dumbo-brooklyn",
            "confidence": 0.95, "operating_status": "open"
        }))
        .unwrap();
        assert_eq!(hit.gers_id, "76f1250d-8e38-40b3-a021-bfe1c16b4e1c");
        assert_eq!(hit.name, "Devocion");
        assert_eq!(
            hit.address.as_deref(),
            Some("105 York St, Brooklyn, NY 11201-2597")
        );
        assert_eq!((hit.lat_e6, hit.lon_e6), (40_701_607, -73_986_565));
        assert_eq!(hit.category.as_deref(), Some("coffee shop"));
        assert!(hit.website.is_some());
    }

    #[test]
    fn address_parts_are_optional_and_never_repeated() {
        let hit = place(serde_json::json!({
            "place_id": "overture:x", "name": "Katz's", "lat": 40.69, "lon": -73.98,
            "address": {"formatted": "New York NY US", "locality": "New York", "region": "ny", "country_code": "US"}
        }))
        .unwrap();
        assert_eq!(hit.address.as_deref(), Some("New York NY US"));
        let hit = place(serde_json::json!({
            "place_id": "overture:y", "name": "Noma", "lat": 55.68, "lon": 12.61,
            "address": {"street": "Refshalevej 96", "locality": "København", "region": "84", "postal_code": "1432"}
        }))
        .unwrap();
        assert_eq!(
            hit.address.as_deref(),
            Some("Refshalevej 96, København, 1432")
        );
        let hit = place(
            serde_json::json!({"place_id": "overture:z", "name": "Cart", "lat": 1.0, "lon": 2.0}),
        )
        .unwrap();
        assert_eq!(hit.address, None);
        assert!(hit.distance_mi.abs() < f64::EPSILON);
        assert_eq!(hit.category, None);
        // An id without the prefix is kept as issued.
        let hit =
            place(serde_json::json!({"place_id": "abc", "name": "P", "lat": 0.0, "lon": 0.0}))
                .unwrap();
        assert_eq!(hit.gers_id, "abc");
    }

    #[test]
    fn unusable_results_are_dropped() {
        assert!(place(
            serde_json::json!({"place_id": "overture:", "name": "P", "lat": 0.0, "lon": 0.0})
        )
        .is_none());
        assert!(place(
            serde_json::json!({"place_id": "overture:a", "name": "  ", "lat": 0.0, "lon": 0.0})
        )
        .is_none());
        assert!(place(
            serde_json::json!({"place_id": "overture:a", "name": "P", "lat": 91.0, "lon": 0.0})
        )
        .is_none());
        let response: SearchResponse = serde_json::from_value(serde_json::json!({
            "results": [
                {"place_id": "overture:a", "name": "Good", "lat": 1.0, "lon": 1.0},
                {"name": "no id"},
                "not an object"
            ],
            "meta": {"data_release": "2026-08-19.0"}
        }))
        .unwrap();
        let hits = response.hits();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Good");
    }

    #[test]
    fn points_parse_strictly_and_key_coarsely() {
        assert_eq!(
            Point::parse(" 40.6888 ", "-73.9799"),
            Some(Point {
                lat: 40.6888,
                lon: -73.9799
            })
        );
        assert_eq!(Point::parse("91", "0"), None);
        assert_eq!(Point::parse("0", "181"), None);
        assert_eq!(Point::parse("nan", "0"), None);
        assert_eq!(Point::parse("", "0"), None);
        assert_eq!(
            Point {
                lat: 40.68884,
                lon: -73.97991
            }
            .cache_key(),
            "40.689,-73.980"
        );
        assert_eq!(normalize_query("  Katz's   Deli "), "katz's deli");
    }
}
