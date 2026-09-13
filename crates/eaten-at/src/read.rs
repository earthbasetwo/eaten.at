//! The cached read path: identity, publications, and documents.
//!
//! Every method here answers from the cache when it can and otherwise
//! fetches through the guarded client. Callers never touch the repo client
//! directly, so TTLs and tombstone handling live in one place.

use eaten_at_atproto::identity::{Did, Identity, IdentityError};
use eaten_at_atproto::lexicon::{Document, Publication, DOCUMENT_NSID, PUBLICATION_NSID};
use eaten_at_atproto::repo::{ListPage, ListParams, Record, RepoError};

use crate::cache::Namespace;
use crate::error::AppError;
use crate::state::AppState;

/// Page size for document listings.
pub const DOCUMENT_PAGE_SIZE: u32 = 20;
/// Most publications a repo is expected to have; more are ignored.
const PUBLICATION_LIST_LIMIT: u32 = 50;

impl AppState {
    /// Resolve a DID, cached. `Ok(None)` when the DID does not exist.
    pub async fn identity_for(&self, did: &Did) -> Result<Option<Identity>, AppError> {
        self.cache()
            .get_or_fetch(Namespace::Identity, did.as_str(), || async {
                match self.identity().resolve_did(did).await {
                    Ok(identity) => Ok(Some(identity)),
                    Err(IdentityError::DidNotFound(_)) => Ok(None),
                    Err(err) => Err(AppError::from(err)),
                }
            })
            .await
    }

    /// Resolve a DID or fail with 404.
    pub async fn require_identity(&self, did: &Did) -> Result<Identity, AppError> {
        self.identity_for(did)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("DID {did} not found")))
    }

    /// Every publication in the repo, cached.
    pub async fn publications(
        &self,
        identity: &Identity,
    ) -> Result<Vec<Record<Publication>>, AppError> {
        let key = format!("list:{}", identity.did);
        let page = self
            .cache()
            .get_or_fetch(Namespace::Publication, &key, || async {
                let page = self
                    .repo_for(identity)
                    .list_records::<Publication>(
                        &identity.did,
                        PUBLICATION_NSID,
                        &ListParams {
                            limit: Some(PUBLICATION_LIST_LIMIT),
                            ..ListParams::default()
                        },
                    )
                    .await
                    .map_err(AppError::from)?;
                Ok::<_, AppError>(Some(page.records))
            })
            .await?;
        Ok(page.unwrap_or_default())
    }

    /// One publication record, cached. `Ok(None)` when it does not exist.
    pub async fn publication(
        &self,
        identity: &Identity,
        rkey: &str,
    ) -> Result<Option<Record<Publication>>, AppError> {
        let key = format!("{}/{rkey}", identity.did);
        self.cache()
            .get_or_fetch(Namespace::Publication, &key, || async {
                not_found_as_none(
                    self.repo_for(identity)
                        .get_record::<Publication>(&identity.did, PUBLICATION_NSID, rkey)
                        .await,
                )
            })
            .await
    }

    /// One page of the repo's documents, newest first, cached.
    pub async fn documents(
        &self,
        identity: &Identity,
        cursor: Option<&str>,
    ) -> Result<ListPage<Document>, AppError> {
        let key = document_list_key(&identity.did, cursor);
        let page = self
            .cache()
            .get_or_fetch(Namespace::DocumentList, &key, || async {
                let page = self
                    .repo_for(identity)
                    .list_records::<Document>(
                        &identity.did,
                        DOCUMENT_NSID,
                        &ListParams {
                            limit: Some(DOCUMENT_PAGE_SIZE),
                            cursor: cursor.map(str::to_owned),
                            reverse: false,
                        },
                    )
                    .await
                    .map_err(AppError::from)?;
                Ok::<_, AppError>(Some(page))
            })
            .await?;
        Ok(page.unwrap_or_else(|| ListPage {
            records: Vec::new(),
            cursor: None,
            skipped: 0,
        }))
    }

    /// One document, cached. A 404 from the PDS is a tombstone: the
    /// document and every cached listing for the repo are evicted so the
    /// deletion shows immediately.
    pub async fn document(
        &self,
        identity: &Identity,
        rkey: &str,
    ) -> Result<Option<Record<Document>>, AppError> {
        let key = format!("{}/{rkey}", identity.did);
        let cache = self.cache();
        let found = cache
            .get_or_fetch(Namespace::Document, &key, || async {
                not_found_as_none(
                    self.repo_for(identity)
                        .get_record::<Document>(&identity.did, DOCUMENT_NSID, rkey)
                        .await,
                )
            })
            .await?;
        if found.is_none() {
            cache
                .evict_prefix(
                    Namespace::DocumentList,
                    &document_list_key(&identity.did, None),
                )
                .await;
        }
        Ok(found)
    }
}

