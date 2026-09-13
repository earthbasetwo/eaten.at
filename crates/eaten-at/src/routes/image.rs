//! `/img/{did}/{doc_rkey}` — the document-image proxy — and
//! `/img/{did}/{doc_rkey}/{cid}`, one of its photos.

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, Response};
use serde::Deserialize;

use super::resolve_repo;
use crate::error::AppError;
use crate::img::{PhotoSize, Rendition, Size};
use crate::model::VisitDocument;
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
        let visit_doc = VisitDocument::from_record(record)
            .ok_or_else(|| AppError::NotFound(format!("document {rkey} is not a visit")))?;
        let size = Size::from_query(query.size.as_deref());
        state.cover_rendition(&identity, &visit_doc, size).await
    };

    jpeg_response(rendition, "cover.jpg")
}

#[derive(Debug, Deserialize)]
pub struct PhotoQuery {
    size: Option<String>,
}

/// One of a visit's photos, by the CID the document lists. Any other CID
/// is not found: this is not a way to read arbitrary blobs.
pub async fn photo(
    State(state): State<AppState>,
    Path((did, rkey, cid)): Path<(String, String, String)>,
    Query(query): Query<PhotoQuery>,
) -> Result<Response<Body>, AppError> {
    let (_, identity) = resolve_repo(&state, &did).await?;
    let record = state
        .document(&identity, &rkey)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("document {rkey} not found")))?;
    let visit_doc = VisitDocument::from_record(record)
        .ok_or_else(|| AppError::NotFound(format!("document {rkey} is not a visit")))?;
    let size = PhotoSize::from_query(query.size.as_deref());
    let rendition = state
        .photo_rendition(&identity, &visit_doc, &cid, size)
        .await
        .ok_or_else(|| AppError::NotFound(format!("photo {cid} not found")))?;
    jpeg_response(rendition, "photo.jpg")
}

fn jpeg_response(rendition: Rendition, filename: &'static str) -> Result<Response<Body>, AppError> {
    let response = Response::builder()
        .header(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"))
        .header(
            header::CONTENT_DISPOSITION,
            format!("inline; filename=\"{filename}\""),
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
