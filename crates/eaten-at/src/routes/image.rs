//! `/img/{did}/{doc_rkey}` — the cover-art proxy.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, Response};
use serde::Deserialize;

use super::resolve_repo;
use crate::error::AppError;
use crate::img::Size;
use crate::model::SubjectDocument;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ImageQuery {
    size: Option<String>,
    /// `icon` selects a publication icon (the rkey is then a publication).
    kind: Option<String>,
}

pub async fn cover(
    State(state): State<AppState>,
    Path((did, rkey)): Path<(String, String)>,
    Query(query): Query<ImageQuery>,
) -> Result<Response<Body>, AppError> {
    let (_, identity) = resolve_repo(&state, &did).await?;
    let rendition = if query.kind.as_deref() == Some("icon") {
        let publication = state
            .publication(&identity, &rkey)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("publication {rkey} not found")))?;
        state.icon_rendition(&identity, &publication).await
    } else {
        let record = state
            .document(&identity, &rkey)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("document {rkey} not found")))?;
        let subject_doc = SubjectDocument::from_record(record)
            .ok_or_else(|| AppError::NotFound(format!("document {rkey} has no subject")))?;
        let size = Size::from_query(query.size.as_deref());
        state.cover_rendition(&identity, &subject_doc, size).await
    };

    let response = Response::builder()
        .header(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"))
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("inline; filename=\"cover.jpg\""),
        )
        .header(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        )
        // Even a JPEG we encoded ourselves is served as if hostile: no
        // scripts, no frames, no origin.
        .header(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'none'; sandbox"),
        )
        .header(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=3600"),
        )
        .header(header::CONTENT_LENGTH, rendition.jpeg.len())
        .body(Body::from(rendition.jpeg))
        .map_err(|e| AppError::Upstream(e.to_string()))?;
    Ok(response)
}
