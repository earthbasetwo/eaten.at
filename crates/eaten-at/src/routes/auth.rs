//! Sign-in and sign-out: `/login`, `/oauth/callback`, `/logout`, and the
//! OAuth client metadata document (plan §5.4, §5.6).
//!
//! Sign-in happens on the site's own origin only. A request that arrives
//! under any other host (a publication subdomain, later) is sent to the
//! bare domain, because the OAuth client is bound to one origin.

use axum::extract::{Query, State};
use axum::http::header::{CACHE_CONTROL, HOST, LOCATION, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Redirect, Response};
use axum::Form;
use eaten_at_atproto::identity::{Did, Handle};
use eaten_at_atproto::oauth::{CallbackParams, OAuthError, METADATA_PATH};
use eaten_at_web::assets::{COMBOBOX_SCRIPT, HANDLE_TYPEAHEAD_SCRIPT};
use eaten_at_web::components::HANDLE_HINT;
use eaten_at_web::layout::{self, Page};
use maud::{html, Markup};
use serde::Deserialize;
use url::Url;

use crate::auth::{CurrentUser, RequireUser, SessionCookie};
use crate::security::{self, Nonce};
use crate::state::AppState;

/// Where a sign-in should land afterwards. Only site-local paths are
/// honoured, so the callback can never be turned into an open redirect.
fn site_local(path: &str) -> Option<String> {
    let trimmed = path.trim();
    (trimmed.starts_with('/') && !trimmed.starts_with("//") && !trimmed.contains('\\'))
        .then(|| trimmed.to_owned())
}

/// Whether the request came in under our own host. A missing `Host` is
/// treated as ours: there is nothing else it could be.
fn is_bare_origin(state: &AppState, headers: &HeaderMap) -> bool {
    headers
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .is_none_or(|host| host.eq_ignore_ascii_case(&state.public_host()))
}

/// Bounce to the same path on the bare origin.
fn to_bare_origin(state: &AppState, path: &str) -> Response {
    Redirect::to(&state.absolute(path)).into_response()
}

/// The sign-in form, with an optional error and the handle as typed.
/// The handle island suggests from the `AppView` as the author types
/// (plan 10); the response's policy has to allow that, which
/// [`login_response`] does.
fn login_page(
    state: &AppState,
    nonce: &Nonce,
    handle: &str,
    return_to: &str,
    error: Option<&str>,
) -> Markup {
    let appview = state.appview_origin();
    layout::render(&Page {
        title: &["Sign in"],
        nonce: Some(nonce.0.clone()),
        scripts: vec![COMBOBOX_SCRIPT, HANDLE_TYPEAHEAD_SCRIPT],
        main: html! {
            div.page-head {
                p.kicker { "Sign in" }
                h1 { "Sign in with your AT Protocol account" }
                p.lede {
                    "Enter your handle. Your account's server will ask you to approve "
                    "eaten.at, then send you back here."
                }
            }
            form.lookup action="/login" method="post" {
                label.kicker.lookup-label for="handle" { "Your handle" }
                div.lookup-row {
                    input #handle name="handle" type="text" inputmode="url" autocomplete="username"
                        placeholder="Start typing your handle…" value=(handle)
                        data-typeahead=(appview)
                        aria-describedby=[error.map(|_| "handle-error")] required;
                    button type="submit" { "Continue" }
                }
                p.meta.field-hint { (HANDLE_HINT) }
                @if !return_to.is_empty() {
                    input type="hidden" name="return_to" value=(return_to);
                }
                @if let Some(error) = error {
                    p.form-error #handle-error role="alert" { (error) }
                }
            }
        },
        ..Page::default()
    })
}

/// The sign-in page as a response whose policy lets the island reach
/// the `AppView`.
fn login_response(
    state: &AppState,
    nonce: &Nonce,
    status: StatusCode,
    handle: &str,
    return_to: &str,
    error: Option<&str>,
) -> Response {
    let page = login_page(state, nonce, handle, return_to, error);
    let mut response = (status, page).into_response();
    security::allow_connect(&mut response, nonce, &state.appview_origin());
    response
}

