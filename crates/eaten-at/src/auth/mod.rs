//! Who is signed in.
//!
//! Sign-in itself is atproto OAuth, done by `eaten_at_atproto::oauth`.
//! This module owns what surrounds it: the scopes we ask for (plan §4.4),
//! the SQLite tables behind the OAuth client, the browser session (a
//! random token in a cookie, mapped to a DID), and the extractor handlers
//! use to learn who is asking.

mod cookie;
mod sessions;
mod store;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::response::{IntoResponse, Redirect, Response};

use eaten_at_atproto::identity::Did;

use crate::state::AppState;

pub use cookie::{SessionCookie, COOKIE_NAME};
pub use sessions::{WebSessions, SESSION_LIFETIME};
pub use store::SqliteOAuthStore;

/// The scopes requested at sign-in, exactly as they go on the wire.
///
/// Granular scopes, no permission set (D14). A `repo:` scope grants
/// create, update, and delete unless narrowed with `action=`; only
/// documents need all three. `blob:` cannot live in a permission set and
/// is requested on its own. Checked against the `oauth-scopes` package
/// in the atproto checkout on 2026-09-09. Bluesky posting is added
/// later, when the user first enables crossposting (§5.7).
pub const SCOPES: &[&str] = &[
    "atproto",
    "repo:site.standard.publication?action=create&action=update",
    "repo:site.standard.document",
    "repo:at.eaten.preferences?action=create&action=update",
    "blob:image/*",
];

/// The scope asked for when the user first enables crossposting (§5.7):
/// create a post, and delete it again when the write-up goes.
pub const CROSSPOST_SCOPE: &str = "repo:app.bsky.feed.post?action=create&action=delete";

/// Every scope the client declares in its metadata: the sign-in set plus
/// the crosspost scope, so the latter can be requested later without
/// changing who the client is.
pub fn all_scopes() -> Vec<String> {
    SCOPES
        .iter()
        .map(|s| (*s).to_owned())
        .chain(std::iter::once(CROSSPOST_SCOPE.to_owned()))
        .collect()
}

/// The sign-in scopes plus the crosspost scope, for the upgrade
/// authorization.
pub fn crosspost_scopes() -> Vec<String> {
    all_scopes()
}

/// The sign-in scopes as owned strings.
pub fn login_scopes() -> Vec<String> {
    SCOPES.iter().map(|s| (*s).to_owned()).collect()
}

/// What a granted scope list lets us do with `app.bsky.feed.post`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PostPermission {
    pub create: bool,
    pub delete: bool,
}

/// Read the `repo:` grants for Bluesky posts out of a scope list. A
/// `repo:` scope without `action=` grants every action; `repo:*` and the
/// legacy `transition:generic` grant everything.
pub fn post_permission(scopes: &[String]) -> PostPermission {
    let mut permission = PostPermission::default();
    for scope in scopes {
        let (name, query) = scope.split_once('?').unwrap_or((scope, ""));
        let all = match name {
            "transition:generic" | "repo:*" => true,
            "repo:app.bsky.feed.post" => query.is_empty(),
            _ => continue,
        };
        let actions: Vec<&str> = query
            .split('&')
            .filter_map(|pair| pair.strip_prefix("action="))
            .collect();
        permission.create |= all || actions.contains(&"create");
        permission.delete |= all || actions.contains(&"delete");
    }
    permission
}

/// The signed-in DID, if any. Never fails: a missing, expired, or
/// unknown cookie is simply nobody.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentUser(pub Option<Did>);

impl FromRequestParts<AppState> for CurrentUser {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let Some(token) = SessionCookie::read(&parts.headers) else {
            return Ok(Self(None));
        };
        Ok(Self(state.sessions().lookup(&token).await))
    }
}

/// The signed-in DID, or a redirect to the sign-in page that comes back
/// to the requested path afterwards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequireUser(pub Did);

impl FromRequestParts<AppState> for RequireUser {
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Response> {
        let CurrentUser(user) = CurrentUser::from_request_parts(parts, state)
            .await
            .unwrap_or(CurrentUser(None));
        if let Some(did) = user {
            return Ok(Self(did));
        }
        let target = format!(
            "/login?return_to={}",
            eaten_at_web::layout::urlencoding(parts.uri.path())
        );
        Err(Redirect::to(&target).into_response())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scopes(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn post_permission_reads_actions() {
        assert_eq!(
            post_permission(&scopes(SCOPES)),
            PostPermission::default(),
            "sign-in grants no posting"
        );
        assert_eq!(
            post_permission(&all_scopes()),
            PostPermission {
                create: true,
                delete: true
            }
        );
        assert_eq!(
            post_permission(&scopes(&[
                "atproto",
                "repo:app.bsky.feed.post?action=create"
            ])),
            PostPermission {
                create: true,
                delete: false
            }
        );
        for full in ["repo:app.bsky.feed.post", "repo:*", "transition:generic"] {
            assert_eq!(
                post_permission(&scopes(&["atproto", full])),
                PostPermission {
                    create: true,
                    delete: true
                },
                "{full}"
            );
        }
        assert_eq!(
            post_permission(&scopes(&["repo:app.bsky.feed.like"])),
            PostPermission::default()
        );
    }

    #[test]
    fn declared_scopes_end_with_the_crosspost_scope() {
        let all = all_scopes();
        assert_eq!(all.len(), SCOPES.len() + 1);
        assert_eq!(all.last().map(String::as_str), Some(CROSSPOST_SCOPE));
        assert_eq!(login_scopes().len(), SCOPES.len());
    }
}
