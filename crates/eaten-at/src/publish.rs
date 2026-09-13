//! From a [`DocumentDraft`] to records in the author's repository
//! (plan §5.4): the publication and preferences a first publish needs,
//! the cover blob, the document itself, and the cache entries that must
//! forget the old state afterwards.

use eaten_at_atproto::at_uri::AtUri;
use eaten_at_atproto::identity::{Did, Identity};
use eaten_at_atproto::lexicon::at_eaten::{PREFERENCES_NSID, PREFERENCES_RKEY};
use eaten_at_atproto::lexicon::{
    BlobRef, Datetime, Document, DOCUMENT_NSID, PUBLICATION_NSID, SUBJECT_NSID,
};
use eaten_at_atproto::oauth::{AuthorizedSession, OAuthError};
use eaten_at_atproto::repo::write::WriteReceipt;
use eaten_at_atproto::repo::Record;
use eaten_at_web::markdown;
use serde_json::{json, Value};
use unicode_normalization::UnicodeNormalization;

use crate::cache::Namespace;
use crate::editor::{DocumentDraft, Target};
use crate::error::AppError;
use crate::img::{Size, MAX_COVER_BYTES};
use crate::model::SubjectDocument;
use crate::state::AppState;
use crate::view;

/// NSID of a Bluesky post.
pub const POST_NSID: &str = "app.bsky.feed.post";
/// Longest post text, in graphemes: the Bluesky lexicon's limit.
pub const MAX_POST_GRAPHEMES: usize = 300;

/// Longest slug, in bytes, before it is cut at a word boundary.
const MAX_SLUG_BYTES: usize = 80;
/// How many pages of the repo's documents are read to keep a new path
/// unique. Past this a collision is possible but the PDS keys by TID, so
/// nothing is overwritten; the address would just be shared.
const MAX_PATH_PAGES: usize = 25;

/// Why a publish did not happen.
#[derive(Debug, thiserror::Error)]
pub enum PublishError {
    #[error("the sign-in has expired")]
    SessionExpired,
    #[error("the author's server refused the write: {0}")]
    Repo(OAuthError),
    #[error(transparent)]
    App(#[from] AppError),
}

impl From<OAuthError> for PublishError {
    fn from(err: OAuthError) -> Self {
        match err {
            OAuthError::NoSession(_) | OAuthError::Unauthenticated => Self::SessionExpired,
            other => Self::Repo(other),
        }
    }
}

/// Where a published document lives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    pub did: Did,
    pub pub_rkey: String,
    pub doc_rkey: String,
}

/// A URL slug from a title: lowercase, accents stripped, anything that is
/// not a letter or digit collapsed to one hyphen. Letters outside ASCII
/// are kept; `untitled` stands in for a title with none.
pub fn slug(title: &str) -> String {
    let mut out = String::new();
    let mut pending_hyphen = false;
    for c in title.nfkd().filter(|c| !is_combining_mark(*c)) {
        if c.is_alphanumeric() {
            if pending_hyphen && !out.is_empty() {
                out.push('-');
            }
            pending_hyphen = false;
            out.extend(c.to_lowercase());
        } else {
            pending_hyphen = true;
        }
    }
    if out.len() > MAX_SLUG_BYTES {
        let cut = out[..MAX_SLUG_BYTES]
            .rfind('-')
            .unwrap_or(MAX_SLUG_BYTES.min(out.len()));
        let mut end = cut;
        while !out.is_char_boundary(end) {
            end -= 1;
        }
        out.truncate(end);
    }
    let out = out.trim_matches('-').to_owned();
    if out.is_empty() {
        "untitled".to_owned()
    } else {
        out
    }
}