/// A page that immediately moves on to the authorization server.
///
/// A redirect straight from the form post would be blocked by the
/// stylesheet's `form-action 'self'` in browsers that apply it to the
/// redirect chain; a page-level refresh is a navigation, which they allow.
fn continue_page(url: &Url) -> Markup {
    let host = url.host_str().unwrap_or_default();
    layout::render(&Page {
        title: &["Signing in"],
        head: html! {
            meta http-equiv="refresh" content=(format!("0;url={url}"));
        },
        main: html! {
            div.page-head {
                p.kicker { "Signing in" }
                h1 { "Continuing to " (host) }
                p.lede { "Your account's server will ask you to approve eaten.at, then send you back here." }
            }
            p.actions { a.button href=(url.as_str()) { "Continue" } }
        },
        ..Page::default()
    })
}

/// A page for a sign-in that did not complete.
fn failed_page(status: StatusCode, heading: &str, detail: &str) -> Response {
    let page = layout::render(&Page {
        title: &["Sign in"],
        main: html! {
            div.page-head {
                p.kicker { "Sign in" }
                h1 { (heading) }
            }
            p { (detail) }
            p.actions { a.button-link href="/login" { "← Try again" } }
        },
        ..Page::default()
    });
    (status, page).into_response()
}

#[derive(Debug, Deserialize)]
pub struct LoginQuery {
    #[serde(default)]
    return_to: String,
}

/// `GET /login` — the form. A signed-in user is sent home.
pub async fn login_form(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<LoginQuery>,
    CurrentUser(user): CurrentUser,
    nonce: Nonce,
) -> Response {
    if !is_bare_origin(&state, &headers) {
        return to_bare_origin(&state, "/login");
    }
    if user.is_some() {
        return Redirect::to("/").into_response();
    }
    login_response(
        &state,
        &nonce,
        StatusCode::OK,
        "",
        site_local(&query.return_to).as_deref().unwrap_or(""),
        None,
    )
}

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    #[serde(default)]
    handle: String,
    #[serde(default)]
    return_to: String,
}

/// `POST /login` — resolve the handle and start the flow.
pub async fn login_start(
    State(state): State<AppState>,
    headers: HeaderMap,
    nonce: Nonce,
    Form(form): Form<LoginForm>,
) -> Response {
    if !is_bare_origin(&state, &headers) {
        return to_bare_origin(&state, "/login");
    }
    let return_to = site_local(&form.return_to);
    let scopes = scopes_for_sign_in(&state, &form.handle).await;
    match state
        .oauth()
        .login_url_scoped(&form.handle, &scopes, return_to.clone())
        .await
    {
        Ok(url) => continue_page(&url).into_response(),
        Err(err) => {
            let (status, message) = match &err {
                OAuthError::InvalidInput(_) => (
                    StatusCode::BAD_REQUEST,
                    "That doesn't look like a handle. Try something like alice.bsky.social.",
                ),
                OAuthError::AccountNotFound(_) => (
                    StatusCode::NOT_FOUND,
                    "No AT Protocol account was found for that handle.",
                ),
                _ => {
                    tracing::warn!(error = %err, "could not start sign-in");
                    (
                        StatusCode::BAD_GATEWAY,
                        "Your account's server couldn't be reached. Try again in a moment.",
                    )
                }
            };
            login_response(
                &state,
                &nonce,
                status,
                &form.handle,
                return_to.as_deref().unwrap_or(""),
                Some(message),
            )
        }
    }
}

/// What a sign-in asks for. The sign-in set (§4.4), unless the account
/// already granted this app posting on an earlier sign-in: a new
/// authorization replaces the stored session for the DID, so asking for
/// the subset would silently drop the grant and the next crosspost would
/// ask for it all over again. An account that opted in stays opted in.
async fn scopes_for_sign_in(state: &AppState, input: &str) -> Vec<String> {
    let trimmed = input.trim().trim_start_matches('@');
    let did = match Did::parse(trimmed) {
        Ok(did) => Some(did),
        Err(_) => match Handle::parse(trimmed) {
            Ok(handle) => state.lookup_handle(&handle).await.ok().flatten(),
            Err(_) => None,
        },
    };
    let granted = match did {
        Some(did) => state.oauth().granted_scopes(&did).await.unwrap_or_default(),
        None => Vec::new(),
    };
    if crate::auth::post_permission(&granted).create {
        crate::auth::crosspost_scopes()
    } else {
        crate::auth::login_scopes()
    }
}

#[derive(Debug, Deserialize)]
pub struct BlueskyForm {
    #[serde(default)]
    return_to: String,
}

