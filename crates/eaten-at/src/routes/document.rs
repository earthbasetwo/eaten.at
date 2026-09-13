//! `/at/{did}/{pub_rkey}/{doc_rkey}` — one document, rendered in full.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use eaten_at_atproto::lexicon::Document;
use eaten_at_web::components::{comments, tag_links, visit_card, visit_links, CommentsView, Link};
use eaten_at_web::dates::human_date;
use eaten_at_web::layout::{self, Masthead, Page};
use eaten_at_web::markdown;
use eaten_at_web::meta;
use maud::{html, Markup, PreEscaped};

use crate::auth::CurrentUser;

use super::{require_publication, resolve_repo};
use crate::error::AppError;
use crate::model::{body_of, Body, VisitDocument};
use crate::paths;
use crate::security::Nonce;
use crate::state::AppState;
use crate::view;

pub async fn document_page(
    State(state): State<AppState>,
    Path((did, pub_rkey, doc_rkey)): Path<(String, String, String)>,
    headers: HeaderMap,
    nonce: Nonce,
    CurrentUser(viewer): CurrentUser,
) -> Result<Markup, AppError> {
    let (did, identity) = resolve_repo(&state, &did).await?;
    let publication = require_publication(&state, &identity, &pub_rkey).await?;
    let record = state
        .document_in(&identity, &publication, &doc_rkey)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("document {doc_rkey} not found")))?;

    // Labels on the document or its publication gate the page (plan §8).
    // The cover proxy is not gated: unfurlers fetch it without cookies.
    let mut labels = crate::labels::values(publication.value.labels.as_ref());
    labels.extend(crate::labels::values(record.value.labels.as_ref()));
    if !labels.is_empty() && !crate::labels::acknowledged(&headers) {
        return Ok(super::interstitial::page(
            &labels,
            &paths::document(&did, &pub_rkey, &doc_rkey),
            &record.value.title,
        ));
    }

    let title = record.value.title.clone();
    let published = record.value.published_at.as_str().to_owned();
    let body = match body_of(&record.value) {
        Body::Markdown(text) => Some(PreEscaped(markdown::render(text))),
        Body::Plain(text) => Some(html! { pre.plain-body { (text) } }),
        Body::Empty => None,
    };
    let tags = view::tag_links(
        &did,
        &pub_rkey,
        &crate::tags::distinct(record.value.tags.iter().map(String::as_str)),
    );
    // A document that is not a visit is still shown; it just has no card.
    // Readers arriving from a shared link should not hit a 404.
    let visit_doc = VisitDocument::from_record(record.clone());
    let page_meta = view::document_meta(
        &state,
        &did,
        &pub_rkey,
        &record,
        visit_doc.as_ref(),
        &publication.value,
    );
    let theme = view::theme(&publication.value);
    let card = visit_doc
        .as_ref()
        .map(|visit_doc| view::visit_card(&did, visit_doc));
    let thread = comment_thread(&state, &record.value).await;
    let publication_path = paths::publication(&did, &pub_rkey);
    let footer = Footer {
        links: card.as_ref().map_or(&[][..], |c| c.links.as_slice()),
        comments_url: view::bluesky_post_url(&record.value),
        tags: &tags,
        feed_path: paths::feed(&did, &pub_rkey),
        author_path: paths::repo(&did),
        author: view::author_label(&identity),
        // The author, signed in, can edit from the page itself.
        edit_path: (viewer.as_ref() == Some(&did)).then(|| format!("/write/{doc_rkey}")),
    };

    Ok(layout::render(&Page {
        title: &[&page_meta.title, &publication.value.name],
        masthead: Some(Masthead {
            name: &publication.value.name,
            href: &publication_path,
        }),
        theme,
        nonce: Some(nonce.0),
        head: meta::head(&page_meta),
        main: html! {
            article.document {
                p.kicker {
                    time datetime=(published) { (human_date(&published)) }
                }
                h1.doc-title { (title) }
                @if let Some(card) = &card {
                    (visit_card(card))
                }
                @if let Some(body) = body {
                    div.prose { (body) }
                } @else {
                    p.notice { "This document has no readable body." }
                }
                @if let Some(thread) = &thread {
                    (comments(thread))
                }
                (footer.render())
            }
        },
        ..Page::default()
    }))
}

/// The comment thread to show under a document: the replies to the
/// Bluesky post it names (plan §5.7). `None` when it names no post, or
/// the post is gone, blocked, or the `AppView` is not answering; the
/// footer's plain link to the thread stands alone then.
async fn comment_thread(state: &AppState, doc: &Document) -> Option<CommentsView> {
    let uri = view::bluesky_post_uri(doc)?;
    let href = crate::bsky::post_url(uri.did().as_str(), uri.rkey());
    let thread = state.bluesky_thread(&uri).await?;
    Some(view::comments_view(&thread, href))
}

/// What the document footer shows: the place's links, the comment
/// thread, the tags, and the publication's feed and author.
struct Footer<'a> {
    links: &'a [Link],
    comments_url: Option<String>,
    tags: &'a [Link],
    feed_path: String,
    author_path: String,
    author: String,
    edit_path: Option<String>,
}

impl Footer<'_> {
    fn render(&self) -> Markup {
        html! {
            footer.doc-footer {
                @if !self.links.is_empty() || self.comments_url.is_some() {
                    div.doc-footer-row {
                        (visit_links(self.links))
                        @if let Some(url) = &self.comments_url {
                            a.push.comments href=(url) rel="noopener" { "Comments on Bluesky →" }
                        }
                    }
                }
                div.doc-footer-row {
                    (tag_links(self.tags, None))
                    div.quiet-links.push {
                        @if let Some(edit) = &self.edit_path {
                            a href=(edit) { "edit" }
                        }
                        a href=(self.feed_path) rel="alternate" type="application/rss+xml" { "rss" }
                        a href=(self.author_path) rel="author" { (self.author) }
                    }
                }
            }
        }
    }
}
