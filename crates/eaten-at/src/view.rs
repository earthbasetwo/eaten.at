//! Map protocol records to the plain view structs the web crate renders.

use eaten_at_atproto::at_uri::AtUri;
use eaten_at_atproto::identity::{Did, Identity};
use eaten_at_atproto::lexicon::{Publication, ThemeBasic};
use eaten_at_atproto::repo::Record;
use eaten_at_web::components::{
    CommentView, CommentsView, Link, ListingItem, PublicationView, SubjectCard,
};
use eaten_at_web::markdown::{excerpt, EXCERPT_TARGET};
use eaten_at_web::meta::{Kind, PageMeta};
use eaten_at_web::theme::{Rgb, Theme};

use crate::state::AppState;

use crate::model::{Body, SubjectDocument};
use crate::paths;

/// Build the subject card for a document from its subject.
pub fn subject_card(did: &Did, subject_doc: &SubjectDocument) -> SubjectCard {
    let subject = &subject_doc.subject;
    SubjectCard {
        title: subject.title.clone(),
        cover_src: Some(paths::cover(did, subject_doc.rkey())),
        links: subject
            .external_urls
            .iter()
            .filter(|u| {
                url::Url::parse(&u.url).is_ok_and(|p| matches!(p.scheme(), "http" | "https"))
            })
            .map(|u| Link {
                label: u.label.clone().unwrap_or_else(|| link_label(u)),
                href: u.url.clone(),
            })
            .collect(),
    }
}

/// Label a link from its declared service when known, else the
/// URL's hostname (plan §7.2).
fn link_label(u: &eaten_at_atproto::lexicon::ExternalUrl) -> String {
    if let Some(service) = u.known_service() {
        return service.display_name().to_owned();
    }
    url::Url::parse(&u.url)
        .ok()
        .and_then(|p| {
            p.host_str()
                .map(|h| h.trim_start_matches("www.").to_owned())
        })
        .unwrap_or_else(|| u.url.clone())
}

/// The summary shown in listings and meta tags: the author's own
/// `description` when present, else derived from the body (plan §7.5).
pub fn summary(subject_doc: &SubjectDocument) -> String {
    if let Some(description) = subject_doc.document().description.as_deref() {
        let trimmed = description.trim();
        if !trimmed.is_empty() {
            return trimmed.to_owned();
        }
    }
    match subject_doc.body() {
        Body::Markdown(text) | Body::Plain(text) => excerpt(text, EXCERPT_TARGET),
        Body::Empty => String::new(),
    }
}

pub fn listing_item(did: &Did, pub_rkey: &str, subject_doc: &SubjectDocument) -> ListingItem {
    ListingItem {
        href: paths::document(did, pub_rkey, subject_doc.rkey()),
        title: subject_doc.document().title.clone(),
        subject_title: subject_doc.subject.title.clone(),
        published: date_only(subject_doc.document().published_at.as_str()),
        excerpt: summary(subject_doc),
        cover_src: Some(paths::cover(did, subject_doc.rkey())),
    }
}

/// The publication's theme, contrast-clamped, if it declares one.
pub fn theme(publication: &Publication) -> Option<Theme> {
    publication.basic_theme.as_ref().map(theme_from_basic)
}

fn theme_from_basic(basic: &ThemeBasic) -> Theme {
    let rgb = |c: &eaten_at_atproto::lexicon::site_standard::Rgb| Rgb::new(c.r, c.g, c.b);
    Theme {
        background: rgb(&basic.background),
        foreground: rgb(&basic.foreground),
        accent: rgb(&basic.accent),
        accent_foreground: rgb(&basic.accent_foreground),
    }
    .clamped()
}

pub fn publication_view(did: &Did, record: &Record<Publication>) -> PublicationView {
    PublicationView {
        href: paths::publication(did, record.rkey()),
        name: record.value.name.clone(),
        description: record.value.description.clone(),
        url: record.value.url.clone(),
    }
}

/// How an author is named in page chrome: the verified handle, shown as
/// one, or the DID when there is none.
pub fn author_label(identity: &Identity) -> String {
    identity
        .handle
        .as_ref()
        .map_or_else(|| identity.did.to_string(), |handle| format!("@{handle}"))
}

/// A publication's site as it is printed in chrome: the host and path
/// without the scheme or a trailing slash, so `https://alice.eaten.test/`
/// reads as `alice.eaten.test`.
pub fn display_url(url: &str) -> String {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_owned()
}

/// `2026-09-07T21:00:00.000Z` → `2026-09-07`. Readers see the date; the
/// full timestamp goes in `<time datetime>` and meta tags.
pub fn date_only(datetime: &str) -> String {
    datetime.split('T').next().unwrap_or(datetime).to_owned()
}

/// Tag links for a publication, one per distinct tag, labelled as typed.
pub fn tag_links(did: &Did, pub_rkey: &str, tags: &[String]) -> Vec<Link> {
    tags.iter()
        .map(|tag| Link {
            label: tag.clone(),
            href: paths::tagged(did, pub_rkey, tag),
        })
        .collect()
}

