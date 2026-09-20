//! From a [`DocumentDraft`] to records in the author's repository
//! (plan §5.4): the publication and preferences a first publish needs,
//! the document itself, and the cache entries that must forget the old
//! state afterwards.

use eaten_at_atproto::at_uri::AtUri;
use eaten_at_atproto::identity::{Did, Identity};
use eaten_at_atproto::lexicon::at_eaten::{PREFERENCES_NSID, PREFERENCES_RKEY};
use eaten_at_atproto::lexicon::{
    BlobRef, Datetime, Document, Photo, Preferences, Publication, Visit, DOCUMENT_NSID,
    PUBLICATION_NSID, VISIT_NSID,
};
use eaten_at_atproto::oauth::{AuthorizedSession, OAuthError};
use eaten_at_atproto::repo::write::WriteReceipt;
use eaten_at_atproto::repo::Record;
use eaten_at_atproto::tid::Tid;
use eaten_at_web::markdown;
use serde_json::{json, Value};
use unicode_normalization::UnicodeNormalization;

use crate::cache::Namespace;
use crate::editor::DocumentDraft;
use crate::error::AppError;
use crate::hosting::{self, ClaimError};
use crate::img::{Size, MAX_IMAGE_BLOB_BYTES};
use crate::model::VisitDocument;
use crate::paths;
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
    /// The publication's hosted address could not be claimed; the
    /// message is for the author.
    #[error("{0}")]
    Home(String),
    #[error(transparent)]
    App(#[from] AppError),
}

impl From<ClaimError> for PublishError {
    fn from(err: ClaimError) -> Self {
        match err {
            ClaimError::Db(err) => Self::App(AppError::Upstream(err.to_string())),
            other => Self::Home(format!("{other}.")),
        }
    }
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
    /// The document's `site`, as written.
    pub site: String,
    pub doc_rkey: String,
}

impl Published {
    /// The document's own page on this site, or the repo's when its
    /// `site` is not a publication record here.
    pub fn document_path(&self) -> String {
        AtUri::parse(&self.site).map_or_else(
            |_| paths::repo(&self.did),
            |site| paths::document(&self.did, site.rkey(), &self.doc_rkey),
        )
    }
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
    /// The publication's AT-URI, or whatever `site` the original had.
    pub site: &'a str,
    pub path: &'a str,
    pub published_at: &'a Datetime,
    /// Set on edits only.
    pub updated_at: Option<&'a Datetime>,
    /// The record being replaced, whose unknown fields are kept.
    pub original: Option<&'a Document>,
}

/// The `site.standard.document` record for a draft, exactly as written
/// (plan §4.1, §5.4). Fields our editor does not know about survive from
/// the original; `description` is present only when the author wrote
/// one; `content` is the visit with the prose inside it, and
/// `textContent` is its plaintext rendering. `links` is not ours and is
/// carried over as found; so is `coverImage`, except that a visit with
/// photos gets its first photo there (D32, D38).
pub fn build_document(draft: &DocumentDraft, placement: &Placement<'_>) -> Value {
    let mut doc = placement.original.map_or_else(
        || json!({}),
        |d| serde_json::to_value(d).unwrap_or_default(),
    );
    let Value::Object(fields) = &mut doc else {
        unreachable!("a document serializes to an object")
    };
    fields.insert("$type".into(), json!(DOCUMENT_NSID));
    fields.insert("site".into(), json!(placement.site));
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
    fields.insert("content".into(), content(draft));
    // The cover follows the photos (D38): the first one, or, once the
    // author has removed the last, none. A cover another client set
    // on a visit that never had photos is not ours to touch.
    let had_photos = placement
        .original
        .and_then(|d| crate::model::extract_visit(d.content.as_ref()))
        .is_some_and(|v| !v.photos.is_empty());
    if draft.visit.photos.is_empty() && had_photos {
        fields.remove("coverImage");
    }
    set_cover_from_photos(fields, &draft.visit.photos);
    fields.insert(
        "textContent".into(),
        json!(text_content(&draft.visit, &draft.markdown)),
    );
    if draft.tags.is_empty() {
        fields.remove("tags");
    } else {
        fields.insert("tags".into(), json!(draft.tags));
    }
    doc
}

