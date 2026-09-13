//! Our own lexicons: `at.eaten.*`.
//!
//! A write-up is a `site.standard.document` whose `content` is an
//! [`Visit`]. The place, the date, the meal, and the rating are typed;
//! the prose sits inside the visit as an open-union `body`. Strings are
//! stored as written; the app validates at the point of use.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::common::{lenient_option, BlobRef, Datetime};
use crate::at_uri::AtUri;

/// NSID of the visit object placed in `document.content`.
pub const VISIT_NSID: &str = "at.eaten.visit";
/// NSID of the place object embedded in a visit.
pub const PLACE_NSID: &str = "at.eaten.place";
/// Most photos on one visit (lexicon `maxLength`).
pub const MAX_PHOTOS: usize = 24;
/// Largest photo blob, in bytes (lexicon `maxSize`).
pub const MAX_PHOTO_BYTES: usize = 1_000_000;

/// A closed set of strings from a lexicon's `knownValues`, matched
/// exactly and case-sensitively; anything else is another client's
/// vocabulary and is carried through untouched.
pub trait KnownValue: Copy + Sized + 'static {
    /// Every known value, in lexicon order.
    const ALL: &'static [Self];
    /// The value as written in records.
    fn as_str(self) -> &'static str;
    /// Label for display.
    fn display_name(self) -> &'static str;
    /// Exact match against the lexicon's `knownValues`.
    fn from_value(value: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|v| v.as_str() == value)
    }
}

macro_rules! known_values {
    ($(#[$meta:meta])* $name:ident { $($variant:ident => $value:literal, $label:literal;)+ }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub enum $name {
            $($variant,)+
        }

        impl KnownValue for $name {
            const ALL: &'static [Self] = &[$(Self::$variant,)+];

            fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value,)+
                }
            }

            fn display_name(self) -> &'static str {
                match self {
                    $(Self::$variant => $label,)+
                }
            }
        }

        impl $name {
            /// Every known value, in lexicon order.
            pub const ALL: &'static [Self] = <Self as KnownValue>::ALL;

            /// Exact, case-sensitive match against the lexicon's `knownValues`.
            pub fn from_value(value: &str) -> Option<Self> {
                <Self as KnownValue>::from_value(value)
            }

            /// The value as written in records.
            pub fn as_str(self) -> &'static str {
                <Self as KnownValue>::as_str(self)
            }

            /// Label for display.
            pub fn display_name(self) -> &'static str {
                <Self as KnownValue>::display_name(self)
            }
        }
    };
}

known_values! {
    /// The `knownValues` of `at.eaten.place#externalUrl.service`.
    KnownService {
        OfficialSite => "officialSite", "Official site";
    }
}

known_values! {
    /// The `knownValues` of `at.eaten.visit.meal`.
    Meal {
        Breakfast => "breakfast", "Breakfast";
        Brunch => "brunch", "Brunch";
        Lunch => "lunch", "Lunch";
        Dinner => "dinner", "Dinner";
        LateNight => "lateNight", "Late night";
    }
}

/// `at.eaten.visit`: the content of a write-up.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Visit {
    #[serde(rename = "$type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    pub place: Place,
    #[serde(rename = "visitedOn")]
    pub visited_on: VisitDate,
    /// Which meal; a [`Meal`] value or another client's word.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub meal: Option<String>,
    /// Out-of-range values from other clients read as unrated.
    #[serde(
        default,
        deserialize_with = "lenient_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub rating: Option<Rating>,
    /// Open union. Kept raw until the app's model interprets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
    /// Photos in the author's order. A malformed entry from another
    /// client is dropped, not the visit.
    #[serde(
        default,
        deserialize_with = "lenient_photos",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub photos: Vec<Photo>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// `at.eaten.visit#photo`: one photo of the visit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Photo {
    pub image: BlobRef,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alt: Option<String>,
    #[serde(
        rename = "aspectRatio",
        default,
        deserialize_with = "lenient_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub aspect_ratio: Option<AspectRatio>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// `at.eaten.visit#aspectRatio`: an image's proportions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AspectRatio {
    pub width: u32,
    pub height: u32,
}

/// Each photo decoded on its own so one bad entry drops that entry.
fn lenient_photos<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Photo>, D::Error> {
    let raw = Vec::<Value>::deserialize(deserializer)?;
    Ok(raw
        .into_iter()
        .filter_map(|value| match serde_json::from_value::<Photo>(value) {
            Ok(photo) => Some(photo),
            Err(err) => {
                tracing::debug!(%err, "ignoring malformed photo");
                None
            }
        })
        .collect())
}