/// Canonical URL of a document: `publication.url + path`. A document
/// without a `path` has no address at its publication origin, so the
/// site route stands in.
pub fn canonical_url(
    state: &AppState,
    did: &Did,
    pub_rkey: &str,
    doc: &Record<eaten_at_atproto::lexicon::Document>,
    publication: &Publication,
) -> String {
    match doc.value.path.as_deref() {
        Some(path) => format!("{}{path}", publication.base_url()),
        None => state.absolute(&paths::document(did, pub_rkey, doc.rkey())),
    }
}

/// The Bluesky post a document names as its comment thread, as a page on
/// bsky.app. Only a post record qualifies; any other reference, or a
/// malformed one, means no link.
pub fn bluesky_post_url(doc: &eaten_at_atproto::lexicon::Document) -> Option<String> {
    let uri = bluesky_post_uri(doc)?;
    Some(crate::bsky::post_url(uri.did().as_str(), uri.rkey()))
}

/// The Bluesky post a document names as its comment thread, when the
/// reference is a post record.
pub fn bluesky_post_uri(doc: &eaten_at_atproto::lexicon::Document) -> Option<AtUri> {
    let uri = AtUri::parse(&doc.bsky_post_ref.as_ref()?.uri).ok()?;
    (uri.collection() == "app.bsky.feed.post").then_some(uri)
}

/// The thread as the page shows it.
pub fn comments_view(thread: &crate::bsky::Thread, thread_href: String) -> CommentsView {
    CommentsView {
        thread_href,
        comments: thread
            .comments
            .iter()
            .map(|c| CommentView {
                author_name: c.author.display_name.clone(),
                author_handle: c.author.handle.clone(),
                author_href: c.author.url(),
                href: c.url(),
                published: c.created_at.clone(),
                text: c.text.clone(),
                depth: c.depth,
            })
            .collect(),
        truncated: thread.truncated,
    }
}

/// The subject's title for a subject document, else the document title.
pub fn document_headline(subject_doc: Option<&SubjectDocument>, title: &str) -> String {
    subject_doc.map_or_else(
        || title.to_owned(),
        |subject_doc| subject_doc.subject.title.clone(),
    )
}

/// Head metadata for a document page.
pub fn document_meta(
    state: &AppState,
    did: &Did,
    pub_rkey: &str,
    doc: &Record<eaten_at_atproto::lexicon::Document>,
    subject_doc: Option<&SubjectDocument>,
    publication: &Publication,
) -> PageMeta {
    let description = match subject_doc {
        Some(subject_doc) => summary(subject_doc),
        None => doc.value.description.clone().unwrap_or_default(),
    };
    PageMeta {
        title: document_headline(subject_doc, &doc.value.title),
        description,
        canonical: canonical_url(state, did, pub_rkey, doc, publication),
        kind: Kind::Article,
        site_name: publication.name.clone(),
        image: state.absolute(&paths::cover_og(did, doc.rkey())),
        published: Some(doc.value.published_at.as_str().to_owned()),
        modified: doc.value.updated_at.as_ref().map(|d| d.as_str().to_owned()),
        feed: Some(state.absolute(&paths::feed(did, pub_rkey))),
    }
}

/// Head metadata for a publication page.
pub fn publication_meta(
    state: &AppState,
    did: &Did,
    pub_rkey: &str,
    publication: &Publication,
) -> PageMeta {
    PageMeta {
        title: publication.name.clone(),
        description: publication.description.clone().unwrap_or_default(),
        canonical: publication.base_url().to_owned(),
        kind: Kind::Website,
        site_name: publication.name.clone(),
        image: state.absolute(&paths::icon(did, pub_rkey)),
        published: None,
        modified: None,
        feed: Some(state.absolute(&paths::feed(did, pub_rkey))),
    }
}

#[cfg(test)]
mod tests {
    use super::bluesky_post_url;
    use eaten_at_atproto::lexicon::{Document, StrongRef};

    fn doc(uri: Option<&str>) -> Document {
        let mut doc: Document = serde_json::from_value(serde_json::json!({
            "site": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1",
            "title": "T",
            "publishedAt": "2026-09-07T12:00:00.000Z"
        }))
        .unwrap();
        doc.bsky_post_ref = uri.map(|uri| StrongRef {
            uri: uri.to_owned(),
            cid: "bafy".to_owned(),
        });
        doc
    }

    #[test]
    fn display_url_drops_scheme_and_slash() {
        assert_eq!(
            super::display_url("https://alice.eaten.test/"),
            "alice.eaten.test"
        );
        assert_eq!(super::display_url("http://x.test/blog"), "x.test/blog");
        assert_eq!(super::display_url("x.test"), "x.test");
    }

    #[test]
    fn bluesky_link_only_for_post_records() {
        assert_eq!(bluesky_post_url(&doc(None)), None);
        assert_eq!(
            bluesky_post_url(&doc(Some(
                "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/app.bsky.feed.post/3kx"
            ))),
            Some("https://bsky.app/profile/did:plc:re3ebnp5v7ffagz6rb6xfei4/post/3kx".into())
        );
        assert_eq!(
            bluesky_post_url(&doc(Some(
                "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/app.bsky.feed.like/3kx"
            ))),
            None
        );
        assert_eq!(bluesky_post_url(&doc(Some("https://bsky.app/x"))), None);
    }
}