/// The document's `coverImage` is its first photo, so Standard readers
/// and unfurlers get a thumbnail (D38). With no photos, whatever was
/// there stays: not ours to touch.
fn set_cover_from_photos(fields: &mut serde_json::Map<String, Value>, photos: &[Photo]) {
    if let Some(first) = photos.first() {
        fields.insert("coverImage".into(), json!(first.image));
    }
}

/// Rewrite a document's photos (plan 07): the visit's `photos`, the
/// derived `coverImage` (removed with the last photo), and `updatedAt`;
/// nothing else changes. Then forget the cached document.
pub async fn save_photos(
    state: &AppState,
    identity: &Identity,
    visit_doc: &VisitDocument,
    photos: Vec<Photo>,
) -> Result<(), PublishError> {
    let did = &identity.did;
    let session = state.oauth().session(did).await?;
    let record = photos_record(visit_doc, photos, &Datetime::now());
    session
        .put_record(DOCUMENT_NSID, visit_doc.rkey(), &record)
        .await?;
    forget_document(state, did, visit_doc.rkey()).await;
    Ok(())
}

/// The record [`save_photos`] writes.
pub fn photos_record(visit_doc: &VisitDocument, photos: Vec<Photo>, now: &Datetime) -> Value {
    let mut visit = visit_doc.visit.clone();
    visit.type_ = Some(VISIT_NSID.to_owned());
    visit.photos = photos;
    let mut doc = serde_json::to_value(visit_doc.document()).unwrap_or_default();
    let Value::Object(fields) = &mut doc else {
        unreachable!("a document serializes to an object")
    };
    fields.insert("$type".into(), json!(DOCUMENT_NSID));
    fields.insert("updatedAt".into(), json!(now));
    if visit.photos.is_empty() {
        fields.remove("coverImage");
    } else {
        set_cover_from_photos(fields, &visit.photos);
    }
    fields.insert(
        "content".into(),
        serde_json::to_value(&visit).unwrap_or_default(),
    );
    doc
}

/// The document's `content`: the draft's visit, typed, with the markdown
/// body inside it (D12: `text.markdown` only).
pub fn content(draft: &DocumentDraft) -> Value {
    let mut visit = draft.visit.clone();
    visit.type_ = Some(VISIT_NSID.to_owned());
    visit.body = Some(json!({
        "$type": "at.markpub.markdown",
        "flavor": "commonmark",
        "text": { "$type": "at.markpub.text", "markdown": draft.markdown },
    }));
    serde_json::to_value(&visit).unwrap_or_default()
}

/// The plaintext a reader that does not know `at.eaten.visit` sees: a
/// line naming the place, the date, and the verdict, then the prose
/// without markup.
pub fn text_content(visit: &Visit, markdown: &str) -> String {
    let mut header = format!("{} · {}", visit.place.name.trim(), visit.visited_on);
    if let Some(rating) = visit.rating {
        header.push_str(" · ");
        header.push_str(rating.word());
    }
    let mut parts = vec![header];
    let prose = markdown::to_plaintext(markdown);
    if !prose.trim().is_empty() {
        parts.push(prose.trim().to_owned());
    }
    parts.join("\n\n")
}

/// Where a new publication is served from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Home {
    /// This exact hosted subdomain label, or an error when it is not free.
    Hosted(String),
    /// This label, or the first of `label-2`, `label-3`, … that is free.
    HostedOrNext(String),
    /// A domain the author serves themselves; an `https` origin.
    Own(String),
    /// The publication's own site route here, for a deployment that
    /// cannot host subdomains (local development).
    SiteRoute,
}

/// What a publication is created with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationSpec {
    pub name: String,
    pub description: Option<String>,
    pub home: Home,
}