fn is_combining_mark(c: char) -> bool {
    matches!(u32::from(c), 0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

/// `/YYYY/MM/slug`, with `-2`, `-3`, … appended until it is not among
/// `taken`.
pub fn document_path(published_at: &Datetime, title: &str, taken: &[String]) -> String {
    let date = published_at.timestamp().to_zoned(jiff::tz::TimeZone::UTC);
    let base = format!("/{:04}/{:02}/{}", date.year(), date.month(), slug(title));
    if !taken.contains(&base) {
        return base;
    }
    let mut n = 2u32;
    loop {
        let candidate = format!("{base}-{n}");
        if !taken.contains(&candidate) {
            return candidate;
        }
        n += 1;
    }
}

/// Everything that fixes the record apart from the draft.
#[derive(Debug, Clone)]
pub struct Placement<'a> {
    pub site: &'a AtUri,
    pub path: &'a str,
    pub published_at: &'a Datetime,
    /// Set on edits only.
    pub updated_at: Option<&'a Datetime>,
    /// The cover to reference; `None` removes any.
    pub cover: Option<&'a BlobRef>,
    /// The record being replaced, whose unknown fields are kept.
    pub original: Option<&'a Document>,
}

/// The `site.standard.document` record for a draft, exactly as written
/// (plan §4.1, §5.4). Fields our editor does not know about survive from
/// the original; `description` is present only when the author wrote
/// one; `links` is a single object unless the original carried other
/// members too.
pub fn build_document(draft: &DocumentDraft, placement: &Placement<'_>) -> Value {
    let mut doc = placement.original.map_or_else(
        || json!({}),
        |d| serde_json::to_value(d).unwrap_or_default(),
    );
    let Value::Object(fields) = &mut doc else {
        unreachable!("a document serializes to an object")
    };
    fields.insert("$type".into(), json!(DOCUMENT_NSID));
    fields.insert("site".into(), json!(placement.site.as_str()));
    fields.insert("title".into(), json!(draft.title));
    fields.insert("path".into(), json!(placement.path));
    fields.insert("publishedAt".into(), json!(placement.published_at));
    match placement.updated_at {
        Some(at) => {
            fields.insert("updatedAt".into(), json!(at));
        }
        None => {
            fields.remove("updatedAt");
        }
    }
    match &draft.description {
        Some(description) => {
            fields.insert("description".into(), json!(description));
        }
        None => {
            fields.remove("description");
        }
    }
    match placement.cover {
        Some(cover) => {
            fields.insert("coverImage".into(), json!(cover));
        }
        None => {
            fields.remove("coverImage");
        }
    }
    fields.insert(
        "content".into(),
        json!({
            "$type": "at.markpub.markdown",
            "flavor": "commonmark",
            "text": { "$type": "at.markpub.text", "markdown": draft.markdown },
        }),
    );
    fields.insert(
        "textContent".into(),
        json!(markdown::to_plaintext(&draft.markdown)),
    );
    if draft.tags.is_empty() {
        fields.remove("tags");
    } else {
        fields.insert("tags".into(), json!(draft.tags));
    }

    let subject = serde_json::to_value(&draft.subject).unwrap_or_default();
    let mut links: Vec<Value> = placement
        .original
        .map(|d| d.links.clone())
        .unwrap_or_default();
    match links
        .iter_mut()
        .find(|l| l.get("$type").and_then(Value::as_str) == Some(SUBJECT_NSID))
    {
        Some(ours) => *ours = subject,
        None => links.push(subject),
    }
    fields.insert(
        "links".into(),
        if links.len() == 1 {
            links.remove(0)
        } else {
            Value::Array(links)
        },
    );
    doc
}

/// Publish a draft: create what a first publish needs, upload the cover,
/// write the document, and forget the cached state it changes.
pub async fn publish(
    state: &AppState,
    identity: &Identity,
    draft: &DocumentDraft,
    editing: Option<&SubjectDocument>,
) -> Result<Published, PublishError> {
    let did = &identity.did;
    let session = state.oauth().session(did).await?;
    let now = Datetime::now();

    let site = match &draft.target {
        Target::Existing(uri) => uri.clone(),
        Target::New { name, url } => {
            let receipt = session
                .create_record(
                    PUBLICATION_NSID,
                    None,
                    &json!({
                        "$type": PUBLICATION_NSID,
                        "url": url.trim_end_matches('/'),
                        "name": name,
                    }),
                )
                .await?;
            let uri = AtUri::parse(&receipt.uri).map_err(|e| {
                PublishError::Repo(OAuthError::Transport(format!(
                    "PDS returned an unusable record URI: {e}"
                )))
            })?;
            state
                .cache()
                .evict(Namespace::Publication, &format!("list:{did}"))
                .await;
            uri
        }
    };

    update_preferences(
        state,
        identity,
        &session,
        &site,
        draft.crosspost.is_some(),
        &now,
    )
    .await?;

    let uploaded = match &draft.cover {
        Some(cover) => Some(session.upload_blob(cover.bytes.clone(), cover.mime).await?),
        None => None,
    };
    let original = editing.map(SubjectDocument::document);
    let cover = uploaded
        .as_ref()
        .or_else(|| original.and_then(|d| d.cover_image.as_ref()));

    let path = if let Some(path) = original.and_then(|d| d.path.clone()) {
        path
    } else {
        let taken = taken_paths(state, identity, &site).await?;
        document_path(&now, &draft.title, &taken)
    };
    let published_at = original.map_or_else(|| now.clone(), |d| d.published_at.clone());
    let record = build_document(
        draft,
        &Placement {
            site: &site,
            path: &path,
            published_at: &published_at,
            updated_at: editing.map(|_| &now),
            cover,
            original,
        },
    );

    let doc_rkey = if let Some(existing) = editing {
        session
            .put_record(DOCUMENT_NSID, existing.rkey(), &record)
            .await?;
        existing.rkey().to_owned()
    } else {
        let receipt = session.create_record(DOCUMENT_NSID, None, &record).await?;
        AtUri::parse(&receipt.uri)
            .map(|uri| uri.rkey().to_owned())
            .map_err(|e| {
                PublishError::Repo(OAuthError::Transport(format!(
                    "PDS returned an unusable record URI: {e}"
                )))
            })?
    };
    forget_document(state, did, &doc_rkey).await;
    Ok(Published {
        did: did.clone(),
        pub_rkey: site.rkey().to_owned(),
        doc_rkey,
    })
}

/// Preferences are written once on a first publish (the default
/// publication), and again only when the crosspost choice changed, so
/// the editor's toggle remembers it (plan §4.3). An author who never
/// crossposts never gets the field.
async fn update_preferences(
    state: &AppState,
    identity: &Identity,
    session: &AuthorizedSession,
    site: &AtUri,
    crosspost: bool,
    now: &Datetime,
) -> Result<(), PublishError> {
    let mut preferences = state.preferences(identity).await?;
    let absent = preferences.created_at.is_none() && preferences.default_publication.is_none();
    if !absent && preferences.crosspost_default() == crosspost {
        return Ok(());
    }
    preferences.type_ = Some(PREFERENCES_NSID.to_owned());
    if absent {
        preferences.default_publication = Some(site.clone());
        preferences.created_at = Some(now.clone());
    }
    if crosspost || preferences.crosspost_to_bluesky.is_some() {
        preferences.crosspost_to_bluesky = Some(crosspost);
    }
    session
        .put_record(
            PREFERENCES_NSID,
            PREFERENCES_RKEY,
            &serde_json::to_value(&preferences).unwrap_or_default(),
        )
        .await?;
    state
        .cache()
        .evict(Namespace::Publication, &format!("prefs:{}", identity.did))
        .await;
    Ok(())
}

/// Delete a document and forget it. With `post`, the Bluesky post the
/// document names is deleted first; if that is refused nothing else is
/// touched, so the author is never left guessing what went (plan §5.7).
pub async fn delete(
    state: &AppState,
    identity: &Identity,
    rkey: &str,
    post: Option<&AtUri>,
) -> Result<(), PublishError> {
    let session = state.oauth().session(&identity.did).await?;
    if let Some(post) = post {
        session.delete_record(POST_NSID, post.rkey()).await?;
    }
    session.delete_record(DOCUMENT_NSID, rkey).await?;
    forget_document(state, &identity.did, rkey).await;
    Ok(())
}

/// The link card a crosspost carries: the write-up as an
/// `app.bsky.embed.external`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkCard {
    /// The canonical URL of the document.
    pub url: String,
    pub title: String,
    pub description: String,
    /// The cover, as a blob in the author's repo.
    pub thumb: Option<BlobRef>,
}

