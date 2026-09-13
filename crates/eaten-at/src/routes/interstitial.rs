//! The label interstitial and its acknowledgement form.

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use eaten_at_web::layout::{self, Page};
use maud::{html, Markup};
use serde::Deserialize;

use crate::labels;

/// Render the interstitial for `labels`, offering to continue to `path`.
pub fn page(labels: &[String], path: &str, title: &str) -> Markup {
    layout::render(&Page {
        title: &["Content warning", title],
        main: html! {
            article.interstitial {
                div.page-head {
                    p.kicker { "Content warning" }
                    h1 { "Before you read this" }
                }
                p {
                    "The author marked this as "
                    @for (i, label) in labels.iter().enumerate() {
                        @if i > 0 && i + 1 == labels.len() { " and " } @else if i > 0 { ", " }
                        strong { (labels::describe(label)) }
                    }
                    "."
                }
                p.meta { "Labels are set by the author, not by this site. Continuing shows labeled content for the rest of your visit." }
                form.actions method="post" action="/labels/continue" {
                    input type="hidden" name="return" value=(path);
                    button type="submit" { "Show it" }
                    a.button-link href="/" { "← Go back" }
                }
            }
        },
        ..Page::default()
    })
}

#[derive(Debug, Deserialize)]
pub struct ContinueForm {
    #[serde(rename = "return")]
    return_to: String,
}

/// `POST /labels/continue` — set the session cookie and go back.
pub async fn acknowledge(headers: HeaderMap, Form(form): Form<ContinueForm>) -> Response {
    // Only site-local paths may be redirected to, never another origin.
    let target = if form.return_to.starts_with('/') && !form.return_to.starts_with("//") {
        form.return_to
    } else {
        "/".to_owned()
    };
    let mut response = Redirect::to(&target).into_response();
    response
        .headers_mut()
        .append(labels::SET_COOKIE_HEADER, labels::ack_cookie(&headers));
    *response.status_mut() = StatusCode::SEE_OTHER;
    response
}
