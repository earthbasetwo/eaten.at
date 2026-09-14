//! `/settings` — the account's one publication (plan 08): its name,
//! what it says about itself, and where it lives (plan §5.5): a hosted
//! subdomain of ours, or a domain the author serves themselves.
//!
//! An account with no publication yet sees the defaults it would be
//! created with; the first save creates it. A publish creates it too,
//! so this page is never a step anyone has to take.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use eaten_at_atproto::at_uri::AtUri;
use eaten_at_atproto::identity::{Did, Identity};
use eaten_at_atproto::lexicon::{Datetime, Publication, PUBLICATION_NSID};
use eaten_at_atproto::oauth::OAuthError;
use eaten_at_atproto::repo::Record;
use eaten_at_web::layout::{self, Page};
use maud::{html, Markup};
use serde::Deserialize;
use unicode_segmentation::UnicodeSegmentation;

use crate::auth::RequireUser;
use crate::cache::Namespace;
use crate::error::AppError;
use crate::hosting::{self, Claim, ClaimError};
use crate::publish::{self, Home, PublicationSpec, PublishError};
use crate::state::AppState;
use crate::view;

/// Lexicon limits on the publication record, in graphemes.
const MAX_NAME_GRAPHEMES: usize = 500;
const MAX_DESCRIPTION_GRAPHEMES: usize = 3000;

/// The publication as it is. `None` means the account has none yet,
/// and the page shows the defaults it would be made with.
type Current = Option<Existing>;

struct Existing {
    record: Record<Publication>,
    claim: Option<Claim>,
}

async fn current(state: &AppState, identity: &Identity) -> Result<Current, AppError> {
    match state.own_publication(identity).await? {
        Some(record) => {
            let claim = state
                .claims()
                .for_publication(&record.uri)
                .await
                .map_err(|e| AppError::Upstream(e.to_string()))?;
            Ok(Some(Existing { record, claim }))
        }
        None => Ok(None),
    }
}

/// The form's fields, as shown or as posted.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct SettingsForm {
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
    /// `hosted` or `own`.
    #[serde(default)]
    mode: String,
    #[serde(default)]
    subdomain: String,
    #[serde(default)]
    url: String,
}

impl SettingsForm {
    /// The form prefilled from the publication and its claim.
    fn from_current(state: &AppState, identity: &Identity, current: &Current) -> Self {
        if let Some(Existing { record, claim }) = current {
            return Self {
                name: record.value.name.clone(),
                description: record.value.description.clone().unwrap_or_default(),
                mode: if claim.is_some() { "hosted" } else { "own" }.to_owned(),
                subdomain: claim.as_ref().map(|c| c.name.clone()).unwrap_or_default(),
                url: if claim.is_some() {
                    String::new()
                } else {
                    record.value.url.clone()
                },
            };
        }
        let spec = publish::default_spec(state, identity);
        let hosted = matches!(spec.home, Home::HostedOrNext(_));
        Self {
            name: spec.name,
            description: String::new(),
            mode: if hosted { "hosted" } else { "own" }.to_owned(),
            subdomain: match spec.home {
                Home::HostedOrNext(label) | Home::Hosted(label) => label,
                Home::Own(_) | Home::SiteRoute => String::new(),
            },
            url: String::new(),
        }
    }
}

/// A problem with the form, shown beside its field.
struct Problem {
    field: &'static str,
    message: String,
}

