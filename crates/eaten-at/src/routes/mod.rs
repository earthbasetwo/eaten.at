//! HTTP route handlers.

pub mod assets;
pub mod auth;
pub mod document;
pub mod feed;
pub mod image;
pub mod interstitial;
pub mod landing;
pub mod lookup;
pub mod publication;
pub mod settings;
pub mod write;

use eaten_at_atproto::identity::{Did, Identity};
use eaten_at_atproto::lexicon::Publication;
use eaten_at_atproto::repo::Record;

use crate::error::AppError;
use crate::state::AppState;

/// Parse the DID path segment and resolve it, or fail with 400 / 404.
pub(crate) async fn resolve_repo(state: &AppState, did: &str) -> Result<(Did, Identity), AppError> {
    let did = Did::parse(did)?;
    let identity = state.require_identity(&did).await?;
    Ok((did, identity))
}

/// Fetch a publication by rkey or fail with 404.
pub(crate) async fn require_publication(
    state: &AppState,
    identity: &Identity,
    pub_rkey: &str,
) -> Result<Record<Publication>, AppError> {
    state
        .publication(identity, pub_rkey)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("publication {pub_rkey} not found")))
}
