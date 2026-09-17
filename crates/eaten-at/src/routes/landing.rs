//! `GET /` — signed out, the pitch and the way in, with a handle lookup
//! as the secondary action (plan 09); signed in, the author's home:
//! one primary "Write a new visit", their publication with its recent
//! write-ups and a way to find one, and settings last (plan 11).

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use eaten_at_atproto::identity::{Did, Identity};
use eaten_at_atproto::lexicon::Publication;
use eaten_at_atproto::repo::Record;
use eaten_at_web::assets::{COMBOBOX_SCRIPT, CONNECT_SCRIPT, FIND_SCRIPT, HANDLE_TYPEAHEAD_SCRIPT};
use eaten_at_web::components::{connect, listing, lookup_form, tag_links, Connect, LookupForm};
use eaten_at_web::layout::{self, Masthead, Page};
use maud::{html, Markup};
use serde::Deserialize;

use crate::auth::CurrentUser;
use crate::error::AppError;
use crate::paths;
use crate::publish::{self, Home};
use crate::security::{self, Nonce};
use crate::state::AppState;
use crate::view;

/// How many recent write-ups the author's home shows.
pub const RECENT: usize = 8;

#[derive(Debug, Deserialize)]
pub struct LandingQuery {
    /// A find over the author's own write-ups (plan 11). Ignored signed
    /// out.
    #[serde(default)]
    q: String,
}

pub async fn landing(
    State(state): State<AppState>,
    CurrentUser(user): CurrentUser,
    Query(query): Query<LandingQuery>,
    nonce: Nonce,
) -> Result<Response, AppError> {
    if let Some(did) = user {
        return signed_in(&state, &did, query.q.trim(), &nonce).await;
    }
    // The handle island calls the AppView from the browser (plan 10),
    // which this page's policy alone allows.
    let appview = state.appview_origin();
    let mut response = signed_out(&nonce, &appview).into_response();
    security::allow_connect(&mut response, &nonce, &appview);
    Ok(response)
}

/// The pitch, one primary "Connect" that becomes the handle field, and
/// the lookup form beneath a hairline as the way to read without
/// signing in.
fn signed_out(nonce: &Nonce, appview: &str) -> Markup {
    layout::render(&Page {
        title: &[],
        masthead: Masthead::Logotype,
        nonce: Some(nonce.0.clone()),
        scripts: vec![COMBOBOX_SCRIPT, HANDLE_TYPEAHEAD_SCRIPT, CONNECT_SCRIPT],
        main: html! {
            div.page-head {
                h1 { "Where have you eaten at?" }
                p.lede {
                    "We would like to know."
                }
                p.lede {
                    "Well, not " em { "us" } ", exactly. But you have thoughts about food, and surely someone demands to read them. Write about it here. Show 'em what's what."
                }
                p.lede {
                    "As for us, we don't actually care to know. We put all of your data in the "
                    strong { "at" } "mosphere, where it belongs."
                }
            }
            (connect(&Connect {
                appview,
                aside: "to start writing",
            }))
            hr.landing-divider;
            (lookup_form(&LookupForm {
                label: "Or read someone's reviews",
                button: "Read",
                primary: false,
                typeahead: Some(appview),
                ..LookupForm::default()
            }))
            p.meta.landing-note {
                "Every review stays in its author's repository; this site only reads."
            }
        },
        ..Page::default()
    })
}

/// The author's publication as the home page shows it.
struct Own<'a> {
    publication: &'a Record<Publication>,
    /// Where it is read: the hosted host, or the author's own domain.
    address: String,
    /// What the list is: the recent write-ups, or the matches for `q`.
    query: &'a str,
    items: Vec<eaten_at_web::components::ListingItem>,
    /// Whether the publication has more than the list shows.
    more: bool,
    truncated: bool,
    tags: Vec<eaten_at_web::components::Link>,
}

/// The author's home. With a publication it ships the live-find island;
/// without one there is nothing to find.
async fn signed_in(
    state: &AppState,
    did: &Did,
    query: &str,
    nonce: &Nonce,
) -> Result<Response, AppError> {
    let identity = state.require_identity(did).await?;
    let label = view::author_label(&identity);
    let (section, scripts) = match state.own_publication(&identity).await? {
        Some(publication) => {
            let own = own(state, &identity, &publication, query).await?;
            (own_section(did, &own), vec![FIND_SCRIPT])
        }
        None => (not_yet(state, &identity), Vec::new()),
    };
    let page = layout::render(&Page {
        title: &[],
        nonce: Some(nonce.0.clone()),
        scripts,
        main: html! {
            div.page-head {
                p.meta.handle { (label) }
                h1 { "Where did you eat?" }
            }
            div.actions.landing-actions {
                a.button href="/write" { "Write a new visit" }
            }
            (section)
            div.meta.tertiary {
                a href="/settings" { "Settings" }
                form.inline-form method="post" action="/logout" {
                    button.link-button type="submit" { "Sign out" }
                }
            }
        },
        ..Page::default()
    });
    Ok(page.into_response())
}