fn page(
    state: &AppState,
    current: &Current,
    form: &SettingsForm,
    problem: Option<&Problem>,
) -> Markup {
    let can_host = state.can_host_subdomains();
    let not_yet = current.is_none();
    let error_for = |field: &str| {
        problem
            .filter(|p| p.field == field)
            .map(|p| html! { p.field-error role="alert" { (p.message) } })
    };
    layout::render(&Page {
        title: &["Settings"],
        main: html! {
            div.page-head {
                p.kicker { "Settings" }
                h1 { "Your publication" }
                p.lede {
                    @if not_yet {
                        "You don't have one yet. This is what it will be; change anything, or leave it."
                    } @else {
                        "Its name, what it says about itself, and where it lives."
                    }
                }
            }
            ol.chooser.settings {
                li.chooser-item.not-yet[not_yet] {
                    @match current {
                        Some(Existing { record, claim }) => {
                            h2.chooser-name { a href=(view::publication_path_for(&record.uri)) { (record.value.name) } }
                            p.meta.chooser-url {
                                @if let Some(claim) = claim {
                                    "Hosted here at " (state.hosted_host(&claim.name))
                                } @else {
                                    "Served by you at " (view::display_url(&record.value.url))
                                }
                            }
                        }
                        None => {
                            h2.chooser-name { (form.name) }
                            p.meta.chooser-url { "Made when you save, or when you publish your first write-up." }
                        }
                    }
                    form.hosting-form method="post" action="/settings" {
                        div.field {
                            label.kicker for="name" { "Name" }
                            input #name name="name" type="text" value=(form.name) required
                                maxlength=(MAX_NAME_GRAPHEMES);
                            (error_for("name").unwrap_or_default())
                        }
                        div.field {
                            label.kicker for="description" { "Description (optional)" }
                            textarea #description name="description" rows="3" { (form.description) }
                            (error_for("description").unwrap_or_default())
                            p.meta.field-hint { "Shown under the name on the front page and in link previews." }
                        }
                        @let hosted = form.mode == "hosted";
                        div.field {
                            label.choice {
                                input type="radio" name="mode" value="hosted" checked[hosted] disabled[!can_host];
                                " Host it here"
                            }
                            div.lookup-row.subdomain {
                                input type="text" name="subdomain" value=(form.subdomain)
                                    placeholder="name" spellcheck="false" disabled[!can_host]
                                    aria-label="Subdomain name";
                                span.meta { "." (state.public_host()) }
                            }
                            (error_for("subdomain").unwrap_or_default())
                            @if !can_host {
                                p.meta.field-hint { "Hosting needs a real domain; this deployment has none." }
                            }
                        }
                        div.field {
                            label.choice {
                                input type="radio" name="mode" value="own" checked[!hosted];
                                " Serve it yourself at"
                            }
                            input type="url" name="url" value=(form.url)
                                placeholder="https://" spellcheck="false" aria-label="Your publication's address";
                            (error_for("url").unwrap_or_default())
                            @if not_yet && !can_host {
                                p.meta.field-hint { "Left blank, the publication's address is its page on this site." }
                            } @else {
                                p.meta.field-hint { "Your site serves the pages; keep the .well-known verification there." }
                            }
                        }
                        (error_for("mode").unwrap_or_default())
                        div.actions { button type="submit" { @if not_yet { "Create it" } @else { "Save" } } }
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
    let identity = state.require_identity(&did).await?;
    let current = current(&state, &identity).await?;
    let form = SettingsForm::from_current(&state, &identity, &current);
    Ok(page(&state, &current, &form, None).into_response())
}

/// `POST /settings` — create the publication, or change its name,
/// description, or address.
pub async fn save(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    Form(form): Form<SettingsForm>,
) -> Result<Response, AppError> {
    let identity = state.require_identity(&did).await?;
    let current = current(&state, &identity).await?;
    let outcome = match &current {
        Some(Existing { record, claim }) => {
            apply(&state, &identity, record, claim.as_ref(), &form).await
        }
        None => create(&state, &identity, &form).await,
    };
    match outcome {
        Ok(()) => Ok(Redirect::to("/settings").into_response()),
        Err(Outcome::Login) => Ok(Redirect::to("/login?return_to=/settings").into_response()),
        Err(Outcome::Problem(problem)) => Ok((
            StatusCode::UNPROCESSABLE_ENTITY,
            page(&state, &current, &form, Some(&problem)),
        )
            .into_response()),
        Err(Outcome::App(err)) => Err(err),
    }
}

enum Outcome {
    Login,
    Problem(Problem),
    App(AppError),
}

impl Outcome {
    fn problem(field: &'static str, message: impl Into<String>) -> Self {
        Self::Problem(Problem {
            field,
            message: message.into(),
        })
    }
}

impl From<DbFailure> for Outcome {
    fn from(err: DbFailure) -> Self {
        Self::App(AppError::Upstream(err.0))
    }
}

impl From<PublishError> for Outcome {
    fn from(err: PublishError) -> Self {
        match err {
            PublishError::SessionExpired => Self::Login,
            PublishError::Home(message) => Self::problem("subdomain", message),
            PublishError::App(err) => Self::App(err),
            PublishError::Repo(err) => {
                tracing::warn!(error = %err, "publication write refused");
                Self::problem(
                    "mode",
                    "Your server did not accept the change. Nothing was changed.",
                )
            }
        }
    }
}

struct DbFailure(String);

/// The name and description as they would be written, checked.
fn checked_text(form: &SettingsForm) -> Result<(String, Option<String>), Outcome> {
    let name = form.name.trim();
    if name.is_empty() {
        return Err(Outcome::problem("name", "Name the publication."));
    }
    if name.graphemes(true).count() > MAX_NAME_GRAPHEMES {
        return Err(Outcome::problem(
            "name",
            format!("Keep the name under {MAX_NAME_GRAPHEMES} characters."),
        ));
    }
    let description = form.description.trim();
    if description.graphemes(true).count() > MAX_DESCRIPTION_GRAPHEMES {
        return Err(Outcome::problem(
            "description",
            format!("Keep the description under {MAX_DESCRIPTION_GRAPHEMES} characters."),
        ));
    }
    Ok((
        name.to_owned(),
        (!description.is_empty()).then(|| description.to_owned()),
    ))
}

/// An `https` origin the author serves, checked.
fn checked_own_url(raw: &str) -> Result<String, Outcome> {
    let url = raw.trim();
    match url::Url::parse(url) {
        Ok(parsed) if parsed.scheme() == "https" && parsed.host_str().is_some() => {
            Ok(url.trim_end_matches('/').to_owned())
        }
        _ => Err(Outcome::problem(
            "url",
            "Enter the https address your publication is served at.",
        )),
    }
}

/// A first save: create the publication as the form describes it.
async fn create(state: &AppState, identity: &Identity, form: &SettingsForm) -> Result<(), Outcome> {
    let (name, description) = checked_text(form)?;
    let home = match form.mode.as_str() {
        "hosted" => {
            if !state.can_host_subdomains() {
                return Err(Outcome::problem(
                    "mode",
                    "Hosting isn't available on this deployment.",
                ));
            }
            Home::Hosted(form.subdomain.clone())
        }
        "own" => {
            if form.url.trim().is_empty() && !state.can_host_subdomains() {
                Home::SiteRoute
            } else {
                Home::Own(checked_own_url(&form.url)?)
            }
        }
        _ => {
            return Err(Outcome::problem(
                "mode",
                "Choose where the publication lives.",
            ))
        }
    };
    let session = session_for(state, &identity.did).await?;
    publish::create_publication(
        state,
        identity,
        &session,
        PublicationSpec {
            name,
            description,
            home,
        },
        state.preferences(identity).await.map_err(Outcome::App)?,
        &Datetime::now(),
    )
    .await?;
    Ok(())
}

async fn session_for(
    state: &AppState,
    did: &Did,
) -> Result<eaten_at_atproto::oauth::AuthorizedSession, Outcome> {
    match state.oauth().session(did).await {
        Ok(session) => Ok(session),
        Err(OAuthError::NoSession(_) | OAuthError::Unauthenticated) => Err(Outcome::Login),
        Err(err) => Err(Outcome::App(AppError::Upstream(err.to_string()))),
    }
}

/// A later save: rewrite the record with what changed, moving the
/// claim first so the two never disagree.
async fn apply(
    state: &AppState,
    identity: &Identity,
    record: &Record<Publication>,
    current: Option<&Claim>,
    form: &SettingsForm,
) -> Result<(), Outcome> {
    let did = &identity.did;
    let db = |e: crate::db::DbError| DbFailure(e.to_string());
    let (name, description) = checked_text(form)?;
    let new_url = match form.mode.as_str() {
        "hosted" => {
            if !state.can_host_subdomains() {
                return Err(Outcome::problem(
                    "mode",
                    "Hosting isn't available on this deployment.",
                ));
            }
            match state
                .claims()
                .claim(&form.subdomain, &record.uri, did)
                .await
            {
                Ok(()) => {}
                Err(ClaimError::Db(err)) => {
                    return Err(Outcome::App(AppError::Upstream(err.to_string())))
                }
                Err(err) => return Err(Outcome::problem("subdomain", format!("{err}."))),
            }
            let label = hosting::validate_name(&form.subdomain)
                .map_err(|e| Outcome::problem("subdomain", e.to_string()))?;
            state.hosted_origin(&label)
        }
        "own" => {
            let url = checked_own_url(&form.url)?;
            state.claims().release(&record.uri).await.map_err(db)?;
            url
        }
        _ => {
            return Err(Outcome::problem(
                "mode",
                "Choose where the publication lives.",
            ))
        }
    };
    let unchanged = new_url == record.value.base_url()
        && name == record.value.name
        && description == record.value.description;
    if unchanged {
        return Ok(());
    }

    // The record is rewritten first; if the author's server refuses, the
    // claim goes back to what it was so the two never disagree.
    let session = session_for(state, did).await?;
    let mut value = serde_json::to_value(&record.value).unwrap_or_default();
    value["$type"] = serde_json::Value::String(PUBLICATION_NSID.to_owned());
    value["url"] = serde_json::Value::String(new_url.clone());
    value["name"] = serde_json::Value::String(name);
    match description {
        Some(description) => value["description"] = serde_json::Value::String(description),
        None => {
            if let Some(fields) = value.as_object_mut() {
                fields.remove("description");
            }
        }
    }
    if let Err(err) = session
        .put_record(PUBLICATION_NSID, record.rkey(), &value)
        .await
    {
        tracing::warn!(error = %err, "publication rewrite refused");
        restore_claim(state, did, record, current).await;
        return Err(Outcome::problem(
            "mode",
            "Your server did not accept the change. Nothing was changed.",
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
