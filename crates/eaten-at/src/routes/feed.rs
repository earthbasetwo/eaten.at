//! `/at/{did}/{pub_rkey}/feed.xml` — RSS for a publication.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, Response};

use super::{require_publication, resolve_repo};
use crate::error::AppError;
use crate::feed::{self, Channel, Item};
use crate::paths;
use crate::state::AppState;
use crate::view;

pub async fn feed(
    State(state): State<AppState>,
    Path((did, pub_rkey)): Path<(String, String)>,
) -> Result<Response<Body>, AppError> {
    let (did, identity) = resolve_repo(&state, &did).await?;
    let publication = require_publication(&state, &identity, &pub_rkey).await?;
    let page = state.visit_listing(&identity, &publication, None).await?;

    let items = page
        .items
        .iter()
        .map(|visit_doc| Item {
            title: view::document_headline(Some(visit_doc), &visit_doc.document().title),
            link: view::canonical_url(
                &state,
                &did,
                &pub_rkey,
                &visit_doc.record,
                &publication.value,
            ),
            description: view::summary(visit_doc),
            published: visit_doc.document().published_at.timestamp(),
            image: state.absolute(&paths::cover_og(&did, visit_doc.rkey())),
        })
        .collect();
    let channel = Channel {
        title: publication.value.name.clone(),
        link: publication.value.base_url().to_owned(),
        description: publication.value.description.clone().unwrap_or_default(),
        self_url: state.absolute(&paths::feed(&did, &pub_rkey)),
        items,
    };
    let body = feed::render(&channel);
    Response::builder()
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/rss+xml; charset=utf-8"),
        )
        .header(
            header::CACHE_CONTROL,
            HeaderValue::from_static("public, max-age=300"),
        )
        .body(Body::from(body))
        .map_err(|e| AppError::Upstream(e.to_string()))
}