/// The account's publication and the preferences that name it.
#[derive(Debug, Clone)]
pub struct Ensured {
    pub publication: Record<Publication>,
    pub preferences: Preferences,
}

/// The defaults a publication is created with when the author has not
/// said otherwise (plan 08, D41): named after the handle, hosted at a
/// subdomain made from its first label.
pub fn default_spec(state: &AppState, identity: &Identity) -> PublicationSpec {
    let name = identity
        .handle
        .as_ref()
        .map_or_else(|| identity.did.to_string(), |h| h.as_str().to_owned());
    let home = if state.can_host_subdomains() {
        Home::HostedOrNext(hosting::suggest_name(
            &identity.did,
            identity.handle.as_ref(),
        ))
    } else {
        Home::SiteRoute
    };
    PublicationSpec {
        name,
        description: None,
        home,
    }
}

/// The account's eaten.at publication, created with the defaults when
/// there is none yet (plan 08, D40). The preferences come back too, so
/// a caller that goes on to write them does not read them twice.
pub async fn ensure_publication(
    state: &AppState,
    identity: &Identity,
    session: &AuthorizedSession,
    now: &Datetime,
) -> Result<Ensured, PublishError> {
    let preferences = state.preferences(identity).await?;
    if let Some(publication) = state.designated(identity, &preferences).await? {
        return Ok(Ensured {
            publication,
            preferences,
        });
    }
    let spec = default_spec(state, identity);
    create_publication(state, identity, session, spec, preferences, now).await
}

/// Create the account's publication and write the preference that
/// designates it. The record key is minted here so a site-route address
/// can name it before the write. A hosted name is claimed before the
/// write and released if the author's server refuses it, so the two
/// never disagree.
pub async fn create_publication(
    state: &AppState,
    identity: &Identity,
    session: &AuthorizedSession,
    spec: PublicationSpec,
    mut preferences: Preferences,
    now: &Datetime,
) -> Result<Ensured, PublishError> {
    let did = &identity.did;
    let rkey = Tid::now();
    let uri = AtUri::from_parts(did, PUBLICATION_NSID, rkey.as_str())
        .map_err(|e| AppError::Upstream(format!("could not address the publication: {e}")))?;
    let (origin, claimed) = match &spec.home {
        Home::Hosted(label) => {
            let name = hosting::validate_name(label)?;
            state.claims().claim(&name, &uri, did).await?;
            (state.hosted_origin(&name), Some(name))
        }
        Home::HostedOrNext(label) => {
            let name = state.claim_free(label, &uri, did).await?;
            (state.hosted_origin(&name), Some(name))
        }
        Home::Own(own) => (own.trim_end_matches('/').to_owned(), None),
        Home::SiteRoute => {
            let path = paths::publication(did, rkey.as_str());
            (state.absolute(path.trim_end_matches('/')), None)
        }
    };
    let mut record = json!({
        "$type": PUBLICATION_NSID,
        "url": origin,
        "name": spec.name,
    });
    if let Some(description) = &spec.description {
        record["description"] = json!(description);
    }
    let receipt = match session
        .create_record(PUBLICATION_NSID, Some(rkey.as_str()), &record)
        .await
    {
        Ok(receipt) => receipt,
        Err(err) => {
            if claimed.is_some() {
                if let Err(release) = state.claims().release(&uri).await {
                    tracing::error!(error = %release, %uri, "could not release the claim after a refused write");
                }
            }
            return Err(err.into());
        }
    };
    let written = AtUri::parse(&receipt.uri).map_err(|e| {
        PublishError::Repo(OAuthError::Transport(format!(
            "PDS returned an unusable record URI: {e}"
        )))
    })?;
    if written != uri {
        // A server that ignored the key we asked for. The claim follows
        // the record; the site-route address, if any, is now stale and
        // the settings page can fix it.
        tracing::warn!(asked = %uri, got = %written, "the server chose its own record key");
        if let Some(name) = &claimed {
            if let Err(err) = state.claims().release(&uri).await {
                tracing::error!(error = %err, "could not move the claim to the written key");
            }
            state.claims().claim(name, &written, did).await?;
        }
    }

    preferences.type_ = Some(PREFERENCES_NSID.to_owned());
    preferences.default_publication = Some(written.clone());
    if preferences.created_at.is_none() {
        preferences.created_at = Some(now.clone());
    }
    session
        .put_record(
            PREFERENCES_NSID,
            PREFERENCES_RKEY,
            &serde_json::to_value(&preferences).unwrap_or_default(),
        )
        .await?;

    let cache = state.cache();
    cache
        .evict(Namespace::Publication, &format!("list:{did}"))
        .await;
    cache
        .evict(Namespace::Publication, &format!("prefs:{did}"))
        .await;
    cache
        .evict(Namespace::Publication, &format!("{did}/{}", written.rkey()))
        .await;
    let value: Publication = serde_json::from_value(record).map_err(|e| {
        AppError::Upstream(format!("the publication written does not read back: {e}"))
    })?;
    Ok(Ensured {
        publication: Record {
            uri: written,
            cid: receipt.cid,
            value,
        },
        preferences,
    })
}

