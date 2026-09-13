//! `/settings` — where each of the author's publications lives (plan
//! §5.5): a hosted subdomain of ours, or a domain they serve themselves.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use eaten_at_atproto::at_uri::AtUri;
use eaten_at_atproto::identity::Did;
use eaten_at_atproto::lexicon::{Publication, PUBLICATION_NSID};
use eaten_at_atproto::oauth::OAuthError;
use eaten_at_atproto::repo::Record;
use eaten_at_web::layout::{self, Page};
use maud::{html, Markup};
use serde::Deserialize;

use crate::auth::RequireUser;
use crate::cache::Namespace;
use crate::error::AppError;
use crate::hosting::{Claim, ClaimError};
use crate::state::AppState;
use crate::view;

/// One publication as the settings page shows it.
struct Row {
    record: Record<Publication>,
    claim: Option<Claim>,
}

async fn rows(state: &AppState, did: &Did) -> Result<Vec<Row>, AppError> {
    let identity = state.require_identity(did).await?;
    let mut rows = Vec::new();
    for record in state.publications(&identity).await? {
        let claim = state
            .claims()
            .for_publication(&record.uri)
            .await
            .map_err(|e| AppError::Upstream(e.to_string()))?;
        rows.push(Row { record, claim });
    }
    Ok(rows)
}

/// A problem with one publication's form, shown beside it.
struct Problem<'a> {
    pub_rkey: &'a str,
    message: &'a str,
}

fn page(state: &AppState, rows: &[Row], problem: Option<&Problem<'_>>) -> Markup {
    let can_host = state.can_host_subdomains();
    layout::render(&Page {
        title: &["Settings"],
        main: html! {
            div.page-head {
                p.kicker { "Settings" }
                h1 { "Your publications" }
                p.lede {
                    "Each publication has a home address. Host it here on a subdomain, "
                    "or serve it yourself and let this site read it."
                }
            }
            @if rows.is_empty() {
                p.empty { "No publications yet. The first one is made when you publish." }
            }
            ol.chooser.settings {
                @for row in rows {
                    li.chooser-item {
                        h2.chooser-name { (row.record.value.name) }
                        p.meta.chooser-url {
                            @if let Some(claim) = &row.claim {
                                "Hosted here at " (state.hosted_host(&claim.name))
                            } @else {
                                "Served by you at " (view::display_url(&row.record.value.url))
                            }
                        }
                        form.hosting-form method="post" action=(format!("/settings/{}/hosting", row.record.rkey())) {
                            @let hosted = row.claim.is_some();
                            div.field {
                                label.choice {
                                    input type="radio" name="mode" value="hosted" checked[hosted] disabled[!can_host];
                                    " Host it here"
                                }
                                div.lookup-row.subdomain {
                                    input type="text" name="name" value=[row.claim.as_ref().map(|c| c.name.as_str())]
                                        placeholder="name" spellcheck="false" disabled[!can_host]
                                        aria-label="Subdomain name";
                                    span.meta { "." (state.public_host()) }
                                }
                                @if !can_host {
                                    p.meta.field-hint { "Hosting needs a real domain; this deployment has none." }
                                }
                            }
                            div.field {
                                label.choice {
                                    input type="radio" name="mode" value="own" checked[!hosted];
                                    " Serve it yourself at"
                                }
                                input type="url" name="url" value=[(!hosted).then_some(row.record.value.url.as_str())]
                                    placeholder="https://" spellcheck="false" aria-label="Your publication's address";
                                p.meta.field-hint { "Your site serves the pages; keep the .well-known verification there." }
                            }
                            @if let Some(problem) = problem.filter(|p| p.pub_rkey == row.record.rkey()) {
                                p.field-error role="alert" { (problem.message) }
                            }
                            div.actions { button type="submit" { "Save" } }
                        }
                    }
                }
            }
        },
        ..Page::default()
    })
}

/// `GET /settings`.
pub async fn settings(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
) -> Result<Response, AppError> {
    let rows = rows(&state, &did).await?;
    Ok(page(&state, &rows, None).into_response())
}

#[derive(Debug, Deserialize)]
pub struct HostingForm {
    #[serde(default)]
    mode: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    url: String,
}