/// The `app.bsky.feed.post` record for a crosspost (plan §5.7): the text
/// as the author left it, and the write-up as a link card. No facets: the
/// card carries the link.
pub fn build_post(text: &str, card: &LinkCard, created_at: &Datetime) -> Value {
    let mut external = json!({
        "uri": card.url,
        "title": card.title,
        "description": card.description,
    });
    if let Some(thumb) = &card.thumb {
        external["thumb"] = json!(thumb);
    }
    json!({
        "$type": POST_NSID,
        "text": text,
        "createdAt": created_at,
        "embed": {
            "$type": "app.bsky.embed.external",
            "external": external,
        },
    })
}

/// The document record with `bskyPostRef` set and nothing else changed.
/// `updatedAt` in particular stays as it was: the author edited nothing
/// (plan §5.7).
pub fn with_post_ref(doc: &Document, post: &WriteReceipt) -> Value {
    let mut value = serde_json::to_value(doc).unwrap_or_default();
    value["$type"] = json!(DOCUMENT_NSID);
    value["bskyPostRef"] = json!({ "uri": post.uri, "cid": post.cid });
    value
}

/// How a crosspost ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Crossposted {
    /// A post was created (or found from an earlier attempt) and the
    /// document now names it.
    Posted(String),
    /// The document already named a post; nothing was written.
    Already(String),
}

