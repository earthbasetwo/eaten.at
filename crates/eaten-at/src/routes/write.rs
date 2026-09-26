//! `GET/POST /write` and `/write/{doc_rkey}` — the editor (plan §7.5),
//! with its delete and crosspost pages.
//!
//! Every submission comes back as the same page: a structural action
//! (add or remove a row, change the place) re-renders the form with
//! everything typed kept; publish validates it, showing any problems
//! beside their fields, then writes the records and, when asked, posts
//! to Bluesky.

use axum::extract::rejection::FormRejection;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Redirect, Response};
use axum::Form;
use eaten_at_atproto::at_uri::AtUri;
use eaten_at_atproto::identity::{Did, Identity};
use eaten_at_web::assets::{
    CHOOSE_PLACE_SCRIPT, COMBOBOX_SCRIPT, DIGEST_SCRIPT, EDITOR_SCRIPT, PHOTOS_SCRIPT, TAGS_SCRIPT,
    WRITE_SCRIPT,
};
use eaten_at_web::layout::{self, urlencoding, Page};
use serde::{Deserialize, Serialize};
use unicode_segmentation::UnicodeSegmentation;

use crate::auth::{PostPermission, RequireUser};
use crate::editor::view::{
    self, CrosspostPage, CrosspostState, DeletePage, DeletePost, EditorPage, Located,
};
use crate::editor::{self, Action, Context, EditorForm, FieldErrors, PlaceMode};
use crate::error::AppError;
use crate::geoip::ClientIp;
use crate::model::VisitDocument;
use crate::paths;
use crate::places::{Point, SearchError};
use crate::publish::{self, PublishError, MAX_POST_GRAPHEMES};
use crate::security::Nonce;
use crate::state::AppState;

/// Suggestion requests one session may make per minute (plan 12, D45).
pub const SUGGESTS_PER_MINUTE: u32 = 30;
/// Suggestions the listbox shows, of the search's ten.
const SUGGEST_LIMIT: usize = 6;

/// Who is writing. The publication is not part of it: every write-up
/// goes to the account's one publication, made on the first publish if
/// need be (plan 08).
struct Author {
    identity: Identity,
    /// What the sign-in may do with Bluesky posts.
    posting: PostPermission,
}

async fn author(state: &AppState, did: &Did) -> Result<Author, AppError> {
    let identity = state.require_identity(did).await?;
    // A session whose grant cannot be read is treated as one that may
    // not post; the worst case is being asked to allow it again.
    let posting = match state.oauth().granted_scopes(did).await {
        Ok(scopes) => crate::auth::post_permission(&scopes),
        Err(err) => {
            tracing::warn!(%did, error = %err, "could not read the session's scopes");
            PostPermission::default()
        }
    };
    Ok(Author { identity, posting })
}

/// The document being edited, when there is one.
struct Editing {
    rkey: String,
    visit_doc: VisitDocument,
}

impl Editing {
    /// The Bluesky post the document names, when it is a post in the
    /// author's own repo.
    fn post(&self, did: &Did) -> Option<AtUri> {
        crate::view::bluesky_post_uri(self.visit_doc.document()).filter(|uri| uri.did() == did)
    }

    fn document_path(&self, did: &Did) -> String {
        AtUri::parse(&self.visit_doc.document().site).map_or_else(
            |_| paths::repo(did),
            |site| paths::document(did, site.rkey(), &self.rkey),
        )
    }
}

/// `/write/{rkey}/crosspost?text=…[&failed=1]`.
fn crosspost_path(rkey: &str, text: &str, failed: bool) -> String {
    format!(
        "/write/{rkey}/crosspost?text={}{}",
        urlencoding(text),
        if failed { "&failed=1" } else { "" }
    )
}

async fn editing(state: &AppState, author: &Author, rkey: &str) -> Result<Editing, AppError> {
    let record = state
        .document(&author.identity, rkey)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("document {rkey} not found")))?;
    let visit_doc = VisitDocument::from_record(record)
        .ok_or_else(|| AppError::BadRequest("that document is not a digest".to_owned()))?;
    Ok(Editing {
        rkey: rkey.to_owned(),
        visit_doc,
    })
}