/// Publish a draft: make sure the account has its publication, write
/// the document, and forget the cached state it changes.
pub async fn publish(
    state: &AppState,
    identity: &Identity,
    draft: &DocumentDraft,
    editing: Option<&VisitDocument>,
) -> Result<Published, PublishError> {
    let did = &identity.did;
    let session = state.oauth().session(did).await?;
    let now = Datetime::now();
    let original = editing.map(VisitDocument::document);

    // An edit stays where it is, whatever publication put it there; a
    // new write-up goes to the account's one publication (plan 08).
    let (site, preferences) = if let Some(doc) = original {
        (
            doc.site.trim_end_matches('/').to_owned(),
            state.preferences(identity).await?,
        )
    } else {
        let ensured = ensure_publication(state, identity, &session, &now).await?;
        (
            ensured.publication.uri.as_str().to_owned(),
            ensured.preferences,
        )
    };

    update_preferences(
        state,
        identity,
        &session,
        preferences,
        draft.crosspost.is_some(),
        &now,
    )
    .await?;

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
        site,
        doc_rkey,
    })
}

/// The crosspost default is written only when the author's choice
/// changed, so the editor's toggle remembers it (plan §4.3). An author
/// who never crossposts never gets the field. The designation itself is
/// [`create_publication`]'s to write.
async fn update_preferences(
    state: &AppState,
    identity: &Identity,
    session: &AuthorizedSession,
    mut preferences: Preferences,
    crosspost: bool,
    now: &Datetime,
) -> Result<(), PublishError> {
    if preferences.crosspost_default() == crosspost {
        return Ok(());
    }
    preferences.type_ = Some(PREFERENCES_NSID.to_owned());
    preferences.crosspost_to_bluesky = Some(crosspost);
    if preferences.created_at.is_none() {
        preferences.created_at = Some(now.clone());
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
    let visit_doc = VisitDocument::from_record(record.clone());
    let title = record.value.title.clone();
    let description = match &visit_doc {
        Some(visit_doc) => view::summary(visit_doc),
        None => record.value.description.clone().unwrap_or_default(),
    };
    let url = view::canonical_url(
        state,
        &identity.did,
        site.rkey(),
        record,
        &publication.value,
    );
    let thumb = match (&record.value.cover_image, &visit_doc) {
        (Some(blob), _) => Some(blob.clone()),
        (None, Some(visit_doc)) => {
            let jpeg = state
                .cover_rendition(identity, visit_doc, Size::Card)
                .await
                .jpeg;
            if jpeg.is_empty() || jpeg.len() > MAX_IMAGE_BLOB_BYTES {
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
    site: &str,
) -> Result<Vec<String>, AppError> {
    let mut taken = Vec::new();
    let mut cursor: Option<String> = None;
    for _ in 0..MAX_PATH_PAGES {
        let page = state.documents(identity, cursor.as_deref()).await?;
        taken.extend(
            page.records
                .iter()
                .filter(|r| r.value.site.trim_end_matches('/') == site)
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
            visit: serde_json::from_value(json!({
                "place": {
                    "name": "Promises",
                    "address": "1 Example St",
                    "price": 2,
                    "gersId": "g1",
                    "latE6": 40_688_838,
                    "lonE6": -73_979_914,
                    "urls": [{"url": "https://x.example/a", "service": "bc"}]
                },
                "visitedOn": "2026-09-08",
                "meal": "dinner",
                "rating": 3
            }))
            .unwrap(),
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
            "links": {"$type": "com.example.link", "url": "https://elsewhere.example"},
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
        let site = "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1";
        let at = Datetime::parse("2026-09-09T15:00:00.000Z").unwrap();
        let doc = build_document(
            &draft(),
            &Placement {
                site,
                path: "/2026/09/a-room-with-the-lights-off",
                published_at: &at,
                updated_at: None,
                original: None,
            },
        );
        insta::assert_snapshot!(serde_json::to_string_pretty(&doc).unwrap());
        assert!(doc.get("updatedAt").is_none());
        assert!(doc.get("description").is_none());
        assert!(doc.get("links").is_none(), "links are not ours to write");
        assert_eq!(doc["content"]["$type"], VISIT_NSID);
        assert_eq!(
            doc["content"]["place"]["urls"][0]["service"], "bc",
            "unknown service preserved"
        );
        assert_eq!(doc["content"]["body"]["$type"], "at.markpub.markdown");
        assert_eq!(
            doc["textContent"],
            "Promises · 2026-09-08 · Strongly Recommended\n\nForty-six minutes.\n\nNine notes."
        );
    }

    #[test]
    fn text_content_reads_on_its_own() {
        let mut d = draft();
        d.visit.rating = None;
        d.markdown = "  ".into();
        assert_eq!(text_content(&d.visit, &d.markdown), "Promises · 2026-09-08");
        d.markdown = "# Head\n\nSome *words*.".into();
        assert_eq!(
            text_content(&d.visit, &d.markdown),
            "Promises · 2026-09-08\n\nHead\n\nSome words."
        );
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
                {"$type": "com.example.link", "url": "https://elsewhere.example"}
            ],
            "content": {
                "$type": VISIT_NSID,
                "place": {"name": "Old"},
                "visitedOn": "2026-08-29",
                "body": {"$type": "at.markpub.markdown", "text": {"$type": "at.markpub.text", "markdown": "old"}}
            }
        }))
        .unwrap();
        let site = "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1";
        let published = Datetime::parse("2026-08-30T12:00:00.000Z").unwrap();
        let updated = Datetime::parse("2026-09-09T15:00:00.000Z").unwrap();
        let mut d = draft();
        d.description = Some("new excerpt".into());
        d.tags.clear();
        let doc = build_document(
            &d,
            &Placement {
                site,
                path: "/2026/08/old-title",
                published_at: &published,
                updated_at: Some(&updated),
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
            "a foreign cover is carried over, not ours to touch (D32)"
        );
        assert_eq!(doc["bskyPostRef"]["cid"], "bafy");
        assert_eq!(doc["labels"]["values"][0]["val"], "!warn");
        assert_eq!(doc["foreignField"]["kept"], true);
        assert!(doc.get("tags").is_none());
        let links = doc["links"]
            .as_array()
            .expect("foreign links keep the array form");
        assert_eq!(links.len(), 2, "links are carried over untouched");
        assert_eq!(links[0]["$type"], "site.standard.somethingElse");
        assert_eq!(doc["content"]["place"]["name"], "Promises");
        assert_eq!(doc["content"]["visitedOn"], "2026-09-08");
        assert_eq!(
            doc["content"]["body"]["text"]["markdown"],
            "Forty-six *minutes*.\n\nNine notes."
        );
    }
}