impl Crossposted {
    /// The post's AT-URI either way.
    pub fn uri(&self) -> &str {
        match self {
            Self::Posted(uri) | Self::Already(uri) => uri,
        }
    }
}

/// Post a published document to Bluesky and point the document at the
/// post (plan §5.7, steps 2 and 3).
///
/// Idempotent by construction: the document is re-read first and an
/// existing `bskyPostRef` ends the call; and the post's record key is the
/// document's own, so a retry after a failed step 3 finds the post that
/// step 2 already created instead of making a second one.
pub async fn crosspost(
    state: &AppState,
    identity: &Identity,
    rkey: &str,
    text: &str,
) -> Result<Crossposted, PublishError> {
    let did = &identity.did;
    let session = state.oauth().session(did).await?;
    let record: Record<Document> = session
        .query(
            "com.atproto.repo.getRecord",
            &[
                ("repo", did.as_str()),
                ("collection", DOCUMENT_NSID),
                ("rkey", rkey),
            ],
        )
        .await?;
    if let Some(existing) = &record.value.bsky_post_ref {
        return Ok(Crossposted::Already(existing.uri.clone()));
    }
    let card = link_card(state, identity, &session, &record).await?;
    let post = match existing_post(&session, did, rkey).await? {
        Some(found) => found,
        None => {
            session
                .create_record(
                    POST_NSID,
                    Some(rkey),
                    &build_post(text, &card, &Datetime::now()),
                )
                .await?
        }
    };
    session
        .put_record(DOCUMENT_NSID, rkey, &with_post_ref(&record.value, &post))
        .await?;
    forget_document(state, did, rkey).await;
    Ok(Crossposted::Posted(post.uri))
}

/// The post an earlier attempt may have left at the document's key.
async fn existing_post(
    session: &AuthorizedSession,
    did: &Did,
    rkey: &str,
) -> Result<Option<WriteReceipt>, PublishError> {
    let found: Result<Record<Value>, OAuthError> = session
        .query(
            "com.atproto.repo.getRecord",
            &[
                ("repo", did.as_str()),
                ("collection", POST_NSID),
                ("rkey", rkey),
            ],
        )
        .await;
    match found {
        Ok(record) => Ok(Some(WriteReceipt {
            uri: record.uri.as_str().to_owned(),
            cid: record.cid,
        })),
        Err(OAuthError::Xrpc { status, .. }) if status == 400 || status == 404 => Ok(None),
        Err(err) => Err(err.into()),
    }
}