fn document_list_key(did: &Did, cursor: Option<&str>) -> String {
    format!("{did}:{}", cursor.unwrap_or_default())
}

fn not_found_as_none<T>(result: Result<T, RepoError>) -> Result<Option<T>, AppError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(RepoError::RecordNotFound | RepoError::RepoNotFound) => Ok(None),
        Err(err) => Err(AppError::from(err)),
    }
}

// ---- Publication choice, preferences, handle lookup, visit listings ----

use eaten_at_atproto::at_uri::AtUri;

use eaten_at_atproto::identity::Handle;
use eaten_at_atproto::lexicon::at_eaten::{PREFERENCES_NSID, PREFERENCES_RKEY};
use eaten_at_atproto::lexicon::Preferences;

use crate::model::VisitDocument;

/// Pages of the document collection scanned per listing request before
/// giving up on filling a page (plan §5.2, "filtering cost").
pub const MAX_LIST_SCAN_PAGES: usize = 5;

/// What `/at/{did}/` should do.
#[derive(Debug, Clone)]
pub enum PublicationChoice {
    /// One publication is the clear answer: the preference, or the only one.
    Chosen(Box<Record<Publication>>),
    /// Several publications and no usable preference: let the reader pick.
    Choose(Vec<Record<Publication>>),
    /// The repo has no publications at all.
    None,
}

/// One page of visit documents for a publication.
#[derive(Debug, Clone, Default)]
pub struct VisitListing {
    pub items: Vec<VisitDocument>,
    /// Cursor for the next page, if there may be more.
    pub next_cursor: Option<String>,
    /// The scan cap stopped the listing before the page was full. The page
    /// should say so rather than imply the publication has no more.
    pub truncated: bool,
}

impl AppState {
    /// The user's `at.eaten.preferences/self`, cached. Absent reads as
    /// default preferences.
    pub async fn preferences(&self, identity: &Identity) -> Result<Preferences, AppError> {
        let key = format!("prefs:{}", identity.did);
        let found = self
            .cache()
            .get_or_fetch(Namespace::Publication, &key, || async {
                not_found_as_none(
                    self.repo_for(identity)
                        .get_record::<Preferences>(
                            &identity.did,
                            PREFERENCES_NSID,
                            PREFERENCES_RKEY,
                        )
                        .await,
                )
            })
            .await?;
        Ok(found.map(|r| r.value).unwrap_or_default())
    }

    /// Resolve `/at/{did}/` per plan §4.3: a valid `defaultPublication`
    /// wins; else a lone publication is used; else the reader chooses.
    pub async fn choose_publication(
        &self,
        identity: &Identity,
    ) -> Result<PublicationChoice, AppError> {
        let publications = self.publications(identity).await?;
        if publications.is_empty() {
            return Ok(PublicationChoice::None);
        }
        let preferences = self.preferences(identity).await?;
        if let Some(preferred) = preferences.default_publication {
            if let Some(found) = publications.iter().find(|p| p.uri == preferred) {
                return Ok(PublicationChoice::Chosen(Box::new(found.clone())));
            }
            tracing::debug!(did = %identity.did, %preferred, "defaultPublication is dangling");
        }
        match publications.as_slice() {
            [only] => Ok(PublicationChoice::Chosen(Box::new(only.clone()))),
            _ => Ok(PublicationChoice::Choose(publications)),
        }
    }

