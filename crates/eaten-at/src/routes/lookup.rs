//! Handle lookup: `/lookup?handle=…` and `/@{handle}`. Both are redirects;
//! nothing is rendered under a handle-addressed URL (plan D20).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use eaten_at_atproto::identity::Handle;
use eaten_at_web::components::{lookup_form, LookupForm};
use eaten_at_web::layout::{self, Page};
use maud::html;
use serde::Deserialize;

use crate::error::AppError;
use crate::paths;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LookupQuery {
    #[serde(default)]
    handle: String,
}

/// `GET /lookup?handle=` — validate and bounce to `/@{handle}`.
pub async fn lookup(Query(query): Query<LookupQuery>) -> Response {
    match Handle::parse(&query.handle) {
        Ok(handle) => Redirect::to(&paths::handle_lookup(&handle)).into_response(),
        Err(err) => {
            let page = layout::render(&Page {
                title: &["Lookup"],
                main: html! {
                    div.page-head {
                        p.kicker { "Lookup" }
                        h1 { "That doesn't look like a handle" }
                    }
                    (lookup_form(&LookupForm {
                        value: &query.handle,
                        error: Some(&err.to_string()),
                        ..LookupForm::default()
                    }))
                },
                ..Page::default()
            });
            (StatusCode::BAD_REQUEST, page).into_response()
        }
    }
}

/// `GET /@{handle}` — 302 to the DID-addressed repo route.
pub async fn handle_root(
    State(state): State<AppState>,
    Path(handle): Path<String>,
) -> Result<Redirect, AppError> {
    let did = resolve(&state, &handle).await?;
    Ok(Redirect::to(&paths::repo(&did)))
}

/// `GET /@{handle}/{*rest}` — 302 to the same path under the DID.
pub async fn handle_rest(
    State(state): State<AppState>,
    Path((handle, rest)): Path<(String, String)>,
) -> Result<Redirect, AppError> {
    let did = resolve(&state, &handle).await?;
    Ok(Redirect::to(&format!("/at/{did}/{rest}")))
}

async fn resolve(state: &AppState, raw: &str) -> Result<eaten_at_atproto::identity::Did, AppError> {
    let handle = Handle::parse(raw).map_err(|e| AppError::BadRequest(e.to_string()))?;
    state
        .lookup_handle(&handle)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("no atproto identity found for {handle}")))
}
