//! The subject document: a `site.standard.document` interpreted through
//! `at.eaten.subject`.
//!
//! Everything here is pure: given records, decide what they mean. Records
//! come from other people's servers, so every field is a claim.

use eaten_at_atproto::lexicon::{Document, Subject, SUBJECT_NSID};
use eaten_at_atproto::repo::Record;
use serde_json::Value;

/// `$type` of the markdown content union member we render.
const MARKPUB_MARKDOWN_TYPE: &str = "at.markpub.markdown";

/// A document that carries our subject.
#[derive(Debug, Clone)]
pub struct SubjectDocument {
    pub record: Record<Document>,
    pub subject: Subject,
}

impl SubjectDocument {
    /// Interpret a record as a subject document. `None` when its `links`
    /// carry no `at.eaten.subject`, which means it is some other kind
    /// of document and not ours to show.
    pub fn from_record(record: Record<Document>) -> Option<Self> {
        let subject = extract_subject(&record.value.links)?;
        Some(Self { record, subject })
    }

    pub fn document(&self) -> &Document {
        &self.record.value
    }

    pub fn rkey(&self) -> &str {
        self.record.rkey()
    }

    /// The body to render.
    pub fn body(&self) -> Body<'_> {
        body_of(&self.record.value)
    }
}

/// Find the first `at.eaten.subject` among a document's links.
///
/// Other link types are skipped, not rejected: a document can carry
/// several relationships and ours is one of them. An entry with our
/// `$type` that does not decode is treated as absent.
pub fn extract_subject(links: &[Value]) -> Option<Subject> {
    links
        .iter()
        .filter(|link| link.get("$type").and_then(Value::as_str) == Some(SUBJECT_NSID))
        .find_map(
            |link| match serde_json::from_value::<Subject>(link.clone()) {
                Ok(subject) => Some(subject),
                Err(err) => {
                    tracing::debug!(%err, "skipping malformed at.eaten.subject");
                    None
                }
            },
        )
}

/// What a document offers as its body, in order of preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Body<'a> {
    /// `content` is `at.markpub.markdown`; this is `text.markdown`.
    Markdown(&'a str),
    /// No markdown we understand, but `textContent` is present.
    Plain(&'a str),
    /// Nothing renderable.
    Empty,
}

/// Choose the body for a document. Only `content.text.markdown` is read
/// from markpub content; facets, lenses, and rendering rules are ignored
/// by design (plan §4.1). Empty strings count as absent.
pub fn body_of(doc: &Document) -> Body<'_> {
    let markdown = doc
        .content
        .as_ref()
        .filter(|c| c.get("$type").and_then(Value::as_str) == Some(MARKPUB_MARKDOWN_TYPE))
        .and_then(|c| c.get("text"))
        .and_then(|t| t.get("markdown"))
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty());
    if let Some(markdown) = markdown {
        return Body::Markdown(markdown);
    }
    match doc.text_content.as_deref().filter(|s| !s.trim().is_empty()) {
        Some(text) => Body::Plain(text),
        None => Body::Empty,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(value: &Value) -> Record<Document> {
        serde_json::from_value(serde_json::json!({
            "uri": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.document/3abc",
            "cid": "bafy",
            "value": value
        }))
        .unwrap()
    }

    fn doc(extra: &Value) -> Record<Document> {
        let mut base = serde_json::json!({
            "$type": "site.standard.document",
            "site": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/3pub",
            "title": "On Sample Subject",
            "publishedAt": "2026-09-07T00:00:00Z"
        });
        base.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        record(&base)
    }

    fn subject() -> Value {
        serde_json::json!({
            "$type": "at.eaten.subject",
            "title": "Sample Subject",
            "externalUrls": [
                {"url": "https://example.com/official", "service": "officialSite"},
                {"url": "https://example.com/buy", "service": "shop"}
            ]
        })
    }

    #[test]
    fn singular_link_is_a_subject_document() {
        let subject_doc =
            SubjectDocument::from_record(doc(&serde_json::json!({"links": subject()}))).unwrap();
        assert_eq!(subject_doc.subject.title, "Sample Subject");
        assert_eq!(subject_doc.subject.external_urls.len(), 2);
    }

    #[test]
    fn array_link_with_foreign_entries_finds_ours() {
        let links = serde_json::json!([
            {"$type": "com.example.link", "url": "https://elsewhere"},
            {"$type": "at.eaten.subject", "name": "no title"},
            subject()
        ]);
        let subject_doc =
            SubjectDocument::from_record(doc(&serde_json::json!({"links": links}))).unwrap();
        // The malformed entry (no title) is skipped; the good one is used.
        assert_eq!(subject_doc.subject.title, "Sample Subject");
    }

    #[test]
    fn document_without_subject_is_not_ours() {
        assert!(SubjectDocument::from_record(doc(&serde_json::json!({}))).is_none());
        let other = serde_json::json!({"links": {"$type": "com.example.link"}});
        assert!(SubjectDocument::from_record(doc(&other)).is_none());
    }

    #[test]
    fn body_prefers_markpub_markdown_and_ignores_facets() {
        let d = doc(&serde_json::json!({
            "content": {"$type": "at.markpub.markdown", "flavor": "gfm",
                        "text": {"$type": "at.markpub.text", "markdown": "# Hi\n\n**bold**",
                                 "facets": [{"index": {"byteStart": 0, "byteEnd": 2}}]},
                        "renderingRules": {"x": 1}},
            "textContent": "Hi bold"
        }));
        assert_eq!(body_of(&d.value), Body::Markdown("# Hi\n\n**bold**"));
    }

    #[test]
    fn body_falls_back_to_text_content() {
        let unknown = doc(&serde_json::json!({
            "content": {"$type": "com.example.blocks", "blocks": []},
            "textContent": "plain words"
        }));
        assert_eq!(body_of(&unknown.value), Body::Plain("plain words"));
        let only_text = doc(&serde_json::json!({"textContent": "plain words"}));
        assert_eq!(body_of(&only_text.value), Body::Plain("plain words"));
    }

    #[test]
    fn body_is_empty_when_nothing_usable() {
        let blank = doc(&serde_json::json!({
            "content": {"$type": "at.markpub.markdown", "text": {"markdown": "   "}},
            "textContent": ""
        }));
        assert_eq!(body_of(&blank.value), Body::Empty);
        let malformed =
            doc(&serde_json::json!({"content": {"$type": "at.markpub.markdown", "text": "oops"}}));
        assert_eq!(body_of(&malformed.value), Body::Empty);
    }
}
