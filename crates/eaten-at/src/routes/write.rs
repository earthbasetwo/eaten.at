//! `GET/POST /write` and `/write/{doc_rkey}` — the editor (plan §7.5),
//! with its delete and crosspost pages.
//!
//! Every submission comes back as the same page: a structural action
//! (add or remove a row) re-renders the form; a preview validates it and
//! shows the draft above the form with any problems beside their fields;
//! publish writes the records and, when asked, posts to Bluesky.

use axum::extract::rejection::FormRejection;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use eaten_at_atproto::at_uri::AtUri;
use eaten_at_atproto::identity::{Did, Identity};
use eaten_at_atproto::lexicon::at_eaten::Preferences;
use eaten_at_atproto::lexicon::Publication;
use eaten_at_atproto::repo::Record;
use eaten_at_web::assets::EDITOR_SCRIPT;
use eaten_at_web::layout::{self, urlencoding, Page, Width};
use serde::Deserialize;
use unicode_segmentation::UnicodeSegmentation;

use crate::auth::{PostPermission, RequireUser};
use crate::editor::view::{
    self, CrosspostPage, CrosspostState, DeletePage, DeletePost, EditorPage, PublicationOption,
};
use crate::editor::{self, Action, Context, EditorForm, FieldErrors, PUBLICATION_NEW};
use crate::error::AppError;
use crate::model::VisitDocument;
use crate::paths;
use crate::publish::{self, PublishError, MAX_POST_GRAPHEMES};
use crate::read::PublicationChoice;
use crate::security::Nonce;
use crate::state::AppState;

/// What the author has to write into.
struct Author {
    identity: Identity,
    publications: Vec<Record<Publication>>,
    /// The preselected publication: the preference, or the only one.
    default: String,
    preferences: Preferences,
    /// What the sign-in may do with Bluesky posts.
    posting: PostPermission,
}

async fn author(state: &AppState, did: &Did) -> Result<Author, AppError> {
    let identity = state.require_identity(did).await?;
    let publications = state.publications(&identity).await?;
    let default = match state.choose_publication(&identity).await? {
        PublicationChoice::Chosen(publication) => publication.uri.as_str().to_owned(),
        PublicationChoice::Choose(_) | PublicationChoice::None => PUBLICATION_NEW.to_owned(),
    };
    let preferences = state.preferences(&identity).await?;
    // A session whose grant cannot be read is treated as one that may
    // not post; the worst case is being asked to allow it again.
    let posting = match state.oauth().granted_scopes(did).await {
        Ok(scopes) => crate::auth::post_permission(&scopes),
        Err(err) => {
            tracing::warn!(%did, error = %err, "could not read the session's scopes");
            PostPermission::default()
        }
    };
    Ok(Author {
        identity,
        publications,
        default,
        preferences,
        posting,
    })
}

impl Author {
    fn options(&self) -> Vec<PublicationOption> {
        self.publications
            .iter()
            .map(|p| PublicationOption {
                uri: p.uri.as_str().to_owned(),
                name: p.value.name.clone(),
            })
            .collect()
    }

    fn uris(&self) -> Vec<AtUri> {
        self.publications.iter().map(|p| p.uri.clone()).collect()
    }
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
        .ok_or_else(|| AppError::BadRequest("that document is not a visit".to_owned()))?;
    Ok(Editing {
        rkey: rkey.to_owned(),
        visit_doc,
    })
}

/// What the page shows besides the form.
#[derive(Default)]
struct Outcome<'a> {
    errors: FieldErrors,
    preview: Option<maud::Markup>,
    publish_error: Option<&'a str>,
}

fn render(
    nonce: &str,
    status: StatusCode,
    author: &Author,
    editing: Option<&Editing>,
    form: &EditorForm,
    outcome: &Outcome<'_>,
) -> Response {
    let action_path = editing.map_or_else(|| "/write".to_owned(), |e| format!("/write/{}", e.rkey));
    let options = author.options();
    let crosspost =
        match editing.and_then(|e| crate::view::bluesky_post_url(e.visit_doc.document())) {
            Some(url) => CrosspostState::Posted(url),
            None if author.posting.create => CrosspostState::Ready,
            None => CrosspostState::NeedsPermission,
        };
    let page = layout::render(&Page {
        title: &[if editing.is_some() { "Edit" } else { "Write" }],
        width: Width::Wide,
        nonce: Some(nonce.to_owned()),
        scripts: vec![EDITOR_SCRIPT],
        main: view::page(&EditorPage {
            form,
            errors: &outcome.errors,
            publications: &options,
            action_path: &action_path,
            editing: editing.is_some(),
            heading: editing.map(|e| e.visit_doc.document().title.as_str()),
            preview: outcome.preview.clone(),
            publish_error: outcome.publish_error,
            crosspost,
        }),
        ..Page::default()
    });
    (status, page).into_response()
}

