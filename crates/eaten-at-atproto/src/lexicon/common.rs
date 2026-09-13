//! Types shared across lexicons: blobs, strong refs, labels, datetimes.

use std::fmt;

use serde::{Deserialize, Deserializer, Serialize};

/// A reference to a blob in the same repository.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobRef {
    #[serde(rename = "$type")]
    pub type_: String,
    #[serde(rename = "ref")]
    pub link: BlobLink,
    #[serde(rename = "mimeType")]
    pub mime_type: String,
    pub size: u64,
}

/// The `{ "$link": "<cid>" }` object inside a blob ref.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlobLink {
    #[serde(rename = "$link")]
    pub cid: String,
}

impl BlobRef {
    /// The blob's CID.
    pub fn cid(&self) -> &str {
        &self.link.cid
    }
}

/// `com.atproto.repo.strongRef`: a record pinned to a specific version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StrongRef {
    pub uri: String,
    pub cid: String,
}

/// `com.atproto.label.defs#selfLabels`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfLabels {
    #[serde(rename = "$type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    #[serde(default)]
    pub values: Vec<SelfLabel>,
}

/// One self-applied label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfLabel {
    pub val: String,
}

/// Deserialize an optional structured field leniently: a value that is
/// present but does not match `T` reads as `None` instead of failing the
/// whole record. Records come from other clients and other people's
/// servers; one bad sub-object must not make a document unreadable.
pub fn lenient_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: serde::de::DeserializeOwned,
{
    let raw = Option::<serde_json::Value>::deserialize(deserializer)?;
    Ok(
        raw.and_then(|value| match serde_json::from_value::<T>(value) {
            Ok(v) => Some(v),
            Err(err) => {
                tracing::debug!(%err, "ignoring malformed optional field");
                None
            }
        }),
    )
}

/// An atproto datetime: RFC 3339 with a required timezone.
///
/// The original string is kept and written back verbatim, so a record read
/// from another client and re-saved keeps its exact timestamp text (for
/// example `.000Z` fractional seconds that a re-formatter would drop).
///
/// Required fields fail deserialization on a malformed value, as the lexicon
/// says they should. For optional fields use [`Datetime::deserialize_lenient`]
/// so a bad value from another client reads as absent rather than sinking
/// the whole record.
#[derive(Clone)]
pub struct Datetime {
    raw: String,
    ts: jiff::Timestamp,
}

impl Datetime {
    /// Current time, formatted the way atproto clients conventionally do:
    /// UTC with millisecond precision.
    pub fn now() -> Self {
        Self::from_timestamp(jiff::Timestamp::now())
    }

    /// Wrap an instant, formatting it as UTC with millisecond precision.
    pub fn from_timestamp(ts: jiff::Timestamp) -> Self {
        let raw = ts.strftime("%Y-%m-%dT%H:%M:%S.%3fZ").to_string();
        Self { raw, ts }
    }

    /// Parse an RFC 3339 string, keeping it verbatim for re-serialization.
    pub fn parse(s: &str) -> Result<Self, jiff::Error> {
        let ts = s.parse()?;
        Ok(Self {
            raw: s.to_owned(),
            ts,
        })
    }

    /// The instant this datetime names.
    pub fn timestamp(&self) -> jiff::Timestamp {
        self.ts
    }

    /// The string as it was read or generated.
    pub fn as_str(&self) -> &str {
        &self.raw
    }

    /// Deserializer for `Option<Datetime>` that maps malformed input to `None`.
    pub fn deserialize_lenient<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Self>, D::Error> {
        let raw = Option::<String>::deserialize(deserializer)?;
        Ok(raw.and_then(|s| match Self::parse(&s) {
            Ok(dt) => Some(dt),
            Err(err) => {
                tracing::debug!(value = %s, %err, "ignoring malformed optional datetime");
                None
            }
        }))
    }
}

/// Equality, hashing, and ordering follow the instant, not the text: two
/// spellings of the same moment are the same datetime.
impl PartialEq for Datetime {
    fn eq(&self, other: &Self) -> bool {
        self.ts == other.ts
    }
}

impl Eq for Datetime {}

impl std::hash::Hash for Datetime {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.ts.hash(state);
    }
}

impl PartialOrd for Datetime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Datetime {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.ts.cmp(&other.ts)
    }
}

impl fmt::Debug for Datetime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Datetime({})", self.raw)
    }
}

impl fmt::Display for Datetime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

impl Serialize for Datetime {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.raw)
    }
}

impl<'de> Deserialize<'de> for Datetime {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = String::deserialize(deserializer)?;
        Self::parse(&raw).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Deserialize)]
    struct Holder {
        required: Datetime,
        #[serde(default, deserialize_with = "Datetime::deserialize_lenient")]
        optional: Option<Datetime>,
    }

    #[test]
    fn required_datetime_is_strict() {
        assert!(serde_json::from_str::<Holder>(r#"{"required": "yesterday"}"#).is_err());
        let h: Holder = serde_json::from_str(r#"{"required": "2026-09-07T12:00:00Z"}"#).unwrap();
        assert_eq!(h.required.to_string(), "2026-09-07T12:00:00Z");
        assert!(h.optional.is_none());
    }

    #[test]
    fn optional_datetime_is_lenient() {
        let h: Holder = serde_json::from_str(
            r#"{"required": "2026-09-07T12:00:00Z", "optional": "not a date"}"#,
        )
        .unwrap();
        assert!(h.optional.is_none());
        let h: Holder = serde_json::from_str(
            r#"{"required": "2026-09-07T12:00:00Z", "optional": "2026-09-08T00:00:00+02:00"}"#,
        )
        .unwrap();
        assert!(h.optional.unwrap() > h.required);
    }

    #[test]
    fn datetime_serializes_verbatim_and_now_uses_millis() {
        let dt = Datetime::parse("2026-02-10T00:00:00.000Z").unwrap();
        assert_eq!(
            serde_json::to_string(&dt).unwrap(),
            "\"2026-02-10T00:00:00.000Z\""
        );
        let offset = Datetime::parse("2026-02-10T02:00:00+02:00").unwrap();
        assert_eq!(offset, dt);
        assert_eq!(offset.as_str(), "2026-02-10T02:00:00+02:00");
        let now = Datetime::now().as_str().to_owned();
        assert!(
            now.ends_with('Z') && now.len() == "2026-09-07T21:00:00.000Z".len(),
            "{now}"
        );
    }

    #[test]
    fn lenient_option_drops_malformed_values_only() {
        #[derive(Deserialize)]
        struct Holder {
            #[serde(default, deserialize_with = "lenient_option")]
            blob: Option<BlobRef>,
        }
        let ok: Holder = serde_json::from_str(
            r#"{"blob": {"$type":"blob","ref":{"$link":"c"},"mimeType":"image/png","size":1}}"#,
        )
        .unwrap();
        assert!(ok.blob.is_some());
        let bad: Holder = serde_json::from_str(r#"{"blob": {"size": "huge"}}"#).unwrap();
        assert!(bad.blob.is_none());
        let absent: Holder = serde_json::from_str("{}").unwrap();
        assert!(absent.blob.is_none());
    }

    #[test]
    fn blob_ref_round_trips() {
        let json =
            r#"{"$type":"blob","ref":{"$link":"bafkreiabc"},"mimeType":"image/png","size":123}"#;
        let blob: BlobRef = serde_json::from_str(json).unwrap();
        assert_eq!(blob.cid(), "bafkreiabc");
        assert_eq!(serde_json::to_string(&blob).unwrap(), json);
    }
}
