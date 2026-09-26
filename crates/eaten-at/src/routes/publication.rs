//! `/at/{did}/` and `/at/{did}/{pub_rkey}/`.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use eaten_at_web::components::{listing, publication_chooser, tag_links};
use eaten_at_web::layout::{self, pagination, Page};
use eaten_at_web::meta;
use maud::html;
use serde::Deserialize;

use super::{require_publication, resolve_repo};
use crate::error::AppError;
use crate::paths;
use crate::read::PublicationChoice;
use crate::security::Nonce;
use crate::state::AppState;
use crate::view;

/// `GET /at/{did}/` — redirect to the chosen publication, or show a chooser.
pub async fn repo_index(
    State(state): State<AppState>,
    Path(did): Path<String>,
) -> Result<Response, AppError> {
    let (did, identity) = resolve_repo(&state, &did).await?;
    let author = view::author_label(&identity);
    match state.choose_publication(&identity).await? {
        PublicationChoice::Chosen(publication) => {
            Ok(Redirect::to(&paths::publication(&did, publication.rkey())).into_response())
        }
        PublicationChoice::Choose(publications) => {
            let views: Vec<_> = publications
                .iter()
                .map(|p| view::publication_view(&did, p))
                .collect();
            Ok(layout::render(&Page {
                title: &[&author],
                main: html! {
                    div.page-head {
                        p.kicker { "Feeds" }
                        h1 { (author) }
                        p.lede { "This account has several feeds. Pick one to read." }
                    }
                    (publication_chooser(&views))
                },
                ..Page::default()
            })
            .into_response())
        }
        PublicationChoice::None => Ok(layout::render(&Page {
            title: &[&author],
            main: html! {
                div.page-head {
                    p.kicker { "Feeds" }
                    h1 { (author) }
                }
                p.empty { "This account has no feeds yet." }
            },
            ..Page::default()
        })
        .into_response()),
    }
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    cursor: Option<String>,
}

/// `GET /at/{did}/{pub_rkey}/` — the publication's write-ups.
pub async fn publication_page(
    State(state): State<AppState>,
    Path((did, pub_rkey)): Path<(String, String)>,
    Query(query): Query<ListQuery>,
    headers: HeaderMap,
    nonce: Nonce,
) -> Result<Response, AppError> {
    let (did, identity) = resolve_repo(&state, &did).await?;
    let publication = require_publication(&state, &identity, &pub_rkey).await?;
    let labels = crate::labels::values(publication.value.labels.as_ref());
    if !labels.is_empty() && !crate::labels::acknowledged(&headers) {
        return Ok(super::interstitial::page(
            &labels,
            &paths::publication(&did, &pub_rkey),
            &publication.value.name,
        )
        .into_response());
    }
    let cursor = query.cursor.as_deref().filter(|c| !c.is_empty());
    let page = state.visit_listing(&identity, &publication, cursor).await?;

    let items: Vec<_> = page
        .items
        .iter()
        .map(|visit_doc| view::listing_item(&did, &pub_rkey, visit_doc))
        .collect();
    let tags = view::tag_links(
        &did,
        &pub_rkey,
        &crate::tags::distinct(
            page.items
                .iter()
                .flat_map(|a| a.document().tags.iter().map(String::as_str)),
        ),
    );
    let author = view::author_label(&identity);
    let base = paths::publication(&did, &pub_rkey);
    let name = &publication.value.name;

    Ok(layout::render(&Page {
        title: &[name],
        theme: view::theme(&publication.value),
        nonce: Some(nonce.0),
        head: meta::head(&view::publication_meta(&state, &did, &pub_rkey, &publication.value)),
        main: html! {
            header.nameplate {
                h1.nameplate-name { (name) }
                ul.dateline {
                    li { "by " a href=(paths::repo(&did)) rel="author" { (author) } }
                    li { a href=(publication.value.url) rel="noopener" { (view::display_url(&publication.value.url)) } }
                    li { a href=(paths::feed(&did, &pub_rkey)) rel="alternate" type="application/rss+xml" { "rss" } }
                }
                @if let Some(description) = &publication.value.description {
                    p.lede { (description) }
                }
                (tag_links(&tags, None))
            }
            @if items.is_empty() {
                @if page.truncated {
                    p.empty { "No digests among the most recent documents. There may be older ones." }
                } @else if cursor.is_some() {
                    p.empty { "No more digests." }
                } @else {
                    p.empty { "No digests yet." }
                }
            } @else {
                (listing(&items))
                @if page.truncated {
                    p.notice { "Showing recent digests; this feed also has many other documents." }
                }
            }
            (pagination(&base, page.next_cursor.as_deref(), cursor.is_none()))
        },
        ..Page::default()
    })
    .into_response())
}

/// `GET /at/{did}/{pub_rkey}/publication.json` — the publication record
/// as JSON. On a hosted subdomain this is what
/// `/.well-known/site.standard.publication` serves, which is how a
/// verifier ties the origin to the record (plan §5.5).
pub async fn publication_json(
    State(state): State<AppState>,
    Path((did, pub_rkey)): Path<(String, String)>,
) -> Result<Response, AppError> {
    let (_, identity) = resolve_repo(&state, &did).await?;
    let publication = require_publication(&state, &identity, &pub_rkey).await?;
    let mut value = serde_json::to_value(&publication.value).unwrap_or_default();
    value["$type"] =
        serde_json::Value::String(eaten_at_atproto::lexicon::PUBLICATION_NSID.to_owned());
    let mut response = axum::Json(value).into_response();
    response.headers_mut().insert(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("public, max-age=300"),
    );
    Ok(response)
}

/// `GET /at/{did}/{pub_rkey}/tagged/{tag}` — write-ups in this
/// publication carrying `tag`.
pub async fn tagged_page(
    State(state): State<AppState>,
    Path((did, pub_rkey, tag)): Path<(String, String, String)>,
    Query(query): Query<ListQuery>,
    nonce: Nonce,
) -> Result<Response, AppError> {
    let (did, identity) = resolve_repo(&state, &did).await?;
    let publication = require_publication(&state, &identity, &pub_rkey).await?;
    let cursor = query.cursor.as_deref().filter(|c| !c.is_empty());
    let page = state
        .tagged_listing(&identity, &publication, &tag, cursor)
        .await?;

    // Display the tag as the first matching document spelled it.
    let display = page
        .items
        .iter()
        .flat_map(|a| a.document().tags.iter())
        .find(|t| crate::tags::matches(t, &tag))
        .cloned()
        .unwrap_or_else(|| tag.trim().to_owned());
    let items: Vec<_> = page
        .items
        .iter()
        .map(|visit_doc| view::listing_item(&did, &pub_rkey, visit_doc))
        .collect();
    let base = paths::tagged(&did, &pub_rkey, &tag);
    let name = &publication.value.name;

    Ok(layout::render(&Page {
        title: &[&format!("Tagged {display}"), name],
        theme: view::theme(&publication.value),
        nonce: Some(nonce.0),
        main: html! {
            header.page-head {
                p.kicker { "Tag" }
                h1 { "Tagged “" (display) "”" }
                p.meta { "Digests in this feed only; tags are not shared across feeds." }
            }
            @if items.is_empty() {
                @if page.truncated {
                    p.empty { "Nothing tagged “" (display) "” among the most recent documents. There may be older ones." }
                } @else {
                    p.empty { "Nothing in this feed is tagged “" (display) "”." }
                }
            } @else {
                (listing(&items))
            }
            (pagination(&base, page.next_cursor.as_deref(), cursor.is_none()))
        },
        ..Page::default()
    })
    .into_response())
}