/// `POST /settings/{pub_rkey}/hosting` — move a publication between a
/// hosted subdomain and the author's own domain.
pub async fn hosting(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    Path(pub_rkey): Path<String>,
    Form(form): Form<HostingForm>,
) -> Result<Response, AppError> {
    let identity = state.require_identity(&did).await?;
    let record = state
        .publication(&identity, &pub_rkey)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("publication {pub_rkey} not found")))?;
    let current = state
        .claims()
        .for_publication(&record.uri)
        .await
        .map_err(|e| AppError::Upstream(e.to_string()))?;

    let outcome = apply(&state, &did, &record, current.as_ref(), &form).await;
    match outcome {
        Ok(()) => Ok(Redirect::to("/settings").into_response()),
        Err(Outcome::Login) => Ok(Redirect::to("/login?return_to=/settings").into_response()),
        Err(Outcome::Problem(message)) => {
            let rows = rows(&state, &did).await?;
            let problem = Problem {
                pub_rkey: &pub_rkey,
                message: &message,
            };
            Ok((
                StatusCode::UNPROCESSABLE_ENTITY,
                page(&state, &rows, Some(&problem)),
            )
                .into_response())
        }
        Err(Outcome::App(err)) => Err(err),
    }
}

enum Outcome {
    Login,
    Problem(String),
    App(AppError),
}

impl From<DbFailure> for Outcome {
    fn from(err: DbFailure) -> Self {
        Self::App(AppError::Upstream(err.0))
    }
}

struct DbFailure(String);

async fn apply(
    state: &AppState,
    did: &Did,
    record: &Record<Publication>,
    current: Option<&Claim>,
    form: &HostingForm,
) -> Result<(), Outcome> {
    let db = |e: crate::db::DbError| DbFailure(e.to_string());
    let new_url = match form.mode.as_str() {
        "hosted" => {
            if !state.can_host_subdomains() {
                return Err(Outcome::Problem(
                    "Hosting isn't available on this deployment.".to_owned(),
                ));
            }
            match state.claims().claim(&form.name, &record.uri, did).await {
                Ok(()) => {}
                Err(ClaimError::Db(err)) => {
                    return Err(Outcome::App(AppError::Upstream(err.to_string())))
                }
                Err(err) => return Err(Outcome::Problem(format!("{err}."))),
            }
            let name = crate::hosting::validate_name(&form.name)
                .map_err(|e| Outcome::Problem(e.to_string()))?;
            state.hosted_origin(&name)
        }
        "own" => {
            let url = form.url.trim();
            match url::Url::parse(url) {
                Ok(parsed) if parsed.scheme() == "https" && parsed.host_str().is_some() => {}
                _ => {
                    return Err(Outcome::Problem(
                        "Enter the https address your publication is served at.".to_owned(),
                    ))
                }
            }
            state.claims().release(&record.uri).await.map_err(db)?;
            url.trim_end_matches('/').to_owned()
        }
        _ => {
            return Err(Outcome::Problem(
                "Choose where the publication lives.".to_owned(),
            ))
        }
    };
    if new_url == record.value.base_url() {
        return Ok(());
    }

    // The record is rewritten first; if the author's server refuses, the
    // claim goes back to what it was so the two never disagree.
    let session = match state.oauth().session(did).await {
        Ok(session) => session,
        Err(OAuthError::NoSession(_) | OAuthError::Unauthenticated) => return Err(Outcome::Login),
        Err(err) => return Err(Outcome::App(AppError::Upstream(err.to_string()))),
    };
    let mut value = serde_json::to_value(&record.value).unwrap_or_default();
    value["$type"] = serde_json::Value::String(PUBLICATION_NSID.to_owned());
    value["url"] = serde_json::Value::String(new_url.clone());
    if let Err(err) = session
        .put_record(PUBLICATION_NSID, record.rkey(), &value)
        .await
    {
        tracing::warn!(error = %err, "publication rewrite refused");
        restore_claim(state, did, record, current).await;
        return Err(Outcome::Problem(
            "Your server did not accept the change. Nothing was changed.".to_owned(),
        ));
    }
    if let Some(old) = current {
        if state.hosted_origin(&old.name) != new_url {
            state
                .claims()
                .record_move(&state.hosted_host(&old.name), &new_url)
                .await
                .map_err(db)?;
        }
    }
    let cache = state.cache();
    cache
        .evict(Namespace::Publication, &format!("list:{did}"))
        .await;
    cache
        .evict(Namespace::Publication, &format!("{did}/{}", record.rkey()))
        .await;
    Ok(())
}

/// Put the claims table back the way it was before a failed rewrite.
async fn restore_claim(
    state: &AppState,
    did: &Did,
    record: &Record<Publication>,
    current: Option<&Claim>,
) {
    let result = match current {
        Some(claim) => state.claims().claim(&claim.name, &record.uri, did).await,
        None => state
            .claims()
            .release(&record.uri)
            .await
            .map_err(ClaimError::from),
    };
    if let Err(err) = result {
        tracing::error!(%err, publication = %record.uri, "could not restore the previous claim");
    }
}

/// For the well-known document: the record's AT-URI is the identity a
/// verifier compares against.
pub fn publication_uri(did: &Did, pub_rkey: &str) -> Option<AtUri> {
    AtUri::from_parts(did, PUBLICATION_NSID, pub_rkey).ok()
}