/// `POST /login/bluesky` — ask the signed-in user's server for the
/// crosspost scope on top of what sign-in granted (plan §5.7). The same
/// flow as sign-in with a larger scope set; the callback replaces the
/// stored session with the new grant and comes back to `return_to`.
pub async fn authorize_bluesky(
    State(state): State<AppState>,
    headers: HeaderMap,
    RequireUser(did): RequireUser,
    Form(form): Form<BlueskyForm>,
) -> Response {
    if !is_bare_origin(&state, &headers) {
        return to_bare_origin(&state, "/settings");
    }
    let return_to = site_local(&form.return_to);
    match state
        .oauth()
        .login_url_scoped(did.as_str(), &crate::auth::crosspost_scopes(), return_to)
        .await
    {
        Ok(url) => continue_page(&url).into_response(),
        Err(err) => {
            tracing::warn!(%did, error = %err, "could not start the Bluesky authorization");
            failed_page(
                StatusCode::BAD_GATEWAY,
                "Bluesky permission didn't start",
                "Your account's server couldn't be reached. Try again in a moment.",
            )
        }
    }
}

/// `GET /oauth/callback` — finish the flow and start a browser session.
pub async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<CallbackParams>,
) -> Response {
    if !is_bare_origin(&state, &headers) {
        return to_bare_origin(&state, "/login");
    }
    let done = match state.oauth().callback(params).await {
        Ok(done) => done,
        Err(OAuthError::Denied { error, description }) => {
            tracing::debug!(
                error,
                ?description,
                "sign-in refused by the authorization server"
            );
            return failed_page(
                StatusCode::OK,
                "Sign-in was cancelled",
                "Your account's server did not approve eaten.at. Nothing was changed.",
            );
        }
        Err(err) => {
            tracing::warn!(error = %err, "sign-in callback failed");
            return failed_page(
                StatusCode::BAD_GATEWAY,
                "Sign-in didn't complete",
                "Something went wrong between here and your account's server. Try signing in again.",
            );
        }
    };
    let token = match state.sessions().create(&done.did).await {
        Ok(token) => token,
        Err(err) => {
            tracing::error!(error = %err, "could not create a session");
            return failed_page(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Sign-in didn't complete",
                "The sign-in worked, but a session could not be saved. Try again.",
            );
        }
    };
    let target = done
        .app_state
        .as_deref()
        .and_then(site_local)
        .unwrap_or_else(|| "/".to_owned());
    let mut response = Redirect::to(&target).into_response();
    response
        .headers_mut()
        .append(SET_COOKIE, state.cookie().set(&token));
    response
}

/// `POST /logout` — end the browser session and drop the OAuth tokens.
pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if let Some(token) = SessionCookie::read(&headers) {
        if let Some(did) = state.sessions().lookup(&token).await {
            if let Err(err) = state.oauth().logout(&did).await {
                tracing::warn!(%did, error = %err, "could not drop the OAuth session");
            }
        }
        if let Err(err) = state.sessions().delete(&token).await {
            tracing::warn!(error = %err, "could not delete the browser session");
        }
    }
    let mut response = Redirect::to("/").into_response();
    response
        .headers_mut()
        .append(SET_COOKIE, state.cookie().clear());
    response
}

/// `GET /client-metadata.json` — what authorization servers fetch to
/// learn who we are. Served on the bare origin only, where the `client_id`
/// says it lives.
pub async fn client_metadata(State(state): State<AppState>, headers: HeaderMap) -> Response {
    if !is_bare_origin(&state, &headers) {
        return crate::error::AppError::NotFound(format!("{METADATA_PATH} is not served here"))
            .into_response();
    }
    let mut response = Json(state.oauth().metadata_document()).into_response();
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=3600"),
    );
    response
}

/// For tests: the `Location` of a redirect response.
pub fn location(response: &Response) -> Option<&str> {
    response
        .headers()
        .get(LOCATION)
        .and_then(|v| v.to_str().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_site_local_paths_are_return_targets() {
        assert_eq!(site_local("/write").as_deref(), Some("/write"));
        assert_eq!(
            site_local(" /at/did:plc:x/ ").as_deref(),
            Some("/at/did:plc:x/")
        );
        assert_eq!(site_local(""), None);
        assert_eq!(site_local("//evil.example"), None);
        assert_eq!(site_local("https://evil.example"), None);
        assert_eq!(site_local("/\\evil.example"), None);
    }
}