/// `GET /write` — a blank editor.
pub async fn new_form(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    nonce: Nonce,
) -> Result<Response, AppError> {
    let nonce = nonce.0.as_str();
    let author = author(&state, &did).await?;
    let mut form = EditorForm::blank(&author.default);
    form.crosspost = author.preferences.crosspost_default();
    let outcome = Outcome::default();
    Ok(render(
        nonce,
        StatusCode::OK,
        &author,
        None,
        &form,
        &outcome,
    ))
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
        nonce,
        StatusCode::OK,
        &author,
        Some(&editing),
        &form,
        &outcome,
    ))
}

/// `POST /write` — a submission for a new write-up.
pub async fn submit_new(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    nonce: Nonce,
    form: Result<Form<Vec<(String, String)>>, FormRejection>,
) -> Result<Response, AppError> {
    let author = author(&state, &did).await?;
    submit(&state, &author, None, &nonce.0, form).await
}

/// `POST /write/{rkey}` — a submission for an existing write-up.
pub async fn submit_edit(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    Path(rkey): Path<String>,
    nonce: Nonce,
    form: Result<Form<Vec<(String, String)>>, FormRejection>,
) -> Result<Response, AppError> {
    let author = author(&state, &did).await?;
    let editing = editing(&state, &author, &rkey).await?;
    submit(&state, &author, Some(&editing), &nonce.0, form).await
}

async fn submit(
    state: &AppState,
    author: &Author,
    editing: Option<&Editing>,
    nonce: &str,
    form: Result<Form<Vec<(String, String)>>, FormRejection>,
) -> Result<Response, AppError> {
    let Form(pairs) =
        form.map_err(|e| AppError::BadRequest(format!("could not read the form: {e}")))?;
    let (mut form, action) = EditorForm::from_pairs(pairs);
    let action = action.unwrap_or(Action::Preview);
    if !matches!(action, Action::Preview | Action::Publish) {
        form.apply(&action);
        return Ok(render(
            nonce,
            StatusCode::OK,
            author,
            editing,
            &form,
            &Outcome::default(),
        ));
    }
    let uris = author.uris();
    let context = Context {
        publications: &uris,
        original: editing.map(|e| &e.visit_doc.visit),
    };
    let draft = match editor::validate(&form, &context) {
        Ok(draft) => draft,
        Err(errors) => {
            return Ok(render(
                nonce,
                StatusCode::UNPROCESSABLE_ENTITY,
                author,
                editing,
                &form,
                &Outcome {
                    errors,
                    ..Outcome::default()
                },
            ))
        }
    };
    if action == Action::Preview {
        return Ok(render(
            nonce,
            StatusCode::OK,
            author,
            editing,
            &form,
            &Outcome {
                preview: Some(view::preview(&draft)),
                ..Outcome::default()
            },
        ));
    }
    match publish::publish(
        state,
        &author.identity,
        &draft,
        editing.map(|e| &e.visit_doc),
    )
    .await
    {
        Ok(published) => {
            let document_path =
                paths::document(&published.did, &published.pub_rkey, &published.doc_rkey);
            Ok(after_publish(state, author, &published.doc_rkey, &document_path, &draft).await)
        }
        Err(PublishError::SessionExpired) => {
            Ok(Redirect::to("/login?return_to=/write").into_response())
        }
        Err(PublishError::App(err)) => Err(err),
        Err(PublishError::Repo(err)) => {
            tracing::warn!(error = %err, "publish refused");
            Ok(render(
                nonce,
                StatusCode::BAD_GATEWAY,
                author,
                editing,
                &form,
                &Outcome {
                    publish_error: Some("Your server did not accept the write. Nothing was changed; try again in a moment."),
                    ..Outcome::default()
                },
            ))
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
    let Some(text) = &draft.crosspost else {
        return Redirect::to(document_path).into_response();
    };
    if !author.posting.create {
        return Redirect::to(&crosspost_path(rkey, text, false)).into_response();
    }
    match publish::crosspost(state, &author.identity, rkey, text).await {
        Ok(_) => Redirect::to(document_path).into_response(),
        Err(PublishError::SessionExpired) => {
            Redirect::to(&format!("/login?return_to={}", urlencoding(document_path)))
                .into_response()
        }
        Err(err) => {
            tracing::warn!(error = %err, "crosspost failed after publish");
            Redirect::to(&crosspost_path(rkey, text, true)).into_response()
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CrosspostQuery {
    #[serde(default)]
    text: String,
    #[serde(default)]
    failed: Option<String>,
}

const CROSSPOST_FAILED: &str =
    "Bluesky didn't accept the post. The write-up is published; try again or skip.";

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
        Err(PublishError::Repo(err)) => {
            tracing::warn!(error = %err, "delete refused");
            Err(AppError::Upstream(err.to_string()))
        }
    }
}
