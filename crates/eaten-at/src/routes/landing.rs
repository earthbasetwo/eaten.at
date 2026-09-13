//! `GET /` — the landing page with the handle lookup form.

use axum::extract::State;
use eaten_at_web::components::lookup_form;
use eaten_at_web::layout::{self, Page};
use maud::{html, Markup};

use crate::auth::CurrentUser;
use crate::paths;
use crate::state::AppState;
use crate::view;

pub async fn landing(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Markup {
    // The signed-in author, named by handle when it resolves.
    let account = match &user {
        Some(did) => {
            let label = match state.identity_for(did).await {
                Ok(Some(identity)) => view::author_label(&identity),
                _ => did.to_string(),
            };
            Some((paths::repo(did), label))
        }
        None => None,
    };
    layout::render(&Page {
        title: &[],
        main: html! {
            div.page-head {
                h1 { "Write-ups, published on the AT Protocol." }
                p.lede {
                    "Authors keep their write-ups in their own repositories. "
                    "This site reads them and sets each publication as a small journal: "
                    "the subject, the cover, and the words."
                }
            }
            (lookup_form("", None))
            p.meta.landing-note {
                "Any AT Protocol handle works, Bluesky handles included. "
                "Every write-up stays in its author's repository; this site only reads."
            }
            div.meta.account {
                @if let Some((href, label)) = &account {
                    span { "Signed in as " a href=(href) { (label) } }
                    a href="/write" { "Write" }
                    a href="/settings" { "Settings" }
                    form.inline-form method="post" action="/logout" {
                        button.link-button type="submit" { "Sign out" }
                    }
                } @else {
                    span { "Have an account? " a href="/login" { "Sign in" } " to write here." }
                }
            }
        },
        ..Page::default()
    })
}
