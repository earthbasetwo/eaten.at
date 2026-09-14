//! `GET /` — signed out, the pitch and the way in, with a handle lookup
//! as the secondary action (plan 09); signed in, the author's account
//! line (plan 11 makes it the author's home).

use axum::extract::State;
use eaten_at_web::components::{lookup_form, LookupForm};
use eaten_at_web::layout::{self, Page};
use maud::{html, Markup};

use crate::auth::CurrentUser;
use crate::paths;
use crate::state::AppState;
use crate::view;

pub async fn landing(State(state): State<AppState>, CurrentUser(user): CurrentUser) -> Markup {
    match user {
        Some(did) => {
            // The signed-in author, named by handle when it resolves.
            let label = match state.identity_for(&did).await {
                Ok(Some(identity)) => view::author_label(&identity),
                _ => did.to_string(),
            };
            signed_in(&paths::repo(&did), &label)
        }
        None => signed_out(),
    }
}

/// The pitch, one primary "Sign in", and the lookup form beneath a
/// hairline as the way to read without signing in.
fn signed_out() -> Markup {
    layout::render(&Page {
        title: &[],
        main: html! {
            div.page-head {
                h1 { "AT where you ate." }
                p.lede {
                    "Your reviews of places to eat, kept in your own AT Protocol "
                    "repository and published as a small journal of your own."
                }
            }
            div.actions.landing-actions {
                a.button href="/login" { "Sign in" }
                span.landing-aside { "with your Bluesky or AT Protocol account" }
            }
            hr.landing-divider;
            (lookup_form(&LookupForm {
                label: "Or read someone's reviews",
                button: "Read",
                primary: false,
                ..LookupForm::default()
            }))
            p.meta.landing-note {
                "Every review stays in its author's repository; this site only reads."
            }
        },
        ..Page::default()
    })
}

/// The page as it was, for a signed-in author, until plan 11.
fn signed_in(repo_path: &str, label: &str) -> Markup {
    layout::render(&Page {
        title: &[],
        main: html! {
            div.page-head {
                h1 { "Write-ups, published on the AT Protocol." }
                p.lede {
                    "Authors keep their write-ups in their own repositories. "
                    "This site reads them and sets each publication as a small journal: "
                    "the place, the visit, and the words."
                }
            }
            (lookup_form(&LookupForm::default()))
            p.meta.landing-note {
                "Any AT Protocol handle works, Bluesky handles included. "
                "Every write-up stays in its author's repository; this site only reads."
            }
            div.meta.account {
                span { "Signed in as " a href=(repo_path) { (label) } }
                a href="/write" { "Write" }
                a href="/settings" { "Settings" }
                form.inline-form method="post" action="/logout" {
                    button.link-button type="submit" { "Sign out" }
                }
            }
        },
        ..Page::default()
    })
}