/// What the page shows besides the form.
#[derive(Default)]
struct Outcome<'a> {
    errors: FieldErrors,
    publish_error: Option<&'a str>,
    reauthenticate: bool,
    /// Why a picked suggestion could not be taken.
    pick_error: Option<&'a str>,
    /// Where the choosing screen's suggestions would look; unknown
    /// means none are offered.
    located: Located,
}

fn render(
    state: &AppState,
    nonce: &str,
    status: StatusCode,
    editing: Option<&Editing>,
    form: &EditorForm,
    outcome: &Outcome<'_>,
) -> Response {
    let action_path = editing.map_or_else(|| "/write".to_owned(), |e| format!("/write/{}", e.rkey));
    let photos_page = editing.map(|e| format!("/write/{}/photos", e.rkey));
    // The choosing screen suggests places; the editing screen keeps a
    // draft, dresses its controls, edits the markdown live, files tags
    // as chips, and manages photos in place. Each ships only its own
    // islands.
    let scripts = if form.place_mode == PlaceMode::Choosing {
        vec![COMBOBOX_SCRIPT, CHOOSE_PLACE_SCRIPT]
    } else {
        vec![
            EDITOR_SCRIPT,
            WRITE_SCRIPT,
            DIGEST_SCRIPT,
            TAGS_SCRIPT,
            PHOTOS_SCRIPT,
        ]
    };
    let page = layout::render(&Page {
        title: &[if editing.is_some() { "Edit" } else { "Write" }],
        nonce: Some(nonce.to_owned()),
        scripts,
        main: view::page(&EditorPage {
            form,
            errors: &outcome.errors,
            action_path: &action_path,
            editing: editing.is_some(),
            publish_error: outcome.publish_error,
            reauthenticate: outcome.reauthenticate,
            pick_error: outcome.pick_error,
            suggesting: state.places_enabled() && outcome.located != Located::Unknown,
            photos_page: photos_page.as_deref(),
        }),
        ..Page::default()
    });
    (status, page).into_response()
}

/// `GET /write` — a blank editor, choosing the place first.
pub async fn new_form(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    ip: ClientIp,
    nonce: Nonce,
) -> Result<Response, AppError> {
    let nonce = nonce.0.as_str();
    let author = author(&state, &did).await?;
    let form = EditorForm::blank();
    let outcome = Outcome {
        located: locate(&state, &author.identity, ip).await.1,
        ..Outcome::default()
    };
    Ok(render(&state, nonce, StatusCode::OK, None, &form, &outcome))
}

/// `GET /write/{rkey}` — the editor prefilled from the author's document.
pub async fn edit_form(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    Path(rkey): Path<String>,
    nonce: Nonce,
) -> Result<Response, AppError> {
    let nonce = nonce.0.as_str();
    let author = author(&state, &did).await?;
    let editing = editing(&state, &author, &rkey).await?;
    let form = EditorForm::from_document(editing.visit_doc.document(), &editing.visit_doc.visit);
    let outcome = Outcome::default();
    Ok(render(
        &state,
        nonce,
        StatusCode::OK,
        Some(&editing),
        &form,
        &outcome,
    ))
}

/// `POST /write` — a submission for a new write-up.
pub async fn submit_new(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    ip: ClientIp,
    nonce: Nonce,
    form: Result<Form<Vec<(String, String)>>, FormRejection>,
) -> Result<Response, AppError> {
    let author = author(&state, &did).await?;
    submit(&state, &author, None, ip, &nonce.0, form).await
}

/// `POST /write/{rkey}` — a submission for an existing write-up.
pub async fn submit_edit(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    Path(rkey): Path<String>,
    ip: ClientIp,
    nonce: Nonce,
    form: Result<Form<Vec<(String, String)>>, FormRejection>,
) -> Result<Response, AppError> {
    let author = author(&state, &did).await?;
    let editing = editing(&state, &author, &rkey).await?;
    submit(&state, &author, Some(&editing), ip, &nonce.0, form).await
}