async fn own<'a>(
    state: &AppState,
    identity: &Identity,
    publication: &'a Record<Publication>,
    query: &'a str,
) -> Result<Own<'a>, AppError> {
    let did = &identity.did;
    let pub_rkey = publication.rkey();
    let claim = state
        .claims()
        .for_publication(&publication.uri)
        .await
        .map_err(|e| AppError::Upstream(e.to_string()))?;
    let address = match claim {
        Some(claim) => state.hosted_host(&claim.name),
        None => view::display_url(&publication.value.url),
    };
    // The recent list is the first page's newest eight; a find shows
    // its whole page. Both come through the same cached scan the front
    // page uses, so a fresh publish shows here at once.
    let listing = if query.is_empty() {
        state.visit_listing(identity, publication, None).await?
    } else {
        state
            .find_visits(identity, publication, query, None)
            .await?
    };
    let shown = if query.is_empty() {
        RECENT
    } else {
        listing.items.len()
    };
    let more = listing.items.len() > shown || listing.next_cursor.is_some();
    let tags = view::tag_links(
        did,
        pub_rkey,
        &crate::tags::distinct(
            listing
                .items
                .iter()
                .flat_map(|a| a.document().tags.iter().map(String::as_str)),
        ),
    );
    let items = listing
        .items
        .iter()
        .take(shown)
        .map(|visit_doc| view::listing_item(did, pub_rkey, visit_doc))
        .collect();
    Ok(Own {
        publication,
        address,
        query,
        items,
        more,
        truncated: listing.truncated,
        tags,
    })
}

/// The publication: a small nameplate, the find form with the tag
/// chips under it, then the list.
fn own_section(did: &Did, own: &Own<'_>) -> Markup {
    let pub_rkey = own.publication.rkey();
    let front = paths::publication(did, pub_rkey);
    let finding = !own.query.is_empty();
    html! {
        section.own-publication aria-labelledby="own-heading" {
            p.kicker #own-heading { "Your publication" }
            div.own-nameplate {
                p.own-name { a href=(front) { (own.publication.value.name) } }
                p.meta.own-address {
                    (own.address) " · "
                    a href=(paths::feed(did, pub_rkey)) rel="alternate" type="application/rss+xml" { "rss" }
                }
            }
            form.lookup.find action="/" method="get" {
                label.kicker.lookup-label for="q" { "Find a write-up" }
                div.lookup-row {
                    input #q name="q" type="search" value=(own.query) autocomplete="off"
                        placeholder="A place, a title, a street";
                    button.button-secondary type="submit" { "Find" }
                }
            }
            (tag_links(&own.tags, None))
            // What a find replaces, live or by a reload: the head and
            // the list, announced to assistive technology when it changes.
            div.find-results aria-live="polite" {
                div.list-head {
                    p.kicker {
                        @if finding { "Matching “" (own.query) "”" } @else { "Recent write-ups" }
                    }
                    @if finding {
                        a.button-link href="/" { "Clear" }
                    }
                }
                @if own.items.is_empty() {
                    @if finding {
                        p.empty { "Nothing called “" (own.query) "” among your write-ups." }
                    } @else {
                        p.empty { "No write-ups yet." }
                    }
                } @else {
                    (listing(&own.items))
                }
                @if own.truncated {
                    p.notice { "Showing recent write-ups; this publication also has many other documents." }
                }
                @if own.more && !finding {
                    div.actions {
                        a.button-link href=(front) { "All write-ups →" }
                    }
                }
            }
        }
    }
}

/// No publication yet: what it will be, and where to change that.
fn not_yet(state: &AppState, identity: &Identity) -> Markup {
    let spec = publish::default_spec(state, identity);
    let address = match &spec.home {
        Home::Hosted(label) | Home::HostedOrNext(label) => {
            format!("at {}", state.hosted_host(label))
        }
        Home::Own(url) => format!("at {}", view::display_url(url)),
        Home::SiteRoute => "at its own page on this site".to_owned(),
    };
    html! {
        section.own-publication.own-none aria-labelledby="own-heading" {
            p.kicker #own-heading { "Your publication" }
            p.lede {
                "Your publication is made when you publish your first write-up. "
                "It will be called " (spec.name) ", " (address) "; "
                a href="/settings" { "change that in settings" } "."
            }
        }
    }
}
