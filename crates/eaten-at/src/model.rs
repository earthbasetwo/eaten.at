//! The visit document: a `site.standard.document` whose `content` is an
//! `at.eaten.visit`.
//!
//! Everything here is pure: given records, decide what they mean. Records
//! come from other people's servers, so every field is a claim.

use eaten_at_atproto::lexicon::{Document, Visit, VISIT_NSID};
use eaten_at_atproto::repo::Record;
use serde_json::Value;

/// `$type` of the markdown body union member we render.
const MARKPUB_MARKDOWN_TYPE: &str = "at.markpub.markdown";

/// A document whose content is one of our visits.
#[derive(Debug, Clone)]
pub struct VisitDocument {
    pub record: Record<Document>,
    pub visit: Visit,
}

impl VisitDocument {
    /// Interpret a record as a visit document. `None` when its `content`
    /// is not an `at.eaten.visit`, which means it is some other kind of
    /// document and not ours to show.
    pub fn from_record(record: Record<Document>) -> Option<Self> {
        let visit = extract_visit(record.value.content.as_ref())?;
        Some(Self { record, visit })
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

/// Whether a content value declares our type.
fn is_visit(content: &Value) -> bool {
    content.get("$type").and_then(Value::as_str) == Some(VISIT_NSID)
}

/// Decode a document's `content` as a visit. Content of another type is
/// not ours; content with our `$type` that does not decode is treated as
/// absent, with a log line, rather than failing the page.
pub fn extract_visit(content: Option<&Value>) -> Option<Visit> {
    let content = content.filter(|c| is_visit(c))?;
    match serde_json::from_value::<Visit>(content.clone()) {
        Ok(visit) => Some(visit),
        Err(err) => {
            tracing::debug!(%err, "skipping malformed at.eaten.visit");
            None
        }
    }
}

/// What a document offers as its body, in order of preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Body<'a> {
    /// The body is `at.markpub.markdown`; this is `text.markdown`.
    Markdown(&'a str),
    /// No markdown we understand, but `textContent` is present.
    Plain(&'a str),
    /// Nothing renderable.
    Empty,
}

/// Choose the body for a document. For a visit the prose is the visit's
/// `body`; for any other document it is `content` itself. Only
/// `text.markdown` is read from markpub content; facets, lenses, and
/// rendering rules are ignored by design (plan §4.1). Empty strings count
/// as absent.
pub fn body_of(doc: &Document) -> Body<'_> {
    let prose = doc.content.as_ref().and_then(|content| {
        if is_visit(content) {
            content.get("body")
        } else {
            Some(content)
        }
    });
    let markdown = prose
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
            "title": "A night at Sample Place",
            "publishedAt": "2026-09-07T00:00:00Z"
        });
        base.as_object_mut()
            .unwrap()
            .extend(extra.as_object().unwrap().clone());
        record(&base)
    }

    fn markdown(text: &str) -> Value {
        serde_json::json!({
            "$type": "at.markpub.markdown", "flavor": "commonmark",
            "text": {"$type": "at.markpub.text", "markdown": text}
        })
    }

    fn visit(body: Option<Value>) -> Value {
        let mut v = serde_json::json!({
            "$type": "at.eaten.visit",
            "place": {"name": "Sample Place", "gersId": "g1"},
            "visitedOn": "2026-09-06",
            "rating": 2
        });
        if let Some(body) = body {
            v["body"] = body;
        }
        v
    }

    #[test]
    fn a_document_with_visit_content_is_ours() {
        let visit_doc = VisitDocument::from_record(doc(
            &serde_json::json!({"content": visit(Some(markdown("# Hi")))}),
        ))
        .unwrap();
        assert_eq!(visit_doc.visit.place.name, "Sample Place");
        assert_eq!(visit_doc.visit.visited_on.as_string(), "2026-09-06");
        assert_eq!(visit_doc.body(), Body::Markdown("# Hi"));
    }

    #[test]
    fn other_content_or_a_malformed_visit_is_not_ours() {
        assert!(VisitDocument::from_record(doc(&serde_json::json!({}))).is_none());
        let markpub = serde_json::json!({"content": markdown("plain post")});
        assert!(VisitDocument::from_record(doc(&markpub)).is_none());
        let malformed = serde_json::json!({
            "content": {"$type": "at.eaten.visit", "place": {"name": "P"}}
        });
        assert!(VisitDocument::from_record(doc(&malformed)).is_none());
        // A stray visit in `links` is not content and does not count.
        let in_links = serde_json::json!({"links": visit(None)});
        assert!(VisitDocument::from_record(doc(&in_links)).is_none());
    }

    #[test]
    fn body_prefers_markpub_markdown_and_ignores_facets() {
        let d = doc(&serde_json::json!({
            "content": visit(Some(serde_json::json!({
                "$type": "at.markpub.markdown", "flavor": "gfm",
                "text": {"$type": "at.markpub.text", "markdown": "# Hi\n\n**bold**",
                         "facets": [{"index": {"byteStart": 0, "byteEnd": 2}}]},
                "renderingRules": {"x": 1}
            }))),
            "textContent": "Hi bold"
        }));
        assert_eq!(body_of(&d.value), Body::Markdown("# Hi\n\n**bold**"));
        // A plain Standard document's markpub content still renders.
        let plain = doc(&serde_json::json!({"content": markdown("Just a post.")}));
        assert_eq!(body_of(&plain.value), Body::Markdown("Just a post."));
    }

    #[test]
    fn body_falls_back_to_text_content() {
        let unknown_body = doc(&serde_json::json!({
            "content": visit(Some(serde_json::json!({"$type": "com.example.blocks", "blocks": []}))),
            "textContent": "plain words"
        }));
        assert_eq!(body_of(&unknown_body.value), Body::Plain("plain words"));
        let no_body = doc(&serde_json::json!({
            "content": visit(None),
            "textContent": "plain words"
        }));
        assert_eq!(body_of(&no_body.value), Body::Plain("plain words"));
        let only_text = doc(&serde_json::json!({"textContent": "plain words"}));
        assert_eq!(body_of(&only_text.value), Body::Plain("plain words"));
    }

    #[test]
    fn body_is_empty_when_nothing_usable() {
        let blank = doc(&serde_json::json!({
            "content": visit(Some(markdown("   "))),
            "textContent": ""
        }));
        assert_eq!(body_of(&blank.value), Body::Empty);
        let malformed = doc(&serde_json::json!({
            "content": visit(Some(serde_json::json!({"$type": "at.markpub.markdown", "text": "oops"})))
        }));
        assert_eq!(body_of(&malformed.value), Body::Empty);
    }
}