async fn submit(
    state: &AppState,
    author: &Author,
    editing: Option<&Editing>,
    ip: ClientIp,
    nonce: &str,
    form: Result<Form<Vec<(String, String)>>, FormRejection>,
) -> Result<Response, AppError> {
    let Form(pairs) =
        form.map_err(|e| AppError::BadRequest(format!("could not read the form: {e}")))?;
    let (mut form, action) = EditorForm::from_pairs(pairs);
    // A submission that names no button (Return in a field, a form sent
    // by hand) only comes back as it is.
    let action = action.unwrap_or(Action::Keep);
    match action {
        Action::Pick(index) => {
            return Ok(pick(state, nonce, author, editing, ip, form, index).await)
        }
        Action::Publish => {}
        Action::Manual if form.place_name.trim().is_empty() => {
            // The one error the choosing page can show: a place by hand
            // needs a name.
            let mut errors = FieldErrors::default();
            errors.add("place_name", "Name the place.");
            let located = locate(state, &author.identity, ip).await.1;
            return Ok(render(
                state,
                nonce,
                StatusCode::UNPROCESSABLE_ENTITY,
                editing,
                &form,
                &Outcome {
                    errors,
                    located,
                    ..Outcome::default()
                },
            ));
        }
        _ => {
            form.apply(&action);
            // Back to choosing needs to know where the search would look.
            let located = if form.place_mode == PlaceMode::Choosing {
                locate(state, &author.identity, ip).await.1
            } else {
                Located::Unknown
            };
            return Ok(render(
                state,
                nonce,
                StatusCode::OK,
                editing,
                &form,
                &Outcome {
                    located,
                    ..Outcome::default()
                },
            ));
        }
    }
    let context = Context {
        original: editing.map(|e| &e.visit_doc.visit),
    };
    let draft = match editor::validate(&form, &context) {
        Ok(draft) => draft,
        Err(errors) => {
            return Ok(render(
                state,
                nonce,
                StatusCode::UNPROCESSABLE_ENTITY,
                editing,
                &form,
                &Outcome {
                    errors,
                    ..Outcome::default()
                },
            ))
        }
    };
    publish_and_continue(state, nonce, author, editing, &form, &draft).await
}

/// Write the draft and move on, or come back to the form saying why not.
async fn publish_and_continue(
    state: &AppState,
    nonce: &str,
    author: &Author,
    editing: Option<&Editing>,
    form: &EditorForm,
    draft: &editor::DocumentDraft,
) -> Result<Response, AppError> {
    match publish::publish(
        state,
        &author.identity,
        draft,
        editing.map(|e| &e.visit_doc),
    )
    .await
    {
        Ok(published) => {
            let outcome = if editing.is_some() {
                "saved"
            } else {
                "published"
            };
            let draft_id = form.draft_id.map_or_else(String::new, |id| format!("&draft={id}"));
            let document_path = format!("{}?after={outcome}{draft_id}", published.document_path());
            Ok(after_publish(state, author, &published.doc_rkey, &document_path, draft).await)
        }
        Err(PublishError::SessionExpired) => Ok(render(
            state, nonce, StatusCode::UNAUTHORIZED, editing, form,
            &Outcome {
                publish_error: Some("Your publishing connection has expired. Your writing is still here. Sign in again, then retry publishing."),
                reauthenticate: true,
                ..Outcome::default()
            },
        )),
        Err(PublishError::App(err)) => Err(err),
        Err(PublishError::Home(message)) => {
            // The default subdomain could not be claimed; settings is
            // where an address is chosen by hand.
            tracing::warn!(message, "publish could not claim the default address");
            Ok(render(
                state,
                nonce,
                StatusCode::UNPROCESSABLE_ENTITY,
                editing,
                form,
                &Outcome {
                    publish_error: Some(
                        "Your feed's address could not be set up. Choose one in settings, then publish again.",
                    ),
                    ..Outcome::default()
                },
            ))
        }
        Err(PublishError::Repo(err)) => {
            tracing::warn!(error = %err, "publish refused");
            Ok(render(
                state,
                nonce,
                StatusCode::BAD_GATEWAY,
                editing,
                form,
                &Outcome {
                    publish_error: Some("Your server did not accept the write. Nothing was changed; try again in a moment."),
                    ..Outcome::default()
                },
            ))
        }
    }
}