impl Visit {
    /// The meal, if it is one we know.
    pub fn known_meal(&self) -> Option<Meal> {
        self.meal.as_deref().and_then(Meal::from_value)
    }
}

/// `at.eaten.place`: where a visit happened.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Place {
    #[serde(rename = "$type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Out-of-range values from other clients read as absent.
    #[serde(
        default,
        deserialize_with = "lenient_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub price: Option<PriceBand>,
    /// The Overture Maps GERS id: the place's identity (D33).
    #[serde(rename = "gersId", default, skip_serializing_if = "Option::is_none")]
    pub gers_id: Option<String>,
    /// Out-of-range values from other clients read as absent.
    #[serde(
        rename = "latE6",
        default,
        deserialize_with = "lenient_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub lat_e6: Option<LatE6>,
    #[serde(
        rename = "lonE6",
        default,
        deserialize_with = "lenient_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub lon_e6: Option<LonE6>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<ExternalUrl>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl Place {
    /// Whether two places share a GERS id, which is what makes them the
    /// same place. A place without one matches nothing.
    pub fn same_as(&self, other: &Self) -> bool {
        matches!((&self.gers_id, &other.gers_id), (Some(a), Some(b)) if a == b)
    }

    /// The place's position in degrees, when both halves are present.
    pub fn coordinates(&self) -> Option<(f64, f64)> {
        Some((self.lat_e6?.degrees(), self.lon_e6?.degrees()))
    }
}

macro_rules! micro_degrees {
    ($(#[$meta:meta])* $name:ident, $limit:literal, $what:literal) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        #[serde(try_from = "i64", into = "i64")]
        pub struct $name(i32);

        impl $name {
            /// The largest magnitude, in microdegrees.
            pub const LIMIT: i32 = $limit;

            /// A value from microdegrees, if it is in range.
            pub fn new(value: i32) -> Option<Self> {
                (-Self::LIMIT..=Self::LIMIT)
                    .contains(&value)
                    .then_some(Self(value))
            }

            /// The stored integer.
            pub fn value(self) -> i32 {
                self.0
            }

            /// The value in degrees.
            pub fn degrees(self) -> f64 {
                f64::from(self.0) / 1_000_000.0
            }
        }

        impl TryFrom<i64> for $name {
            type Error = String;

            fn try_from(value: i64) -> Result<Self, Self::Error> {
                i32::try_from(value)
                    .ok()
                    .and_then(Self::new)
                    .ok_or_else(|| format!("{} {value} is out of range", $what))
            }
        }

        impl From<$name> for i64 {
            fn from(value: $name) -> Self {
                Self::from(value.0)
            }
        }
    };
}

micro_degrees! {
    /// A latitude in microdegrees, `-90_000_000..=90_000_000`. Atproto
    /// lexicons have no float type; degrees × 1 000 000 are exact.
    LatE6, 90_000_000, "latitude"
}

micro_degrees! {
    /// A longitude in microdegrees, `-180_000_000..=180_000_000`.
    LonE6, 180_000_000, "longitude"
}

/// `at.eaten.place#externalUrl`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalUrl {
    pub url: String,
    /// Service name. Dispatch happens on an exact match against
    /// [`KnownService`]; anything else is another client's vocabulary and
    /// is preserved untouched.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl ExternalUrl {
    /// The service this entry declares, if it is one we know.
    pub fn known_service(&self) -> Option<KnownService> {
        self.service.as_deref().and_then(KnownService::from_value)
    }
}

/// The house rating scale: four steps, stored as the integers 1 to 4.
/// The words and the plus glyphs are how eaten.at renders them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub enum Rating {
    Solid = 1,
    Recommended = 2,
    StronglyRecommended = 3,
    CantMiss = 4,
}

impl Rating {
    /// Every step, lowest first.
    pub const ALL: [Self; 4] = [
        Self::Solid,
        Self::Recommended,
        Self::StronglyRecommended,
        Self::CantMiss,
    ];

    /// The stored integer.
    pub fn value(self) -> u8 {
        self as u8
    }

    /// The step from its stored integer.
    pub fn from_value(value: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.value() == value)
    }

    /// The verdict in words.
    pub fn word(self) -> &'static str {
        match self {
            Self::Solid => "Solid",
            Self::Recommended => "Recommended",
            Self::StronglyRecommended => "Strongly Recommended",
            Self::CantMiss => "Can’t Miss",
        }
    }

    /// The verdict as plus signs, one per step.
    pub fn marks(self) -> &'static str {
        match self {
            Self::Solid => "+",
            Self::Recommended => "++",
            Self::StronglyRecommended => "+++",
            Self::CantMiss => "++++",
        }
    }
}