/// The card for a document: canonical URL, headline, summary, and the
/// cover. The document's own cover blob is reused when it has one;
/// otherwise the cover the site shows is uploaded once.
async fn link_card(
    state: &AppState,
    identity: &Identity,
    session: &AuthorizedSession,
    record: &Record<Document>,
) -> Result<LinkCard, PublishError> {
    let site = AtUri::parse(&record.value.site)
        .map_err(|_| AppError::BadRequest("the document names no publication".to_owned()))?;
    let publication = state
        .publication(identity, site.rkey())
        .await?
        .ok_or_else(|| AppError::NotFound(format!("publication {} not found", site.rkey())))?;
    let subject_doc = SubjectDocument::from_record(record.clone());
    let title = view::document_headline(subject_doc.as_ref(), &record.value.title);
    let description = match &subject_doc {
        Some(subject_doc) => view::summary(subject_doc),
        None => record.value.description.clone().unwrap_or_default(),
    };
    let url = view::canonical_url(
        state,
        &identity.did,
        site.rkey(),
        record,
        &publication.value,
    );
    let thumb = match (&record.value.cover_image, &subject_doc) {
        (Some(blob), _) => Some(blob.clone()),
        (None, Some(subject_doc)) => {
            let jpeg = state
                .cover_rendition(identity, subject_doc, Size::Card)
                .await
                .jpeg;
            if jpeg.is_empty() || jpeg.len() > MAX_COVER_BYTES {
                None
            } else {
                Some(session.upload_blob(jpeg, "image/jpeg").await?)
            }
        }
        (None, None) => None,
    };
    Ok(LinkCard {
        url,
        title,
        description,
        thumb,
    })
}

/// The paths already used in a publication, from the repo's most recent
/// documents.
async fn taken_paths(
    state: &AppState,
    identity: &Identity,
    site: &AtUri,
) -> Result<Vec<String>, AppError> {
    let mut taken = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PATH_PAGES {
        let page = state.documents(identity, cursor.as_deref()).await?;
        taken.extend(
            page.records
                .iter()
                .filter(|r| r.value.site == site.as_str())
                .filter_map(|r| r.value.path.clone()),
        );
        match page.cursor {
            Some(next) if !page.records.is_empty() => cursor = Some(next),
            _ => break,
        }
    }
    Ok(taken)
}