/// The point to search near (plan 12, D44): where the request's IP is,
/// else the author's most recent visit with coordinates.
async fn locate(state: &AppState, identity: &Identity, ip: ClientIp) -> (Option<Point>, Located) {
    if let Some(here) = ip.0.and_then(|ip| state.geoip().locate(ip)) {
        return (Some(here.point), Located::Ip(here.city));
    }
    match last_visit_point(state, identity).await {
        Some(point) => (Some(point), Located::LastVisit),
        None => (None, Located::Unknown),
    }
}

/// The coordinates of the newest visit that has them, from the repo's
/// first page of documents.
async fn last_visit_point(state: &AppState, identity: &Identity) -> Option<Point> {
    let page = state.documents(identity, None).await.ok()?;
    page.records
        .into_iter()
        .filter_map(VisitDocument::from_record)
        .find_map(|doc| {
            let place = &doc.visit.place;
            Some(Point::from_e6(place.lat_e6?, place.lon_e6?))
        })
}

/// `action=pick:N`: take the Nth result of the suggestion search the
/// browser showed (a cache hit) as the place and move on to writing.
async fn pick(
    state: &AppState,
    nonce: &str,
    author: &Author,
    editing: Option<&Editing>,
    ip: ClientIp,
    mut form: EditorForm,
    index: usize,
) -> Response {
    let (point, located) = locate(state, &author.identity, ip).await;
    let hit = match point {
        Some(point) => state
            .search_places(&form.place_query, point)
            .await
            .ok()
            .and_then(|hits| hits.get(index).cloned()),
        None => None,
    };
    let outcome = match hit {
        Some(hit) => {
            form.pick(&hit);
            Outcome::default()
        }
        None => Outcome {
            pick_error: Some("That suggestion is gone; pick it again, or keep what you typed."),
            located,
            ..Outcome::default()
        },
    };
    render(state, nonce, StatusCode::OK, editing, &form, &outcome)
}

#[derive(Debug, Deserialize)]
pub struct SuggestQuery {
    #[serde(default)]
    q: String,
}

/// One place the listbox offers: its index into the same cached search
/// a pick re-reads, what to show, and the address the pick fills in.
#[derive(Debug, Serialize)]
struct Suggestion {
    i: usize,
    name: String,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    address: Option<String>,
}

#[derive(Debug, Serialize)]
struct Suggestions {
    q: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    near: Option<String>,
    hits: Vec<Suggestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

/// `GET /write/suggest?q=…` — places called `q` near the request, for
/// the choosing screen's suggestions (plan 12, D45). The same search a
/// pick re-reads, so a suggestion's index is a pick's index. Signed-in only, and no more than
/// [`SUGGESTS_PER_MINUTE`] a minute per author, since every uncached
/// call spends Open Places quota.
pub async fn suggest(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    ip: ClientIp,
    Query(query): Query<SuggestQuery>,
) -> Result<Response, AppError> {
    let q = query.q.trim().to_owned();
    let reply = |status: StatusCode, body: Suggestions| {
        let mut response = (status, Json(body)).into_response();
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        );
        Ok(response)
    };
    let empty = |error: Option<&'static str>| Suggestions {
        q: q.clone(),
        near: None,
        hits: Vec::new(),
        error,
    };
    if !state.suggest_limit().allow(did.as_str()) {
        return reply(StatusCode::TOO_MANY_REQUESTS, empty(Some("rate_limited")));
    }
    let identity = state.require_identity(&did).await?;
    let (point, located) = locate(&state, &identity, ip).await;
    let Some(point) = point else {
        return reply(StatusCode::OK, empty(Some("no_location")));
    };
    let near = match located {
        Located::Ip(city) => city,
        Located::LastVisit | Located::Unknown => None,
    };
    match state.search_places(&q, point).await {
        Ok(hits) => reply(
            StatusCode::OK,
            Suggestions {
                q: q.clone(),
                near,
                hits: hits
                    .iter()
                    .take(SUGGEST_LIMIT)
                    .enumerate()
                    .map(|(i, hit)| Suggestion {
                        i,
                        name: hit.name.clone(),
                        detail: match &hit.address {
                            Some(address) => format!("{address} · {:.1} mi", hit.distance_mi),
                            None => format!("{:.1} mi", hit.distance_mi),
                        },
                        address: hit.address.clone(),
                    })
                    .collect(),
                error: None,
            },
        ),
        // A prefix the API will not search yet is not an error worth a word.
        Err(SearchError::Query(_)) => reply(StatusCode::OK, empty(None)),
        Err(SearchError::Disabled | SearchError::Unavailable) => {
            reply(StatusCode::OK, empty(Some("unavailable")))
        }
    }
}