impl TryFrom<u8> for Rating {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::from_value(value).ok_or_else(|| format!("rating {value} is not between 1 and 4"))
    }
}

impl From<Rating> for u8 {
    fn from(rating: Rating) -> Self {
        rating.value()
    }
}

/// A place's price band, 1 to 4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct PriceBand(u8);

impl PriceBand {
    pub const MIN: u8 = 1;
    pub const MAX: u8 = 4;

    /// A band from its integer, if it is in range.
    pub fn new(value: u8) -> Option<Self> {
        (Self::MIN..=Self::MAX)
            .contains(&value)
            .then_some(Self(value))
    }

    /// The stored integer.
    pub fn value(self) -> u8 {
        self.0
    }

    /// The band as that many dollar signs.
    pub fn signs(self) -> String {
        "$".repeat(usize::from(self.0))
    }
}

impl TryFrom<u8> for PriceBand {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value).ok_or_else(|| format!("price band {value} is not between 1 and 4"))
    }
}

impl From<PriceBand> for u8 {
    fn from(band: PriceBand) -> Self {
        band.0
    }
}

/// A calendar date, `YYYY-MM-DD`, as `at.eaten.visit.visitedOn` carries
/// it. Kept as text and validated on read; equality follows the date.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VisitDate {
    date: jiff::civil::Date,
}

impl VisitDate {
    /// Parse `YYYY-MM-DD` exactly: ten characters, hyphens in place, a
    /// real date.
    pub fn parse(s: &str) -> Result<Self, String> {
        let bytes = s.as_bytes();
        let shape_ok = bytes.len() == 10
            && bytes[4] == b'-'
            && bytes[7] == b'-'
            && bytes
                .iter()
                .enumerate()
                .all(|(i, b)| matches!(i, 4 | 7) || b.is_ascii_digit());
        if !shape_ok {
            return Err(format!("{s:?} is not a YYYY-MM-DD date"));
        }
        let date: jiff::civil::Date = s.parse().map_err(|e| format!("{s:?}: {e}"))?;
        Ok(Self { date })
    }

    /// Wrap a civil date.
    pub fn from_date(date: jiff::civil::Date) -> Self {
        Self { date }
    }

    /// Today, in the system's time zone.
    pub fn today() -> Self {
        Self::from_date(jiff::Zoned::now().date())
    }

    /// The day this date names.
    pub fn date(&self) -> jiff::civil::Date {
        self.date
    }

    /// `YYYY-MM-DD`.
    pub fn as_string(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}",
            self.date.year(),
            self.date.month(),
            self.date.day()
        )
    }
}

impl fmt::Debug for VisitDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VisitDate({})", self.as_string())
    }
}

impl fmt::Display for VisitDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_string())
    }
}

impl Serialize for VisitDate {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.as_string())
    }
}

impl<'de> Deserialize<'de> for VisitDate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

/// NSID of the per-user preferences record.
pub const PREFERENCES_NSID: &str = "at.eaten.preferences";
/// The only record key a preferences record may have.
pub const PREFERENCES_RKEY: &str = "self";

/// `at.eaten.preferences`: per-user settings, one record per repo.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Preferences {
    #[serde(rename = "$type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    /// Publication to show for a bare DID route and to preselect in the
    /// editor. A hint, not a restriction.
    #[serde(
        rename = "defaultPublication",
        default,
        deserialize_with = "lenient_at_uri",
        skip_serializing_if = "Option::is_none"
    )]
    pub default_publication: Option<AtUri>,
    /// Whether the Bluesky crosspost toggle defaults on.
    #[serde(
        rename = "crosspostToBluesky",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub crosspost_to_bluesky: Option<bool>,
    #[serde(
        rename = "createdAt",
        default,
        deserialize_with = "Datetime::deserialize_lenient",
        skip_serializing_if = "Option::is_none"
    )]
    pub created_at: Option<Datetime>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

impl Preferences {
    /// Whether crossposting defaults on. Absent means the lexicon default, `false`.
    pub fn crosspost_default(&self) -> bool {
        self.crosspost_to_bluesky.unwrap_or(false)
    }
}