    /// Handle → DID for the `/@handle` redirect, cached briefly. A handle
    /// is a lookup of who holds it *now*, so the TTL is short.
    pub async fn lookup_handle(&self, handle: &Handle) -> Result<Option<Did>, AppError> {
        self.cache()
            .get_or_fetch(Namespace::Handle, handle.as_str(), || async {
                match self.identity().resolve_handle(handle).await {
                    Ok(identity) => Ok(Some(identity.did)),
                    Err(
                        IdentityError::HandleNotFound(_)
                        | IdentityError::HandleNotClaimed { .. }
                        | IdentityError::AmbiguousDns(_)
                        | IdentityError::DidNotFound(_),
                    ) => Ok(None),
                    Err(err) => Err(AppError::from(err)),
                }
            })
            .await
    }

    /// Visit documents belonging to `publication`, newest first, one page
    /// at a time. Scans the repo's document collection page by page,
    /// keeping only visit documents whose `site` is this publication,
    /// until a page is full or [`MAX_LIST_SCAN_PAGES`] pages have been read.
    pub async fn visit_listing(
        &self,
        identity: &Identity,
        publication: &Record<Publication>,
        cursor: Option<&str>,
    ) -> Result<VisitListing, AppError> {
        self.scan_visits(identity, publication, cursor, |_| true)
            .await
    }

    /// [`visit_listing`](Self::visit_listing) restricted to documents
    /// carrying `tag` (plan §7.6: publication-scoped, matched loosely).
    pub async fn tagged_listing(
        &self,
        identity: &Identity,
        publication: &Record<Publication>,
        tag: &str,
        cursor: Option<&str>,
    ) -> Result<VisitListing, AppError> {
        self.scan_visits(identity, publication, cursor, |visit_doc| {
            visit_doc
                .document()
                .tags
                .iter()
                .any(|t| crate::tags::matches(t, tag))
        })
        .await
    }

    async fn scan_visits(
        &self,
        identity: &Identity,
        publication: &Record<Publication>,
        cursor: Option<&str>,
        keep: impl Fn(&VisitDocument) -> bool,
    ) -> Result<VisitListing, AppError> {
        let mut listing = VisitListing::default();
        let mut cursor = cursor.map(str::to_owned);
        let page_size = DOCUMENT_PAGE_SIZE as usize;
        for scanned in 0.. {
            let page = self.documents(identity, cursor.as_deref()).await?;
            let more = page.cursor.is_some();
            for record in page.records {
                if listing.items.len() >= page_size {
                    break;
                }
                if !belongs_to(&record.value, &publication.uri) {
                    continue;
                }
                if let Some(visit_doc) = VisitDocument::from_record(record) {
                    if keep(&visit_doc) {
                        listing.items.push(visit_doc);
                    }
                }
            }
            cursor = page.cursor;
            if !more {
                listing.next_cursor = None;
                return Ok(listing);
            }
            if listing.items.len() >= page_size {
                listing.next_cursor = cursor;
                return Ok(listing);
            }
            if scanned + 1 >= MAX_LIST_SCAN_PAGES {
                listing.truncated = true;
                listing.next_cursor = cursor;
                return Ok(listing);
            }
        }
        unreachable!("loop returns")
    }

    /// One document, only if it belongs to `publication`.
    pub async fn document_in(
        &self,
        identity: &Identity,
        publication: &Record<Publication>,
        rkey: &str,
    ) -> Result<Option<Record<Document>>, AppError> {
        Ok(self
            .document(identity, rkey)
            .await?
            .filter(|record| belongs_to(&record.value, &publication.uri)))
    }
}

/// Whether a document's `site` names this publication. Trailing slashes
/// are tolerated, as the standard's own guidance suggests.
fn belongs_to(doc: &Document, publication: &AtUri) -> bool {
    doc.site.trim_end_matches('/') == publication.as_str()
}