/// The document is published; now the Bluesky side (plan §5.7). With
/// the toggle on: post inline when the session may, else hand over to
/// the crosspost page, which asks for permission. A refused post lands
/// on the same page with the text kept, to retry or skip.
async fn after_publish(
    state: &AppState,
    author: &Author,
    rkey: &str,
    document_path: &str,
    draft: &editor::DocumentDraft,
) -> Response {
    let next = match &draft.crosspost {
        None => document_path.to_owned(),
        Some(text) if !author.posting.create => crosspost_path(rkey, text, false),
        Some(text) => match publish::crosspost(state, &author.identity, rkey, text).await {
            Ok(_) => document_path.to_owned(),
            Err(PublishError::SessionExpired) => {
                return Redirect::to(&format!("/login?return_to={}", urlencoding(document_path)))
                    .into_response()
            }
            Err(err) => {
                tracing::warn!(error = %err, "crosspost failed after publish");
                crosspost_path(rkey, text, true)
            }
        },
    };
    Redirect::to(&next).into_response()
}

#[derive(Debug, Deserialize)]
pub struct CrosspostQuery {
    #[serde(default)]
    text: String,
    #[serde(default)]
    failed: Option<String>,
}

const CROSSPOST_FAILED: &str =
    "Bluesky didn't accept the post. The digest is published; try again or skip.";

fn crosspost_response(
    status: StatusCode,
    did: &Did,
    author: &Author,
    editing: &Editing,
    post_text: &str,
    error: Option<&str>,
) -> Response {
    let state = match editing.post(did) {
        Some(post) => {
            CrosspostState::Posted(crate::bsky::post_url(post.did().as_str(), post.rkey()))
        }
        None if author.posting.create => CrosspostState::Ready,
        None => CrosspostState::NeedsPermission,
    };
    let default_text = editor::default_post_text(&editing.visit_doc.visit.place.name);
    let text = if post_text.trim().is_empty() {
        default_text
    } else {
        post_text.trim().to_owned()
    };
    let title = &editing.visit_doc.document().title;
    let page = layout::render(&Page {
        title: &["Bluesky", title],
        main: view::crosspost_page(&CrosspostPage {
            rkey: &editing.rkey,
            title,
            document_path: &editing.document_path(did),
            post_text: &text,
            error,
            state,
        }),
        ..Page::default()
    });
    (status, page).into_response()
}

/// `GET /write/{rkey}/crosspost` — post a published write-up to Bluesky,
/// or see that it is there.
pub async fn crosspost_form(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    Path(rkey): Path<String>,
    Query(query): Query<CrosspostQuery>,
) -> Result<Response, AppError> {
    let author = author(&state, &did).await?;
    let editing = editing(&state, &author, &rkey).await?;
    Ok(crosspost_response(
        StatusCode::OK,
        &did,
        &author,
        &editing,
        &query.text,
        query.failed.as_deref().map(|_| CROSSPOST_FAILED),
    ))
}

#[derive(Debug, Deserialize)]
pub struct CrosspostForm {
    #[serde(default)]
    post_text: String,
}

