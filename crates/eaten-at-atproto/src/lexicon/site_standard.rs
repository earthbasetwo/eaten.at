//! `site.standard.*` records, validated against the schemas pinned in
//! `lexicons/upstream/`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::Value;

use super::common::{lenient_option, BlobRef, Datetime, SelfLabels, StrongRef};

/// NSID of the document collection.
pub const DOCUMENT_NSID: &str = "site.standard.document";
/// NSID of the publication collection.
pub const PUBLICATION_NSID: &str = "site.standard.publication";

/// `site.standard.document`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Document {
    #[serde(rename = "$type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    /// `at://` of the publication, or `https://` for a loose document.
    pub site: String,
    pub title: String,
    #[serde(rename = "publishedAt")]
    pub published_at: Datetime,
    /// Leading slash. Appended to `publication.url` for the canonical URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        rename = "coverImage",
        default,
        deserialize_with = "lenient_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub cover_image: Option<BlobRef>,
    /// Open union. Kept raw until the app's model interprets it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<Value>,
    #[serde(
        rename = "textContent",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub text_content: Option<String>,
    #[serde(
        rename = "bskyPostRef",
        default,
        deserialize_with = "lenient_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub bsky_post_ref: Option<StrongRef>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// The schema types this as a single open union but describes it as an
    /// array (plan §3.4a). Both shapes are read; one object is written when
    /// there is exactly one, an array otherwise.
    #[serde(
        default,
        deserialize_with = "one_or_many",
        serialize_with = "one_or_many_out",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub links: Vec<Value>,
    #[serde(
        default,
        deserialize_with = "lenient_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub labels: Option<SelfLabels>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributors: Vec<Contributor>,
    #[serde(
        rename = "updatedAt",
        default,
        deserialize_with = "Datetime::deserialize_lenient",
        skip_serializing_if = "Option::is_none"
    )]
    pub updated_at: Option<Datetime>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// `site.standard.document#contributor`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Contributor {
    pub did: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(
        rename = "displayName",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub display_name: Option<String>,
}

/// `site.standard.publication`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Publication {
    #[serde(rename = "$type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    /// Base web URL, no trailing slash.
    pub url: String,
    pub name: String,
    #[serde(
        default,
        deserialize_with = "lenient_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub icon: Option<BlobRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(
        rename = "basicTheme",
        default,
        deserialize_with = "lenient_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub basic_theme: Option<ThemeBasic>,
    #[serde(
        default,
        deserialize_with = "lenient_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub labels: Option<SelfLabels>,
    #[serde(
        default,
        deserialize_with = "lenient_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub preferences: Option<PublicationPreferences>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

impl Publication {
    /// `url` with any trailing slashes removed, ready to prepend to a path.
    pub fn base_url(&self) -> &str {
        self.url.trim_end_matches('/')
    }
}

/// `site.standard.publication#preferences`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicationPreferences {
    #[serde(
        rename = "showInDiscover",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub show_in_discover: Option<bool>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// `site.standard.theme.basic`. Colors are validated to 0–255 on read so the
/// values can be interpolated into CSS as numbers without further checks.
///
/// PDS validation requires `$type` here (`site.standard.theme.basic`) and on
/// each color (`site.standard.theme.color#rgb`); both are kept so a record
/// round-trips, and writers must set them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThemeBasic {
    #[serde(rename = "$type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    pub background: Rgb,
    pub foreground: Rgb,
    pub accent: Rgb,
    #[serde(rename = "accentForeground")]
    pub accent_foreground: Rgb,
}

/// `site.standard.theme.color#rgb`. `u8` enforces the 0–255 range at the
/// type level; anything out of range fails deserialization, and a bad theme
/// reads as no theme at the publication level.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    #[serde(rename = "$type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Lowercase hex without a `#`, e.g. `0084b4`.
    pub fn to_hex(&self) -> String {
        format!("{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

fn one_or_many<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<Value>, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::Array(items) => items,
        Value::Null => Vec::new(),
        single => vec![single],
    })
}

