//! `GET/POST /write/{rkey}/photos` — the photos of a published write-up
//! (plan 07). Each action uploads or rearranges and writes the record at
//! once.

use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use eaten_at_atproto::at_uri::AtUri;
use eaten_at_atproto::identity::{Did, Identity};
use eaten_at_atproto::lexicon::{AspectRatio, Photo, MAX_PHOTOS};
use eaten_at_web::layout::{self, urlencoding, Page};
use serde::Deserialize;

use crate::auth::RequireUser;
use crate::editor::photos::{
    self, AltError, PhotoRow, PhotosAction, PhotosForm, PhotosPage, MAX_FILES_PER_REQUEST,
};
use crate::error::AppError;
use crate::img::{self, ImageError, MAX_PHOTO_UPLOAD_BYTES};
use crate::model::VisitDocument;
use crate::paths;
use crate::publish::{self, PublishError};
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct PhotosQuery {
    /// Set by the handover after a first publish.
    #[serde(default)]
    new: Option<String>,
    /// Where to go when done: a local path, else ignored.
    #[serde(default)]
    then: Option<String>,
}

impl PhotosQuery {
    /// The continuation, only when it is a path on this site.
    fn then(&self) -> Option<&str> {
        self.then
            .as_deref()
            .filter(|t| t.starts_with('/') && !t.starts_with("//"))
    }

    /// The page's own path with the handover kept.
    fn action_path(&self, rkey: &str) -> String {
        let mut path = format!("/write/{rkey}/photos");
        let mut sep = '?';
        if self.new.is_some() {
            path.push_str("?new=1");
            sep = '&';
        }
        if let Some(then) = self.then() {
            path.push(sep);
            path.push_str("then=");
            path.push_str(&urlencoding(then));
        }
        path
    }
}

/// The author's visit document, or why not.
async fn load(
    state: &AppState,
    did: &Did,
    rkey: &str,
) -> Result<(Identity, VisitDocument), AppError> {
    let identity = state.require_identity(did).await?;
    let record = state
        .document(&identity, rkey)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("document {rkey} not found")))?;
    let visit_doc = VisitDocument::from_record(record)
        .ok_or_else(|| AppError::BadRequest("that document is not a visit".to_owned()))?;
    Ok((identity, visit_doc))
}

fn document_path(did: &Did, visit_doc: &VisitDocument) -> String {
    AtUri::parse(&visit_doc.document().site).map_or_else(
        |_| paths::repo(did),
        |site| paths::document(did, site.rkey(), visit_doc.rkey()),
    )
}

/// What the page shows besides the photos.
#[derive(Default)]
struct Outcome<'a> {
    problems: Vec<String>,
    alt_error: Option<AltError>,
    error: Option<&'a str>,
}

fn render(
    status: StatusCode,
    did: &Did,
    visit_doc: &VisitDocument,
    photos: &[Photo],
    query: &PhotosQuery,
    outcome: &Outcome<'_>,
) -> Response {
    let rkey = visit_doc.rkey();
    let rows: Vec<PhotoRow> = photos
        .iter()
        .map(|photo| PhotoRow {
            thumb_src: paths::photo(did, rkey, photo.image.cid(), "thumb"),
            full_src: paths::photo(did, rkey, photo.image.cid(), "full"),
            alt: photo.alt.clone().unwrap_or_default(),
        })
        .collect();
    let place_name = &visit_doc.visit.place.name;
    let page = layout::render(&Page {
        title: &["Photos", place_name],
        main: photos::page(&PhotosPage {
            rkey,
            place_name,
            action_path: &query.action_path(rkey),
            document_path: &document_path(did, visit_doc),
            then: query.new.as_ref().map(|_| query.then().unwrap_or("/")),
            photos: &rows,
            problems: &outcome.problems,
            alt_error: outcome.alt_error.as_ref(),
            error: outcome.error,
        }),
        ..Page::default()
    });
    (status, page).into_response()
}

/// `GET /write/{rkey}/photos`.
pub async fn photos_form(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    Path(rkey): Path<String>,
    Query(query): Query<PhotosQuery>,
) -> Result<Response, AppError> {
    let (_, visit_doc) = load(&state, &did, &rkey).await?;
    Ok(render(
        StatusCode::OK,
        &did,
        &visit_doc,
        &visit_doc.visit.photos,
        &query,
        &Outcome::default(),
    ))
}