/// The document and every listing that might have shown it.
async fn forget_document(state: &AppState, did: &Did, rkey: &str) {
    let cache = state.cache();
    cache
        .evict(Namespace::Document, &format!("{did}/{rkey}"))
        .await;
    cache
        .evict_prefix(Namespace::DocumentList, &format!("{did}:"))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::Cover;
    use eaten_at_atproto::lexicon::{ExternalUrl, Subject};

    #[test]
    fn slugs() {
        let cases = [
            ("A room with the lights off", "a-room-with-the-lights-off"),
            (
                "Curió Curió: the sound of missing two places",
                "curio-curio-the-sound-of-missing-two-places",
            ),
            ("  Loud!!  (Live) ", "loud-live"),
            ("Ænima / 1996", "ænima-1996"),
            ("東京", "東京"),
            ("!!!", "untitled"),
            ("", "untitled"),
            ("naïve façade über", "naive-facade-uber"),
        ];
        for (title, expected) in cases {
            assert_eq!(slug(title), expected, "{title:?}");
        }
        let long = slug(&"word ".repeat(40));
        assert!(long.len() <= MAX_SLUG_BYTES, "{long}");
        assert!(!long.ends_with('-'));
    }

    #[test]
    fn paths_are_dated_and_unique() {
        let at = Datetime::parse("2026-09-09T15:00:00.000Z").unwrap();
        assert_eq!(document_path(&at, "Promises", &[]), "/2026/09/promises");
        let taken = vec![
            "/2026/09/promises".to_owned(),
            "/2026/09/promises-2".to_owned(),
        ];
        assert_eq!(
            document_path(&at, "Promises", &taken),
            "/2026/09/promises-3"
        );
    }

    fn draft() -> DocumentDraft {
        DocumentDraft {
            title: "A room with the lights off".into(),
            markdown: "Forty-six *minutes*.\n\nNine notes.".into(),
            description: None,
            tags: vec!["notes".into(), "Short".into()],
            subject: Subject {
                type_: Some(SUBJECT_NSID.into()),
                title: "Promises".into(),
                external_urls: vec![ExternalUrl {
                    url: "https://x.example/a".into(),
                    service: Some("bc".into()),
                    label: None,
                    extra: serde_json::Map::new(),
                }],
                extra: serde_json::Map::new(),
            },
            cover: None,
            target: Target::Existing(
                AtUri::parse(
                    "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1",
                )
                .unwrap(),
            ),
            crosspost: None,
        }
    }

    #[test]
    fn a_post_is_text_plus_a_link_card() {
        let at = Datetime::parse("2026-09-09T15:00:00.000Z").unwrap();
        let thumb: BlobRef = serde_json::from_value(json!({
            "$type": "blob", "ref": {"$link": "bafycover"}, "mimeType": "image/jpeg", "size": 5
        }))
        .unwrap();
        let post = build_post(
            "Floating Points — Promises",
            &LinkCard {
                url: "https://ross.eaten.at/2026/09/promises".into(),
                title: "Promises — Floating Points".into(),
                description: "Forty-six minutes.".into(),
                thumb: Some(thumb),
            },
            &at,
        );
        assert_eq!(
            post,
            json!({
                "$type": "app.bsky.feed.post",
                "text": "Floating Points — Promises",
                "createdAt": "2026-09-09T15:00:00.000Z",
                "embed": {
                    "$type": "app.bsky.embed.external",
                    "external": {
                        "uri": "https://ross.eaten.at/2026/09/promises",
                        "title": "Promises — Floating Points",
                        "description": "Forty-six minutes.",
                        "thumb": {"$type": "blob", "ref": {"$link": "bafycover"}, "mimeType": "image/jpeg", "size": 5}
                    }
                }
            })
        );
        let bare = build_post(
            "x",
            &LinkCard {
                url: "https://a.test/p".into(),
                title: "t".into(),
                description: String::new(),
                thumb: None,
            },
            &at,
        );
        assert!(bare["embed"]["external"].get("thumb").is_none());
        assert!(bare.get("facets").is_none());
    }

    #[test]
    fn the_post_ref_is_the_only_change() {
        let original = json!({
            "$type": DOCUMENT_NSID,
            "site": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1",
            "title": "T",
            "path": "/2026/09/t",
            "publishedAt": "2026-09-09T15:00:00.000Z",
            "updatedAt": "2026-09-09T16:00:00.000Z",
            "foreignField": {"kept": true},
            "links": {"$type": SUBJECT_NSID, "title": "A"},
            "content": {"$type": "at.markpub.markdown", "text": {"$type": "at.markpub.text", "markdown": "m"}}
        });
        let doc: Document = serde_json::from_value(original.clone()).unwrap();
        let written = with_post_ref(
            &doc,
            &WriteReceipt {
                uri: "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/app.bsky.feed.post/3k".into(),
                cid: "bafypost".into(),
            },
        );
        let mut expected = original;
        expected["bskyPostRef"] = json!({
            "uri": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/app.bsky.feed.post/3k",
            "cid": "bafypost"
        });
        assert_eq!(written, expected, "updatedAt untouched, nothing else added");
    }

    #[test]
    fn a_new_document_has_exactly_the_planned_shape() {
        let site =
            AtUri::parse("at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1")
                .unwrap();
        let at = Datetime::parse("2026-09-09T15:00:00.000Z").unwrap();
        let doc = build_document(
            &draft(),
            &Placement {
                site: &site,
                path: "/2026/09/a-room-with-the-lights-off",
                published_at: &at,
                updated_at: None,
                cover: None,
                original: None,
            },
        );
        insta::assert_snapshot!(serde_json::to_string_pretty(&doc).unwrap());
        assert!(doc.get("updatedAt").is_none());
        assert!(doc.get("description").is_none());
        assert_eq!(
            doc["links"]["externalUrls"][0]["service"], "bc",
            "unknown service preserved"
        );
        assert_eq!(doc["textContent"], "Forty-six minutes.\n\nNine notes.");
    }

    #[test]
    fn an_edit_keeps_what_the_editor_does_not_know_and_stamps_updated_at() {
        let original: Document = serde_json::from_value(json!({
            "$type": DOCUMENT_NSID,
            "site": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1",
            "title": "Old title",
            "path": "/2026/08/old-title",
            "publishedAt": "2026-08-30T12:00:00.000Z",
            "description": "old excerpt",
            "coverImage": {"$type": "blob", "ref": {"$link": "bafyold"}, "mimeType": "image/png", "size": 10},
            "bskyPostRef": {"uri": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/app.bsky.feed.post/3k", "cid": "bafy"},
            "labels": {"$type": "com.atproto.label.defs#selfLabels", "values": [{"val": "!warn"}]},
            "foreignField": {"kept": true},
            "links": [
                {"$type": "site.standard.somethingElse", "x": 1},
                {"$type": SUBJECT_NSID, "title": "Old"}
            ],
            "content": {"$type": "at.markpub.markdown", "text": {"$type": "at.markpub.text", "markdown": "old"}}
        }))
        .unwrap();
        let site =
            AtUri::parse("at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1")
                .unwrap();
        let published = Datetime::parse("2026-08-30T12:00:00.000Z").unwrap();
        let updated = Datetime::parse("2026-09-09T15:00:00.000Z").unwrap();
        let mut d = draft();
        d.description = Some("new excerpt".into());
        d.tags.clear();
        let doc = build_document(
            &d,
            &Placement {
                site: &site,
                path: "/2026/08/old-title",
                published_at: &published,
                updated_at: Some(&updated),
                cover: original.cover_image.as_ref(),
                original: Some(&original),
            },
        );
        assert_eq!(doc["title"], "A room with the lights off");
        assert_eq!(doc["path"], "/2026/08/old-title", "the path is immutable");
        assert_eq!(doc["publishedAt"], "2026-08-30T12:00:00.000Z");
        assert_eq!(doc["updatedAt"], "2026-09-09T15:00:00.000Z");
        assert_eq!(doc["description"], "new excerpt");
        assert_eq!(
            doc["coverImage"]["ref"]["$link"], "bafyold",
            "the cover is kept"
        );
        assert_eq!(doc["bskyPostRef"]["cid"], "bafy");
        assert_eq!(doc["labels"]["values"][0]["val"], "!warn");
        assert_eq!(doc["foreignField"]["kept"], true);
        assert!(doc.get("tags").is_none());
        let links = doc["links"]
            .as_array()
            .expect("other members keep the array form");
        assert_eq!(links.len(), 2);
        assert_eq!(links[0]["$type"], "site.standard.somethingElse");
        assert_eq!(links[1]["title"], "Promises");
        assert_eq!(
            doc["content"]["text"]["markdown"],
            "Forty-six *minutes*.\n\nNine notes."
        );
    }

    #[test]
    fn a_new_cover_replaces_the_old_one() {
        let site =
            AtUri::parse("at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1")
                .unwrap();
        let at = Datetime::parse("2026-09-09T15:00:00.000Z").unwrap();
        let blob: BlobRef = serde_json::from_value(json!({
            "$type": "blob", "ref": {"$link": "bafynew"}, "mimeType": "image/jpeg", "size": 5
        }))
        .unwrap();
        let mut d = draft();
        d.cover = Some(Cover {
            bytes: vec![1],
            mime: "image/jpeg",
        });
        let doc = build_document(
            &d,
            &Placement {
                site: &site,
                path: "/p",
                published_at: &at,
                updated_at: None,
                cover: Some(&blob),
                original: None,
            },
        );
        assert_eq!(doc["coverImage"]["ref"]["$link"], "bafynew");
        assert_eq!(doc["coverImage"]["mimeType"], "image/jpeg");
    }
}
