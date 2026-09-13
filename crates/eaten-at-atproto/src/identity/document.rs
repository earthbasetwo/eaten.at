//! The subset of a DID document that atproto cares about.

use serde::{Deserialize, Serialize};
use url::Url;

use super::did::Did;
use super::handle::Handle;

/// Service `id` fragment naming the personal data server.
const PDS_SERVICE_FRAGMENT: &str = "#atproto_pds";
/// Service `type` for a personal data server.
const PDS_SERVICE_TYPE: &str = "AtprotoPersonalDataServer";

/// A DID document as returned by PLC or a `did:web` host.
///
/// Only the fields the read path needs are modelled. Everything else is
/// accepted and ignored so future additions to the document format never
/// break resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DidDocument {
    /// The DID this document describes. Callers must check it matches the
    /// DID they asked for.
    pub id: Did,
    /// Aliases, as `at://<handle>` URIs. A handle is only valid if it also
    /// resolves back to this DID.
    #[serde(default, rename = "alsoKnownAs")]
    pub also_known_as: Vec<String>,
    /// Service endpoints. The PDS is the one we use.
    #[serde(default)]
    pub service: Vec<Service>,
}

/// One entry of a DID document's `service` array.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: String,
    #[serde(rename = "type")]
    pub kind: String,
    /// DID core allows a string, a map, or a set here. atproto always writes
    /// a string; anything else is treated as absent.
    #[serde(rename = "serviceEndpoint")]
    pub endpoint: serde_json::Value,
}

impl DidDocument {
    /// The personal data server endpoint, if the document declares one.
    ///
    /// Matches on both the short (`#atproto_pds`) and fully qualified
    /// (`did:...#atproto_pds`) forms of the service id, as both appear in the
    /// wild. The URL is returned as written; scheme policy is the resolver's
    /// job.
    pub fn pds_endpoint(&self) -> Option<Url> {
        self.service
            .iter()
            .filter(|s| s.kind == PDS_SERVICE_TYPE && s.id.ends_with(PDS_SERVICE_FRAGMENT))
            .find_map(|s| s.endpoint.as_str().and_then(|raw| Url::parse(raw).ok()))
    }

    /// The first `alsoKnownAs` entry that is a well-formed `at://` handle.
    ///
    /// This is the DID's *claimed* handle. It is only trustworthy once the
    /// handle has been resolved forward to the same DID.
    pub fn claimed_handle(&self) -> Option<Handle> {
        self.also_known_as
            .iter()
            .filter_map(|aka| aka.strip_prefix("at://"))
            .find_map(|raw| Handle::parse(raw).ok())
    }

    /// Whether the document lists `handle` among its aliases.
    pub fn claims_handle(&self, handle: &Handle) -> bool {
        self.also_known_as
            .iter()
            .filter_map(|aka| aka.strip_prefix("at://"))
            .any(|raw| Handle::parse(raw).as_ref() == Ok(handle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLC_DOC: &str = r##"{
      "@context": ["https://www.w3.org/ns/did/v1"],
      "id": "did:plc:re3ebnp5v7ffagz6rb6xfei4",
      "alsoKnownAs": ["at://standard.site"],
      "verificationMethod": [{"id": "did:plc:re3ebnp5v7ffagz6rb6xfei4#atproto", "type": "Multikey"}],
      "service": [
        {"id": "#atproto_labeler", "type": "AtprotoLabeler", "serviceEndpoint": "https://labeler.example"},
        {"id": "#atproto_pds", "type": "AtprotoPersonalDataServer", "serviceEndpoint": "https://pds.example.com"}
      ]
    }"##;

    #[test]
    fn extracts_pds_and_handle() {
        let doc: DidDocument = serde_json::from_str(PLC_DOC).unwrap();
        assert_eq!(
            doc.pds_endpoint().unwrap().as_str(),
            "https://pds.example.com/"
        );
        let handle = doc.claimed_handle().unwrap();
        assert_eq!(handle.as_str(), "standard.site");
        assert!(doc.claims_handle(&handle));
        assert!(!doc.claims_handle(&Handle::parse("other.test").unwrap()));
    }

    #[test]
    fn accepts_fully_qualified_service_id() {
        let doc: DidDocument = serde_json::from_str(
            r#"{"id": "did:web:example.com",
                "service": [{"id": "did:web:example.com#atproto_pds",
                             "type": "AtprotoPersonalDataServer",
                             "serviceEndpoint": "https://pds.example.com"}]}"#,
        )
        .unwrap();
        assert!(doc.pds_endpoint().is_some());
        assert!(doc.claimed_handle().is_none());
    }

    #[test]
    fn ignores_wrong_type_non_string_endpoint_and_bad_aliases() {
        let doc: DidDocument = serde_json::from_str(
            r##"{"id": "did:web:example.com",
                "alsoKnownAs": ["https://not-a-handle", "at://Not Valid", "at://ok.example.com"],
                "service": [
                  {"id": "#atproto_pds", "type": "SomethingElse", "serviceEndpoint": "https://a.example"},
                  {"id": "#atproto_pds", "type": "AtprotoPersonalDataServer", "serviceEndpoint": {"uri": "x"}},
                  {"id": "#atproto_pds", "type": "AtprotoPersonalDataServer", "serviceEndpoint": "not a url"}
                ]}"##,
        )
        .unwrap();
        assert!(doc.pds_endpoint().is_none());
        assert_eq!(doc.claimed_handle().unwrap().as_str(), "ok.example.com");
    }

    #[test]
    fn rejects_document_with_invalid_id() {
        assert!(serde_json::from_str::<DidDocument>(r#"{"id": "did:key:zzz"}"#).is_err());
    }
}