/// A malformed `defaultPublication` reads as absent: the plan treats an
/// unusable preference the same as a missing one (§4.3).
fn lenient_at_uri<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<AtUri>, D::Error> {
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.and_then(|s| AtUri::parse(&s).ok()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn visit_json() -> Value {
        serde_json::json!({
            "$type": "at.eaten.visit",
            "place": {
                "name": "Sample Place",
                "address": "1 Example St",
                "price": 2,
                "gersId": "08f2a5b6c7d8e9f0a1b2c3d4e5f60718",
                "latE6": 40_688_838,
                "lonE6": -73_979_914,
                "urls": [
                    {"url": "https://example.com/elsewhere", "service": "menu"},
                    {"url": "https://example.com", "service": "officialSite", "label": "Home"}
                ],
                "neighbourhood": "Old Town"
            },
            "visitedOn": "2026-09-12",
            "meal": "dinner",
            "rating": 3,
            "body": {"$type": "at.markpub.markdown", "text": {"markdown": "Good."}},
            "photos": [
                {"image": {"$type": "blob", "ref": {"$link": "bafyone"}, "mimeType": "image/jpeg", "size": 5},
                 "alt": "The room", "aspectRatio": {"width": 4, "height": 3}}
            ],
            "future": true
        })
    }

    #[test]
    fn visit_round_trips_preserving_unknown_values_and_fields() {
        let json = visit_json();
        let visit: Visit = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(visit.place.name, "Sample Place");
        assert_eq!(visit.place.price, PriceBand::new(2));
        assert_eq!(
            visit.place.gers_id.as_deref(),
            Some("08f2a5b6c7d8e9f0a1b2c3d4e5f60718")
        );
        assert_eq!(visit.place.coordinates(), Some((40.688_838, -73.979_914)));
        assert_eq!(visit.place.urls[0].known_service(), None);
        assert_eq!(
            visit.place.urls[1].known_service(),
            Some(KnownService::OfficialSite)
        );
        assert_eq!(visit.visited_on.as_string(), "2026-09-12");
        assert_eq!(visit.known_meal(), Some(Meal::Dinner));
        assert_eq!(visit.rating, Some(Rating::StronglyRecommended));
        assert_eq!(visit.photos.len(), 1);
        assert_eq!(visit.photos[0].image.cid(), "bafyone");
        assert_eq!(visit.photos[0].alt.as_deref(), Some("The room"));
        assert_eq!(serde_json::to_value(&visit).unwrap(), json);
    }

    #[test]
    fn a_malformed_photo_is_dropped_not_the_visit() {
        let mut json = visit_json();
        json["photos"] = serde_json::json!([
            {"alt": "no image"},
            "nonsense",
            {"image": {"$type": "blob", "ref": {"$link": "bafytwo"}, "mimeType": "image/png", "size": 9},
             "aspectRatio": {"width": "wide"}}
        ]);
        let visit: Visit = serde_json::from_value(json).unwrap();
        assert_eq!(visit.photos.len(), 1);
        assert_eq!(visit.photos[0].image.cid(), "bafytwo");
        assert_eq!(
            visit.photos[0].aspect_ratio, None,
            "a bad ratio reads as absent"
        );
        let none: Visit = serde_json::from_value(serde_json::json!({
            "place": {"name": "P"}, "visitedOn": "2026-09-12"
        }))
        .unwrap();
        assert!(none.photos.is_empty());
        assert!(serde_json::to_value(&none).unwrap().get("photos").is_none());
    }

    #[test]
    fn known_values_are_exact_matches() {
        assert_eq!(
            KnownService::from_value("officialSite"),
            Some(KnownService::OfficialSite)
        );
        assert_eq!(KnownService::from_value("OfficialSite"), None);
        assert_eq!(Meal::from_value("lateNight"), Some(Meal::LateNight));
        assert_eq!(Meal::from_value("late night"), None);
        for s in KnownService::ALL {
            assert_eq!(KnownService::from_value(s.as_str()), Some(*s));
        }
        for m in Meal::ALL {
            assert_eq!(Meal::from_value(m.as_str()), Some(*m));
        }
    }

    #[test]
    fn coordinates_are_bounded_microdegrees() {
        assert_eq!(LatE6::new(90_000_000).map(LatE6::degrees), Some(90.0));
        assert_eq!(LatE6::new(90_000_001), None);
        assert_eq!(
            LonE6::new(-180_000_000).map(LonE6::value),
            Some(-180_000_000)
        );
        assert_eq!(LonE6::new(180_000_001), None);
        assert!(serde_json::from_value::<LatE6>(serde_json::json!(91_000_000)).is_err());
        assert!(serde_json::from_value::<LatE6>(serde_json::json!(40.5)).is_err());
        assert_eq!(serde_json::to_value(LonE6::new(-1).unwrap()).unwrap(), -1);
        let mut json = visit_json();
        json["place"]["latE6"] = serde_json::json!(95_000_000);
        let visit: Visit = serde_json::from_value(json).unwrap();
        assert_eq!(visit.place.lat_e6, None, "out of range reads as absent");
        assert_eq!(visit.place.coordinates(), None, "half a position is none");
    }

    #[test]
    fn rating_is_an_integer_between_one_and_four() {
        for r in Rating::ALL {
            assert_eq!(Rating::from_value(r.value()), Some(r));
            assert_eq!(r.marks().len(), usize::from(r.value()));
        }
        assert_eq!(serde_json::to_value(Rating::CantMiss).unwrap(), 4);
        assert_eq!(
            serde_json::from_value::<Rating>(serde_json::json!(2)).unwrap(),
            Rating::Recommended
        );
        assert!(serde_json::from_value::<Rating>(serde_json::json!(0)).is_err());
        assert!(serde_json::from_value::<Rating>(serde_json::json!(5)).is_err());
        assert!(serde_json::from_value::<Rating>(serde_json::json!("3")).is_err());
    }

    #[test]
    fn out_of_range_rating_and_price_read_as_absent() {
        let mut json = visit_json();
        json["rating"] = serde_json::json!(9);
        json["place"]["price"] = serde_json::json!(-1);
        let visit: Visit = serde_json::from_value(json).unwrap();
        assert_eq!(visit.rating, None);
        assert_eq!(visit.place.price, None);
    }

    #[test]
    fn visit_requires_place_and_a_real_date() {
        assert!(serde_json::from_value::<Visit>(serde_json::json!({})).is_err());
        assert!(serde_json::from_value::<Visit>(serde_json::json!({
            "place": {"name": "P"}, "visitedOn": "2026-02-30"
        }))
        .is_err());
        assert!(serde_json::from_value::<Visit>(serde_json::json!({
            "place": {"name": "P"}, "visitedOn": "2026-09-12T12:00:00Z"
        }))
        .is_err());
        let v: Visit = serde_json::from_value(serde_json::json!({
            "place": {"name": "P"}, "visitedOn": "2026-09-12"
        }))
        .unwrap();
        assert_eq!(v.rating, None);
        assert_eq!(v.body, None);
    }

    #[test]
    fn visit_dates_parse_strictly_and_print_canonically() {
        assert_eq!(
            VisitDate::parse("2026-09-12").unwrap().to_string(),
            "2026-09-12"
        );
        assert!(VisitDate::parse("2026-9-12").is_err());
        assert!(VisitDate::parse("12/09/2026").is_err());
        assert!(VisitDate::parse("2026-13-01").is_err());
        assert!(VisitDate::parse("").is_err());
        assert!(VisitDate::parse("2026-09-12").unwrap() < VisitDate::parse("2026-09-13").unwrap());
        assert_eq!(VisitDate::today().as_string().len(), 10);
    }

    #[test]
    fn places_match_on_a_shared_gers_id() {
        let a: Place =
            serde_json::from_value(serde_json::json!({"name": "A", "gersId": "1"})).unwrap();
        let b: Place =
            serde_json::from_value(serde_json::json!({"name": "B", "gersId": "1"})).unwrap();
        let c: Place = serde_json::from_value(serde_json::json!({"name": "A"})).unwrap();
        assert!(a.same_as(&b));
        assert!(!a.same_as(&c), "a shared name is not identity");
        assert!(!c.same_as(&c), "no id matches nothing, not even itself");
        assert_eq!(PriceBand::new(2).unwrap().signs(), "$$");
        assert_eq!(PriceBand::new(5), None);
    }

    #[test]
    fn absent_fields_mean_defaults() {
        let p: Preferences = serde_json::from_str("{}").unwrap();
        assert!(p.default_publication.is_none());
        assert!(!p.crosspost_default());
    }

    #[test]
    fn malformed_default_publication_reads_as_absent() {
        let p: Preferences = serde_json::from_str(
            r#"{"defaultPublication": "https://nope", "crosspostToBluesky": true}"#,
        )
        .unwrap();
        assert!(p.default_publication.is_none());
        assert!(p.crosspost_default());
    }

    #[test]
    fn unknown_fields_survive_round_trip() {
        let json = r#"{"$type":"at.eaten.preferences","defaultPublication":"at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/abc","future":{"x":1}}"#;
        let p: Preferences = serde_json::from_str(json).unwrap();
        assert_eq!(p.default_publication.as_ref().unwrap().rkey(), "abc");
        let out: serde_json::Value = serde_json::to_value(&p).unwrap();
        assert_eq!(out["future"]["x"], 1);
    }
}
