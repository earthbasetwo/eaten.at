//! Handle lookup: `/lookup?handle=…` and `/@{handle}`. Both are redirects;
//! nothing is rendered under a handle-addressed URL (plan D20).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use eaten_at_atproto::identity::Handle;
use eaten_at_web::assets::{COMBOBOX_SCRIPT, HANDLE_TYPEAHEAD_SCRIPT};
use eaten_at_web::components::{lookup_form, LookupForm};
use eaten_at_web::layout::{self, Page};
use maud::html;
use serde::Deserialize;

use crate::error::AppError;
use crate::paths;
use crate::security::{self, Nonce};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LookupQuery {
    #[serde(default)]
    handle: String,
}

/// `GET /lookup?handle=` — validate and bounce to `/@{handle}`.
///
/// Asked for with no handle at all it is not an error: that is where the
/// landing page's "Look up a friend" leads without the script that would
/// have turned it into a field in place, so the page it lands on is the
/// form itself, asking.
pub async fn lookup(
    State(state): State<AppState>,
    Query(query): Query<LookupQuery>,
    nonce: Nonce,
) -> Response {
    let raw = query.handle.trim();
    if raw.is_empty() {
        return form_response(&state, &nonce, StatusCode::OK, "Whose digests?", "", None);
    }
    match Handle::parse(raw) {
        Ok(handle) => Redirect::to(&paths::handle_lookup(&handle)).into_response(),
        Err(err) => form_response(
            &state,
            &nonce,
            StatusCode::BAD_REQUEST,
            "That doesn't look like a handle",
            raw,
            Some(&err.to_string()),
        ),
    }
}

/// The lookup page: the heading, the form, and the error when there is
/// one. Its policy lets the handle island reach the `AppView`.
fn form_response(
    state: &AppState,
    nonce: &Nonce,
    status: StatusCode,
    heading: &str,
    value: &str,
    error: Option<&str>,
) -> Response {
    let appview = state.appview_origin();
    let page = layout::render(&Page {
        title: &["Lookup"],
        nonce: Some(nonce.0.clone()),
        scripts: vec![COMBOBOX_SCRIPT, HANDLE_TYPEAHEAD_SCRIPT],
        main: html! {
            div.page-head {
                p.kicker { "Lookup" }
                h1 { (heading) }
            }
            (lookup_form(&LookupForm {
                value,
                error,
                typeahead: Some(&appview),
                ..LookupForm::default()
            }))
        },
        ..Page::default()
    });
    let mut response = (status, page).into_response();
    security::allow_connect(&mut response, nonce, &appview);
    response
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