/// `POST /write/{rkey}/crosspost` — do it.
pub async fn crosspost_submit(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    Path(rkey): Path<String>,
    Form(form): Form<CrosspostForm>,
) -> Result<Response, AppError> {
    let author = author(&state, &did).await?;
    let editing = editing(&state, &author, &rkey).await?;
    let text = form.post_text.trim();
    if text.graphemes(true).count() > MAX_POST_GRAPHEMES {
        return Ok(crosspost_response(
            StatusCode::UNPROCESSABLE_ENTITY,
            &did,
            &author,
            &editing,
            text,
            Some("Keep the post under 300 characters."),
        ));
    }
    if !author.posting.create {
        return Ok(crosspost_response(
            StatusCode::OK,
            &did,
            &author,
            &editing,
            text,
            None,
        ));
    }
    match publish::crosspost(&state, &author.identity, &rkey, text).await {
        Ok(_) => Ok(Redirect::to(&editing.document_path(&did)).into_response()),
        Err(PublishError::SessionExpired) => Ok(Redirect::to(&format!(
            "/login?return_to={}",
            urlencoding(&crosspost_path(&rkey, text, false))
        ))
        .into_response()),
        Err(PublishError::App(err)) => Err(err),
        Err(PublishError::Home(message)) => Err(AppError::Upstream(message)),
        Err(PublishError::Repo(err)) => {
            tracing::warn!(error = %err, "crosspost refused");
            Ok(crosspost_response(
                StatusCode::BAD_GATEWAY,
                &did,
                &author,
                &editing,
                text,
                Some(CROSSPOST_FAILED),
            ))
        }
    }
}

/// `GET /write/{rkey}/delete` — are you sure?
pub async fn delete_form(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    Path(rkey): Path<String>,
) -> Result<Response, AppError> {
    let author = author(&state, &did).await?;
    let editing = editing(&state, &author, &rkey).await?;
    let title = editing.visit_doc.document().title.clone();
    let post = match editing.post(&did) {
        None => DeletePost::None,
        Some(_) if author.posting.delete => DeletePost::Offered,
        Some(post) => {
            DeletePost::NotPermitted(crate::bsky::post_url(post.did().as_str(), post.rkey()))
        }
    };
    Ok(layout::render(&Page {
        title: &["Delete", &title],
        main: view::delete_page(&DeletePage {
            rkey: &rkey,
            title: &title,
            post,
        }),
        ..Page::default()
    })
    .into_response())
}

#[derive(Debug, Deserialize, Default)]
pub struct DeleteForm {
    #[serde(default)]
    delete_post: Option<String>,
}

/// `POST /write/{rkey}/delete` — delete, then back to the publication.
pub async fn delete_submit(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    Path(rkey): Path<String>,
    form: Result<Form<DeleteForm>, FormRejection>,
) -> Result<Response, AppError> {
    let author = author(&state, &did).await?;
    let editing = editing(&state, &author, &rkey).await?;
    let back = AtUri::parse(&editing.visit_doc.document().site).map_or_else(
        |_| paths::repo(&did),
        |site| paths::publication(&did, site.rkey()),
    );
    // A body that is not a form (none at all, say) means the default: no.
    let delete_post = form.is_ok_and(|Form(f)| f.delete_post.is_some()) && author.posting.delete;
    let post = if delete_post {
        editing.post(&did)
    } else {
        None
    };
    match publish::delete(&state, &author.identity, &rkey, post.as_ref()).await {
        Ok(()) => Ok(Redirect::to(&back).into_response()),
        Err(PublishError::SessionExpired) => {
            Ok(Redirect::to(&format!("/login?return_to=/write/{rkey}/delete")).into_response())
        }
        Err(PublishError::App(err)) => Err(err),
        Err(PublishError::Home(message)) => Err(AppError::Upstream(message)),
        Err(PublishError::Repo(err)) => {
            tracing::warn!(error = %err, "delete refused");
            Err(AppError::Upstream(err.to_string()))
        }
    }
}
