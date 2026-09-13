//! Lexicon schema files: the JSON we publish as `com.atproto.lexicon.schema`
//! records, validated just enough to catch the mistakes that matter.

use serde_json::{Map, Value};

/// Collection that holds published schemas.
pub const SCHEMA_COLLECTION: &str = "com.atproto.lexicon.schema";

/// A lexicon document loaded from disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaFile {
    /// The NSID, which is also the record key.
    pub id: String,
    /// The document as it will be written, minus `$type`.
    pub json: Map<String, Value>,
}

/// Why a schema file was rejected.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SchemaError {
    #[error("not valid JSON: {0}")]
    Json(String),
    #[error("top level is not an object")]
    NotObject,
    #[error("`lexicon` must be the integer 1")]
    Version,
    #[error("`id` is missing or not a string")]
    MissingId,
    #[error("`id` {0:?} is not a valid NSID")]
    BadId(String),
    #[error("`id` {id:?} does not match the file name {expected:?}")]
    IdMismatch { id: String, expected: String },
    #[error("`defs` is missing, not an object, or empty")]
    Defs,
}

impl SchemaFile {
    /// Parse and validate. `expected_id` is the NSID implied by the file
    /// name; the document's `id` must equal it so a renamed file cannot
    /// silently publish under the wrong key.
    pub fn parse(text: &str, expected_id: &str) -> Result<Self, SchemaError> {
        let json: Value =
            serde_json::from_str(text).map_err(|e| SchemaError::Json(e.to_string()))?;
        let Value::Object(mut object) = json else {
            return Err(SchemaError::NotObject);
        };
        if object.get("lexicon").and_then(Value::as_u64) != Some(1) {
            return Err(SchemaError::Version);
        }
        let id = object
            .get("id")
            .and_then(Value::as_str)
            .ok_or(SchemaError::MissingId)?
            .to_owned();
        if !is_nsid(&id) {
            return Err(SchemaError::BadId(id));
        }
        if id != expected_id {
            return Err(SchemaError::IdMismatch {
                id,
                expected: expected_id.to_owned(),
            });
        }
        // `main` is optional: a lexicon may hold only named defs, as
        // `site.standard.theme.color` does.
        let defs_ok = object
            .get("defs")
            .and_then(Value::as_object)
            .is_some_and(|d| !d.is_empty());
        if !defs_ok {
            return Err(SchemaError::Defs);
        }
        object.remove("$type");
        Ok(Self { id, json: object })
    }

    /// The NSID authority: every segment but the last, as written.
    pub fn authority(&self) -> &str {
        self.id
            .rsplit_once('.')
            .map_or(self.id.as_str(), |(a, _)| a)
    }

    /// The record value to publish: the document plus its `$type`.
    pub fn record_value(&self) -> Value {
        let mut object = self.json.clone();
        object.insert("$type".into(), Value::String(SCHEMA_COLLECTION.into()));
        Value::Object(object)
    }

    /// The document as a JSON value, for comparison with a fetched record.
    pub fn value(&self) -> Value {
        Value::Object(self.json.clone())
    }
}

/// Domain implied by an NSID's authority: `at.eaten` → `eaten.at`.
pub fn authority_domain(nsid: &str) -> Option<String> {
    let (authority, _) = nsid.rsplit_once('.')?;
    let mut labels: Vec<&str> = authority.split('.').collect();
    if labels.len() < 2 {
        return None;
    }
    labels.reverse();
    Some(labels.join("."))
}

/// NSID syntax: at least three dot-separated segments; all but the last are
/// DNS-like labels, the last is a name in `[A-Za-z][A-Za-z0-9]*`.
pub fn is_nsid(s: &str) -> bool {
    let segments: Vec<&str> = s.split('.').collect();
    if segments.len() < 3 || s.len() > 317 {
        return false;
    }
    let Some((name, authority)) = segments.split_last() else {
        return false;
    };
    let label_ok = |l: &&str| {
        !l.is_empty()
            && l.len() <= 63
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    authority.iter().all(label_ok)
        && name.bytes().next().is_some_and(|b| b.is_ascii_alphabetic())
        && name.bytes().all(|b| b.is_ascii_alphanumeric())
}

#[cfg(test)]
mod tests {
    use super::*;

    const VISIT: &str = include_str!("../../../../lexicons/at.eaten.visit.json");

    #[test]
    fn loads_our_schema_and_computes_authority() {
        let schema = SchemaFile::parse(VISIT, "at.eaten.visit").unwrap();
        assert_eq!(schema.id, "at.eaten.visit");
        assert_eq!(schema.authority(), "at.eaten");
        assert_eq!(authority_domain(&schema.id).as_deref(), Some("eaten.at"));
        assert_eq!(schema.record_value()["$type"], SCHEMA_COLLECTION);
        assert!(!schema.json.contains_key("$type"));
    }

    #[test]
    fn rejects_bad_files() {
        assert!(matches!(
            SchemaFile::parse("nope", "a.b.c"),
            Err(SchemaError::Json(_))
        ));
        assert!(matches!(
            SchemaFile::parse("[]", "a.b.c"),
            Err(SchemaError::NotObject)
        ));
        assert!(matches!(
            SchemaFile::parse(
                r#"{"lexicon": 2, "id": "a.b.c", "defs": {"main": {}}}"#,
                "a.b.c"
            ),
            Err(SchemaError::Version)
        ));
        assert!(matches!(
            SchemaFile::parse(r#"{"lexicon": 1, "defs": {"main": {}}}"#, "a.b.c"),
            Err(SchemaError::MissingId)
        ));
        assert!(matches!(
            SchemaFile::parse(
                r#"{"lexicon": 1, "id": "not an nsid", "defs": {"main": {}}}"#,
                "a.b.c"
            ),
            Err(SchemaError::BadId(_))
        ));
        assert!(matches!(
            SchemaFile::parse(
                r#"{"lexicon": 1, "id": "a.b.d", "defs": {"main": {}}}"#,
                "a.b.c"
            ),
            Err(SchemaError::IdMismatch { .. })
        ));
        assert!(matches!(
            SchemaFile::parse(r#"{"lexicon": 1, "id": "a.b.c", "defs": {}}"#, "a.b.c"),
            Err(SchemaError::Defs)
        ));
        // Named defs without `main` are valid.
        assert!(SchemaFile::parse(
            r#"{"lexicon": 1, "id": "a.b.c", "defs": {"rgb": {"type": "object"}}}"#,
            "a.b.c"
        )
        .is_ok());
    }

    #[test]
    fn nsid_syntax() {
        assert!(is_nsid("at.eaten.visit"));
        assert!(is_nsid("com.atproto.lexicon.schema"));
        assert!(!is_nsid("eaten.visit"));
        assert!(!is_nsid("at.eaten.1visit"));
        assert!(!is_nsid("at.-eaten.visit"));
        assert!(!is_nsid("at.eaten.vi-sit"));
        assert_eq!(
            authority_domain("site.standard.document").as_deref(),
            Some("standard.site")
        );
        assert_eq!(authority_domain("a.b"), None);
    }
}
