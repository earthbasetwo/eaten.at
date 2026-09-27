//! `/at/{did}/{pub_rkey}/feed.xml` — RSS for a publication.

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, Response};

use super::{require_publication, resolve_repo};
use crate::error::AppError;
use crate::feed::{self, Channel, Item};
use crate::img::Size;
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

    // An enclosure says how long it is, so each cover is rendered (or
    // read from the image cache). Downloads overlap; image preparation
    // shares the proxy/upload budget through the end of each encoding.
    let mut covers = tokio::task::JoinSet::new();
    for (i, visit_doc) in page.items.iter().enumerate() {
        let (state, identity, visit_doc) = (state.clone(), identity.clone(), visit_doc.clone());
        covers.spawn(async move {
            let cover = state.cover_rendition(&identity, &visit_doc, Size::Og).await;
            (i, cover.jpeg.len())
        });
    }
    let mut lengths = vec![0; page.items.len()];
    while let Some(done) = covers.join_next().await {
        if let Ok((i, length)) = done {
            lengths[i] = length;
        }
    }
    let items = page
        .items
        .iter()
        .zip(lengths)
        .map(|(visit_doc, image_length)| Item {
            title: visit_doc.document().title.clone(),
            link: view::canonical_url(
                &state,
                &did,
                &pub_rkey,
                &visit_doc.record,
                &publication.value,
            ),
            description: view::summary(visit_doc),
            published: visit_doc.document().published_at.timestamp(),
            image: state.absolute(&paths::cover_og(
                &did,
                visit_doc.rkey(),
                &visit_doc.record.cid,
            )),
            image_length,
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
