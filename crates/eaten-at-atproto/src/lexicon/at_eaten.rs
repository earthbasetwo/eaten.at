//! Our own lexicons: `at.eaten.*`.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::common::Datetime;
use crate::at_uri::AtUri;

/// NSID of the subject object placed in `document.links`.
pub const SUBJECT_NSID: &str = "at.eaten.subject";

/// `at.eaten.subject`: what a document is about. Its presence in
/// `document.links` is what makes a document one of ours.
///
/// Strings are stored as written; the app validates at the point of use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Subject {
    #[serde(rename = "$type", default, skip_serializing_if = "Option::is_none")]
    pub type_: Option<String>,
    /// The subject's name, as the author gives it.
    pub title: String,
    /// Places to read more about the subject, in the author's order.
    #[serde(
        rename = "externalUrls",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub external_urls: Vec<ExternalUrl>,
    #[serde(flatten)]
    pub extra: serde_json::Map<String, Value>,
}

/// `at.eaten.subject#externalUrl`.
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

/// The `knownValues` of `externalUrl.service`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KnownService {
    OfficialSite,
}

impl KnownService {
    /// Every known value, in lexicon order.
    pub const ALL: [Self; 1] = [Self::OfficialSite];

    /// Exact, case-sensitive match against the lexicon's `knownValues`.
    pub fn from_value(value: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.as_str() == value)
    }

    /// The value as written in records.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OfficialSite => "officialSite",
        }
    }

    /// Label for a link.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::OfficialSite => "Official site",
        }
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

    #[test]
    fn subject_round_trips_preserving_unknown_service_and_fields() {
        let json = serde_json::json!({
            "$type": "at.eaten.subject",
            "title": "Sample Subject",
            "externalUrls": [
                {"url": "https://example.com/elsewhere", "service": "bc"},
                {"url": "https://example.com", "service": "officialSite", "label": "Home"}
            ],
            "future": true
        });
        let subject: Subject = serde_json::from_value(json.clone()).unwrap();
        assert_eq!(subject.external_urls[0].known_service(), None);
        assert_eq!(
            subject.external_urls[1].known_service(),
            Some(KnownService::OfficialSite)
        );
        assert_eq!(serde_json::to_value(&subject).unwrap(), json);
    }

    #[test]
    fn known_service_is_exact_match() {
        assert_eq!(
            KnownService::from_value("officialSite"),
            Some(KnownService::OfficialSite)
        );
        assert_eq!(KnownService::from_value("OfficialSite"), None);
        assert_eq!(KnownService::from_value("site"), None);
        for s in KnownService::ALL {
            assert_eq!(KnownService::from_value(s.as_str()), Some(s));
        }
    }

    #[test]
    fn subject_requires_title() {
        assert!(serde_json::from_value::<Subject>(serde_json::json!({})).is_err());
        let s: Subject = serde_json::from_value(serde_json::json!({"title": "T"})).unwrap();
        assert!(s.external_urls.is_empty());
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
