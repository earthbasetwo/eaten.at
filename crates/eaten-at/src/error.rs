//! Request-level errors and their HTTP mapping.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use eaten_at_atproto::identity::{DidError, IdentityError};
use eaten_at_atproto::repo::RepoError;
use eaten_at_web::layout::{self, Page};
use maud::html;

/// Anything a handler can fail with, rendered as an HTML status page.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("upstream failure: {0}")]
    Upstream(String),
}

impl AppError {
    fn status(&self) -> StatusCode {
        match self {
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::Upstream(_) => StatusCode::BAD_GATEWAY,
        }
    }

    fn heading(&self) -> &'static str {
        match self {
            Self::BadRequest(_) => "That request doesn't make sense",
            Self::NotFound(_) => "Not found",
            Self::Upstream(_) => "Couldn't reach the author's server",
        }
    }

    /// What the reader sees. Upstream details are logged, not shown.
    fn detail(&self) -> String {
        match self {
            Self::BadRequest(msg) | Self::NotFound(msg) => msg.clone(),
            Self::Upstream(_) => "The data lives on the author's own server, and it did not answer. Try again in a moment.".to_owned(),
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = self.status();
        if status.is_server_error() {
            tracing::warn!(error = %self, "request failed");
        } else {
            tracing::debug!(error = %self, "request rejected");
        }
        let page = layout::render(&Page {
            title: &[self.heading()],
            main: html! {
                div.page-head {
                    p.kicker { "Error " (status.as_u16()) }
                    h1 { (self.heading()) }
                }
                p { (self.detail()) }
                p.actions { a.button-link href="/" { "← Back to the start" } }
            },
            ..Page::default()
        });
        (status, page).into_response()
    }
}

impl From<DidError> for AppError {
    fn from(err: DidError) -> Self {
        Self::BadRequest(err.to_string())
    }
}

impl From<IdentityError> for AppError {
    fn from(err: IdentityError) -> Self {
        match err {
            IdentityError::DidNotFound(_)
            | IdentityError::HandleNotFound(_)
            | IdentityError::HandleNotClaimed { .. }
            | IdentityError::AmbiguousDns(_) => Self::NotFound(err.to_string()),
            IdentityError::NoPds(_)
            | IdentityError::MalformedDocument { .. }
            | IdentityError::InsecureEndpoint(_)
            | IdentityError::UpstreamStatus { .. }
            | IdentityError::Http(_) => Self::Upstream(err.to_string()),
        }
    }
}

impl From<RepoError> for AppError {
    fn from(err: RepoError) -> Self {
        match err {
            RepoError::RecordNotFound | RepoError::RepoNotFound => Self::NotFound(err.to_string()),
            RepoError::Xrpc { .. } | RepoError::Decode { .. } | RepoError::Http(_) => {
                Self::Upstream(err.to_string())
            }
        }
    }
}
