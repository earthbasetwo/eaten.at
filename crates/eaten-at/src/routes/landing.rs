//! `GET /` — signed out, the pitch and the two ways in, each one button
//! that becomes a handle field (plan 09); signed in, the author's home:
//! one primary "Write a new digest", their feed with its recent
//! digests and a way to find one, and settings last (plan 11). Both
//! states end on the same quiet line, the page foot (S18), which is
//! where the about page (and with it the credits) is reached from.

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use eaten_at_atproto::identity::{Did, Identity};
use eaten_at_atproto::lexicon::Publication;
use eaten_at_atproto::repo::Record;
use eaten_at_web::assets::{COMBOBOX_SCRIPT, CONNECT_SCRIPT, FIND_SCRIPT, HANDLE_TYPEAHEAD_SCRIPT};
use eaten_at_web::components::{connect, listing_compact, tag_line, Connect, ConnectWay};
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
    let origin = state.absolute("");
    let mut response = signed_out(&nonce, &appview, &origin).into_response();
    security::allow_connect_and_post(&mut response, &nonce, &appview, &origin);
    Ok(response)
}

/// The pitch, one primary "Connect" that becomes the reader's own
/// handle field, and beneath a hairline the same treatment for reading
/// without signing in: a line, then one secondary button that becomes a
/// field for someone else's handle.
fn signed_out(nonce: &Nonce, appview: &str, origin: &str) -> Markup {
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
                origin,
                way: ConnectWay::Write,
                intro: None,
                label: "Connect to start writing",
            }))
            hr.landing-divider;
            (connect(&Connect {
                appview,
                origin,
                way: ConnectWay::Read,
                intro: Some("Oh, so you're one of the demanding public, eh?"),
                label: "Look up a friend",
            }))
        },
        // The one quiet line signed out, in the page foot: where the
        // credits are.
        foot: html! {
            div.meta.tertiary {
                a href="/about" { "About" }
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
    /// How many digests the feed has, when one scan saw them all.
    count: Option<usize>,
    truncated: bool,
    /// The feed's tags, the most used first, each with its count (S4, S9).
    tags: Vec<(eaten_at_web::components::Link, usize)>,
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
                a.button href="/write" { "Write a new digest" }
            }
            (section)
        },
        foot: html! {
            div.meta.tertiary {
                a href="/settings" { "Settings" }
                a href="/about" { "About" }
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
    // The tag line is always the feed's, not the find's: it is hidden
    // while a find is on and comes back whole when it is cleared (S19).
    let recent = state.visit_listing(identity, publication, None).await?;
    let tags = view::tag_counts(
        did,
        pub_rkey,
        &crate::tags::tally(
            recent
                .items
                .iter()
                .flat_map(|a| a.document().tags.iter().map(String::as_str)),
        ),
    );
    let count = (recent.next_cursor.is_none() && !recent.truncated).then_some(recent.items.len());
    let listing = if query.is_empty() {
        recent
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
        count,
        truncated: listing.truncated,
        tags,
    })
}

/// The publication: the front page's nameplate in miniature (S17) with
/// the tag line closing it, the find field, then the compact rows. The field has no button and
/// no visible label (S1, S3): Return sends it, its placeholder says
/// what it finds, and the live find filters as you type anyway. The
/// "Your feed" heading names the landmark and shows nowhere (S10). The
/// field carries the find's whole state (S19): while it holds a query
/// a clear mark stands at the field's end where the return mark was,
/// and the list has no visible head; a hidden "Matching …" line still
/// tells assistive technology. The tags live in the masthead and stay.
fn own_section(did: &Did, own: &Own<'_>) -> Markup {
    let pub_rkey = own.publication.rkey();
    let front = paths::publication(did, pub_rkey);
    let finding = !own.query.is_empty();
    html! {
        section.own-publication aria-labelledby="own-heading" {
            p.kicker.visually-hidden #own-heading { "Your feed" }
            // The front page's nameplate in miniature (S17): the reader's
            // masthead, so the rows below read as the feed. A double rule
            // parts it from the action above; a hairline closes it.
            header.own-masthead {
                p.own-name { a href=(front) { (own.publication.value.name) } }
                ul.dateline {
                    li { (own.address) }
                    li { a href=(paths::feed(did, pub_rkey)) rel="alternate" type="application/rss+xml" { "rss" } }
                    @if let Some(count) = own.count {
                        li { (count) " " @if count == 1 { "digest" } @else { "digests" } }
                    }
                }
                @if let Some(description) = &own.publication.value.description {
                    p.lede { (description) }
                }
                // The tags close the masthead as the front page's chips do,
                // and stay through a find: they are the feed's, not the find's.
                (tag_line(&own.tags))
            }
            div.own-find {
                form.lookup.find action="/" method="get" {
                    label.visually-hidden for="q" { "Find a digest" }
                    div.lookup-row {
                        span.return-rule {
                            input #q name="q" type="search" value=(own.query) autocomplete="off"
                                placeholder="Find a place, a title, a street";
                            // A link home without script; with it, the field empties in place.
                            a.find-clear href="/" aria-label="Clear the find" hidden[!finding] {}
                        }
                    }
                }
            }
            // What a find replaces, live or by a reload: the head and
            // the list, announced to assistive technology when it changes.
            div.find-results aria-live="polite" {
                @if finding {
                    p.kicker.visually-hidden { "Matching “" (own.query) "”" }
                }
                @if own.items.is_empty() {
                    @if finding {
                        p.empty { "Nothing called “" (own.query) "” among your digests." }
                    } @else {
                        p.empty { "No digests yet." }
                    }
                } @else {
                    (listing_compact(&own.items))
                }
                @if own.truncated {
                    p.notice { "Showing recent digests; this feed also has many other documents." }
                }
                @if own.more && !finding {
                    div.actions {
                        a.button-link href=(front) { "All digests →" }
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
            p.kicker #own-heading { "Your feed" }
            p.lede {
                "Your feed is made when you publish your first digest. "
                "It will be called " (spec.name) ", " (address) "; "
                a href="/settings" { "change that in settings" } "."
            }
        }
    }
}