/// `POST /write/{rkey}/photos`.
pub async fn photos_submit(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    Path(rkey): Path<String>,
    Query(query): Query<PhotosQuery>,
    multipart: Multipart,
) -> Result<Response, AppError> {
    let (identity, visit_doc) = load(&state, &did, &rkey).await?;
    let form = PhotosForm::from_multipart(multipart)
        .await
        .map_err(|e| AppError::BadRequest(format!("could not read the form: {e}")))?;
    let current = visit_doc.visit.photos.clone();
    let action = form.action.unwrap_or(PhotosAction::Add);

    // The alt texts as typed apply to every action, so a caption typed
    // before "Move up" is not lost.
    let photos = match photos::with_alts(current.clone(), &form.alts) {
        Ok(photos) => photos,
        Err(alt_error) => {
            return Ok(render(
                StatusCode::UNPROCESSABLE_ENTITY,
                &did,
                &visit_doc,
                &current,
                &query,
                &Outcome {
                    alt_error: Some(alt_error),
                    ..Outcome::default()
                },
            ))
        }
    };

    let (photos, outcome) = match action {
        PhotosAction::Add => match add(&state, &identity, photos, &form).await {
            Ok((photos, problems)) => (
                photos,
                Outcome {
                    problems,
                    ..Outcome::default()
                },
            ),
            Err(Refused::Session) => {
                return Ok(login_redirect(&query.action_path(&rkey)));
            }
            Err(Refused::Repo(problems)) => {
                return Ok(render(
                    StatusCode::BAD_GATEWAY,
                    &did,
                    &visit_doc,
                    &current,
                    &query,
                    &Outcome {
                        problems,
                        error: Some(WRITE_REFUSED),
                        ..Outcome::default()
                    },
                ))
            }
        },
        PhotosAction::Save => (photos, Outcome::default()),
        other => (photos::rearranged(photos, other), Outcome::default()),
    };

    // Nothing to write when nothing changed (a save with no edits, an
    // add with no usable file).
    if photos == current {
        let status = if outcome.problems.is_empty() {
            StatusCode::OK
        } else {
            StatusCode::UNPROCESSABLE_ENTITY
        };
        return Ok(render(status, &did, &visit_doc, &current, &query, &outcome));
    }
    match publish::save_photos(&state, &identity, &visit_doc, photos.clone()).await {
        Ok(()) => Ok(render(
            StatusCode::OK,
            &did,
            &visit_doc,
            &photos,
            &query,
            &outcome,
        )),
        Err(PublishError::SessionExpired) => Ok(login_redirect(&query.action_path(&rkey))),
        Err(PublishError::App(err)) => Err(err),
        Err(PublishError::Repo(err)) => {
            tracing::warn!(error = %err, "photos write refused");
            Ok(render(
                StatusCode::BAD_GATEWAY,
                &did,
                &visit_doc,
                &current,
                &query,
                &Outcome {
                    error: Some(WRITE_REFUSED),
                    ..outcome
                },
            ))
        }
    }
}

const WRITE_REFUSED: &str =
    "Your server did not accept the change. Nothing was changed; try again in a moment.";

fn login_redirect(return_to: &str) -> Response {
    Redirect::to(&format!("/login?return_to={}", urlencoding(return_to))).into_response()
}

/// Why an add could not finish.
enum Refused {
    Session,
    /// A blob upload was refused; the problems so far are kept for the
    /// page.
    Repo(Vec<String>),
}

/// Prepare and upload the chosen files, appending each good one. Files
/// that cannot be used are reported by name; the good ones still go in.
async fn add(
    state: &AppState,
    identity: &Identity,
    mut photos: Vec<Photo>,
    form: &PhotosForm,
) -> Result<(Vec<Photo>, Vec<String>), Refused> {
    let mut problems = Vec::new();
    if form.too_many {
        problems.push(format!(
            "At most {MAX_FILES_PER_REQUEST} photos at a time; the rest were not added."
        ));
    }
    if form.files.is_empty() {
        problems.push("Choose at least one photo.".to_owned());
        return Ok((photos, problems));
    }
    if photos.len() + form.files.len() > MAX_PHOTOS {
        problems.push(format!(
            "At most {MAX_PHOTOS} photos on a visit; remove some first."
        ));
        return Ok((photos, problems));
    }
    let session =
        state.oauth().session(&identity.did).await.map_err(|err| {
            match PublishError::from(err) {
                PublishError::SessionExpired => Refused::Session,
                _ => Refused::Repo(problems.clone()),
            }
        })?;
    for upload in &form.files {
        let name = if upload.file_name.trim().is_empty() {
            "A file".to_owned()
        } else {
            upload.file_name.clone()
        };
        let bytes = upload.bytes.clone();
        let prepared = tokio::task::spawn_blocking(move || img::photo_upload(&bytes)).await;
        let prepared = match prepared {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(err)) => {
                tracing::debug!(%err, file = %name, "photo rejected");
                problems.push(match err {
                    ImageError::TooBig(..) => format!(
                        "{name} is over {} MB.",
                        MAX_PHOTO_UPLOAD_BYTES / (1024 * 1024)
                    ),
                    _ => format!(
                        "{name} isn't an image we can use. JPEG, PNG, GIF, or WebP, please."
                    ),
                });
                continue;
            }
            Err(err) => {
                tracing::warn!(%err, "photo task failed");
                problems.push(format!("{name} could not be processed."));
                continue;
            }
        };
        let blob = match session.upload_blob(prepared.jpeg, "image/jpeg").await {
            Ok(blob) => blob,
            Err(err) => {
                return Err(match PublishError::from(err) {
                    PublishError::SessionExpired => Refused::Session,
                    other => {
                        tracing::warn!(error = %other, "photo upload refused");
                        Refused::Repo(problems)
                    }
                })
            }
        };
        photos.push(Photo {
            image: blob,
            alt: None,
            aspect_ratio: Some(AspectRatio {
                width: prepared.width,
                height: prepared.height,
            }),
            extra: serde_json::Map::new(),
        });
    }
    Ok((photos, problems))
}