fn one_or_many_out<S: Serializer>(links: &[Value], serializer: S) -> Result<S::Ok, S::Error> {
    match links {
        [single] => single.serialize(serializer),
        many => many.serialize(serializer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL_DOC: &str = include_str!("../../tests/fixtures/standard-site-document.json");
    const REAL_PUB: &str = include_str!("../../tests/fixtures/standard-site-publication.json");

    fn link() -> Value {
        serde_json::json!({
            "$type": "com.example.link",
            "url": "https://example.com/elsewhere"
        })
    }

    fn minimal(links: &Value) -> Value {
        serde_json::json!({
            "$type": "site.standard.document",
            "site": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/3me5vykp6lf2y",
            "title": "T",
            "publishedAt": "2026-09-07T00:00:00Z",
            "links": links
        })
    }

    #[test]
    fn real_document_round_trips_with_extra_fields() {
        let doc: Document = serde_json::from_str(REAL_DOC).unwrap();
        assert_eq!(doc.title, "Document Lexicon");
        assert_eq!(doc.path.as_deref(), Some("/docs/lexicons/document"));
        assert!(doc.cover_image.is_some());
        assert_eq!(
            doc.extra["canonicalUrl"],
            "https://standard.site/docs/lexicons/document"
        );
        let out = serde_json::to_value(&doc).unwrap();
        let original: Value = serde_json::from_str(REAL_DOC).unwrap();
        assert_eq!(out, original);
    }

    #[test]
    fn real_publication_round_trips() {
        let publication: Publication = serde_json::from_str(REAL_PUB).unwrap();
        assert_eq!(publication.base_url(), "https://standard.site");
        assert_eq!(
            publication.preferences.unwrap().show_in_discover,
            Some(true)
        );
        let out =
            serde_json::to_value(serde_json::from_str::<Publication>(REAL_PUB).unwrap()).unwrap();
        assert_eq!(out, serde_json::from_str::<Value>(REAL_PUB).unwrap());
    }

    #[test]
    fn links_reads_single_object() {
        let doc: Document = serde_json::from_value(minimal(&link())).unwrap();
        assert_eq!(doc.links, vec![link()]);
    }

    #[test]
    fn links_reads_array() {
        let doc: Document =
            serde_json::from_value(minimal(&Value::Array(vec![link(), link()]))).unwrap();
        assert_eq!(doc.links.len(), 2);
    }

    #[test]
    fn links_absent_or_null_is_empty() {
        let doc: Document = serde_json::from_value(minimal(&Value::Null)).unwrap();
        assert!(doc.links.is_empty());
        let mut without = minimal(&Value::Null);
        without.as_object_mut().unwrap().remove("links");
        let doc: Document = serde_json::from_value(without).unwrap();
        assert!(doc.links.is_empty());
    }

    #[test]
    fn links_writes_singular_for_one_and_array_for_many() {
        let one: Document = serde_json::from_value(minimal(&link())).unwrap();
        assert!(serde_json::to_value(&one).unwrap()["links"].is_object());
        let two: Document =
            serde_json::from_value(minimal(&Value::Array(vec![link(), link()]))).unwrap();
        assert!(serde_json::to_value(&two).unwrap()["links"].is_array());
        let mut none = one.clone();
        none.links.clear();
        assert!(serde_json::to_value(&none).unwrap().get("links").is_none());
    }

    #[test]
    fn theme_parses_and_rejects_out_of_range() {
        let json = serde_json::json!({
            "$type": "site.standard.theme.basic",
            "background": {"$type": "site.standard.theme.color#rgb", "r": 255, "g": 255, "b": 255},
            "foreground": {"r": 0, "g": 0, "b": 0},
            "accent": {"r": 0, "g": 132, "b": 180},
            "accentForeground": {"r": 255, "g": 255, "b": 255}
        });
        let theme: ThemeBasic = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(theme.accent.to_hex(), "0084b4");
        assert_eq!(
            serde_json::to_value(&theme).unwrap(),
            json,
            "$type values round-trip"
        );
        assert!(
            serde_json::from_value::<Rgb>(serde_json::json!({"r": 256, "g": 0, "b": 0})).is_err()
        );
        assert!(
            serde_json::from_value::<Rgb>(serde_json::json!({"r": -1, "g": 0, "b": 0})).is_err()
        );
        assert!(
            serde_json::from_value::<Rgb>(serde_json::json!({"r": "ff", "g": 0, "b": 0})).is_err()
        );
    }

    #[test]
    fn malformed_theme_reads_as_no_theme() {
        let publication: Publication = serde_json::from_value(serde_json::json!({
            "url": "https://x.example", "name": "X",
            "basicTheme": {"background": {"r": 999, "g": 0, "b": 0}}
        }))
        .unwrap();
        assert!(publication.basic_theme.is_none());
    }

    #[test]
    fn malformed_optional_updated_at_is_dropped() {
        let mut value = minimal(&Value::Null);
        value["updatedAt"] = Value::String("soon".into());
        let doc: Document = serde_json::from_value(value).unwrap();
        assert!(doc.updated_at.is_none());
    }

    #[test]
    fn missing_required_field_fails() {
        let mut value = minimal(&Value::Null);
        value.as_object_mut().unwrap().remove("publishedAt");
        assert!(serde_json::from_value::<Document>(value).is_err());
    }
}
