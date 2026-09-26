//! `GET/POST /write/{rkey}/photos` — the photos of a published write-up
//! (plan 07), the way they are managed without script. Each action
//! uploads or rearranges and writes the record at once; asked for JSON
//! (`Accept: application/json`), it answers with the list instead of
//! the page.
//!
//! `POST /write/upload` and `GET /write/photo/{cid}` serve the editor's
//! photos island (D37 amended): a file is uploaded to the author's
//! repository the moment it is picked and its blob reference rides in
//! the form until the record is written, and its tile is drawn from the
//! author's own blob whether or not a record lists it yet.

use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Redirect, Response};
use eaten_at_atproto::at_uri::AtUri;
use eaten_at_atproto::identity::{Did, Identity};
use eaten_at_atproto::lexicon::{AspectRatio, Photo, MAX_PHOTOS};
use eaten_at_web::layout::{self, urlencoding, Page};
use serde::{Deserialize, Serialize};

use crate::auth::RequireUser;
use crate::editor::photos::{
    self, AltError, PhotoRow, PhotosAction, PhotosForm, PhotosPage, MAX_FILES_PER_REQUEST,
};
use crate::error::AppError;
use crate::img::{self, ImageError, PhotoSize, MAX_PHOTO_UPLOAD_BYTES};
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
        .ok_or_else(|| AppError::BadRequest("that document is not a digest".to_owned()))?;
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

/// How the caller wants the outcome: the page, or JSON for the editor's
/// island.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Wants {
    Page,
    Json,
}

impl Wants {
    /// JSON only when it is asked for by name; a browser's `Accept`
    /// names HTML first, and anything else gets the page.
    fn from_headers(headers: &HeaderMap) -> Self {
        let accept = headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default();
        if accept
            .split(',')
            .any(|part| part.trim().starts_with("application/json"))
        {
            Self::Json
        } else {
            Self::Page
        }
    }
}

/// One photo as the island shows it, and, from an upload, the blob's
/// facts the form carries.
#[derive(Debug, Serialize)]
struct PhotoJson {
    cid: String,
    thumb: String,
    full: String,
    alt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    mime: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    height: Option<u32>,
}

/// The outcome of an action, for the island.
#[derive(Debug, Serialize)]
struct PhotosJson {
    photos: Vec<PhotoJson>,
    /// Problems with the files just chosen, and a refused alt text, in
    /// the author's words.
    problems: Vec<String>,
    /// Why the write failed, when it did.
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

fn render(
    status: StatusCode,
    wants: Wants,
    did: &Did,
    visit_doc: &VisitDocument,
    photos: &[Photo],
    query: &PhotosQuery,
    outcome: &Outcome<'_>,
) -> Response {
    let rkey = visit_doc.rkey();
    if wants == Wants::Json {
        let mut problems = outcome.problems.clone();
        if let Some(alt_error) = &outcome.alt_error {
            problems.push(alt_error.message.clone());
        }
        let body = PhotosJson {
            photos: photos
                .iter()
                .map(|photo| PhotoJson {
                    cid: photo.image.cid().to_owned(),
                    thumb: paths::photo(did, rkey, photo.image.cid(), "thumb"),
                    full: paths::photo(did, rkey, photo.image.cid(), "full"),
                    alt: photo.alt.clone().unwrap_or_default(),
                    mime: None,
                    size: None,
                    width: None,
                    height: None,
                })
                .collect(),
            problems,
            error: outcome.error.map(str::to_owned),
        };
        let mut response = (status, Json(body)).into_response();
        response.headers_mut().insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, no-store"),
        );
        return response;
    }
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
        Wants::Page,
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
    headers: HeaderMap,
    multipart: Multipart,
) -> Result<Response, AppError> {
    let wants = Wants::from_headers(&headers);
    let (identity, visit_doc) = load(&state, &did, &rkey).await?;
    let form = PhotosForm::from_multipart(multipart)
        .await
        .map_err(|e| AppError::BadRequest(format!("could not read the form: {e}")))?;
    let current = visit_doc.visit.photos.clone();
    let action = form.action.clone().unwrap_or(PhotosAction::Add);

    // The alt texts as typed apply to every action, so a caption typed
    // before "Move up" is not lost.
    let photos = match photos::with_alts(current.clone(), &form.alts) {
        Ok(photos) => photos,
        Err(alt_error) => {
            return Ok(render(
                StatusCode::UNPROCESSABLE_ENTITY,
                wants,
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
                    wants,
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
        other => (photos::rearranged(photos, &other), Outcome::default()),
    };

    // Nothing to write when nothing changed (a save with no edits, an
    // add with no usable file).
    if photos == current {
        let status = if outcome.problems.is_empty() {
            StatusCode::OK
        } else {
            StatusCode::UNPROCESSABLE_ENTITY
        };
        return Ok(render(
            status, wants, &did, &visit_doc, &current, &query, &outcome,
        ));
    }
    match publish::save_photos(&state, &identity, &visit_doc, photos.clone()).await {
        Ok(()) => Ok(render(
            StatusCode::OK,
            wants,
            &did,
            &visit_doc,
            &photos,
            &query,
            &outcome,
        )),
        Err(PublishError::SessionExpired) => Ok(login_redirect(&query.action_path(&rkey))),
        Err(PublishError::App(err)) => Err(err),
        Err(PublishError::Home(message)) => Err(AppError::Upstream(message)),
        Err(PublishError::Repo(err)) => {
            tracing::warn!(error = %err, "photos write refused");
            Ok(render(
                StatusCode::BAD_GATEWAY,
                wants,
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

/// `POST /write/upload` — the editor's photos island sends the files it
/// was given; each good one is prepared, uploaded to the author's
/// repository, and answered as the reference the form will carry. The
/// record is not touched: Publish or Save writes the references with it.
/// A blob nothing ever references is the repository's to forget.
pub async fn upload(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    multipart: Multipart,
) -> Result<Response, AppError> {
    let identity = state.require_identity(&did).await?;
    let form = PhotosForm::from_multipart(multipart)
        .await
        .map_err(|e| AppError::BadRequest(format!("could not read the form: {e}")))?;
    let (photos, problems, status) = match add(&state, &identity, Vec::new(), &form).await {
        Ok((photos, problems)) if photos.is_empty() => {
            (photos, problems, StatusCode::UNPROCESSABLE_ENTITY)
        }
        Ok((photos, problems)) => (photos, problems, StatusCode::OK),
        Err(Refused::Session) => return Ok(login_redirect("/write")),
        Err(Refused::Repo(mut problems)) => {
            problems.push(WRITE_REFUSED.to_owned());
            (Vec::new(), problems, StatusCode::BAD_GATEWAY)
        }
    };
    let body = PhotosJson {
        photos: photos.iter().map(PhotoJson::own).collect(),
        problems,
        error: None,
    };
    let mut response = (status, Json(body)).into_response();
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    Ok(response)
}

impl PhotoJson {
    /// A photo as the editor draws it: through the author's own blob,
    /// whether or not a record lists it yet.
    fn own(photo: &Photo) -> Self {
        let cid = photo.image.cid();
        Self {
            cid: cid.to_owned(),
            thumb: paths::own_photo(cid, "thumb"),
            full: paths::own_photo(cid, "full"),
            alt: photo.alt.clone().unwrap_or_default(),
            mime: Some(photo.image.mime_type.clone()),
            size: Some(photo.image.size),
            width: photo.aspect_ratio.map(|r| r.width),
            height: photo.aspect_ratio.map(|r| r.height),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct OwnPhotoQuery {
    size: Option<String>,
}

/// `GET /write/photo/{cid}` — one of the signed-in author's own blobs,
/// re-encoded at `thumb` or `full`, for the editor's tiles. Only the
/// author sees their own; a reader's way to a photo is the document's
/// proxy, which serves the CIDs a record lists and nothing else.
pub async fn own_photo(
    State(state): State<AppState>,
    RequireUser(did): RequireUser,
    Path(cid): Path<String>,
    Query(query): Query<OwnPhotoQuery>,
) -> Result<Response, AppError> {
    if cid.is_empty() || !cid.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(AppError::NotFound("that is not a blob".to_owned()));
    }
    let identity = state.require_identity(&did).await?;
    let size = PhotoSize::from_query(query.size.as_deref());
    let rendition = state
        .own_photo_rendition(&identity, &cid, size)
        .await
        .ok_or_else(|| AppError::NotFound(format!("photo {cid} not found")))?;
    let response = Response::builder()
        .header(header::CONTENT_TYPE, HeaderValue::from_static("image/jpeg"))
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("inline; filename=\"photo.jpg\""),
        )
        .header(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        )
        .header(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("default-src 'none'; sandbox"),
        )
        .header(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, max-age=3600"),
        )
        .header(header::VARY, "Cookie")
        .header(header::CONTENT_LENGTH, rendition.jpeg.len())
        .body(Body::from(rendition.jpeg))
        .map_err(|e| AppError::Upstream(e.to_string()))?;
    Ok(response)
}

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
            "At most {MAX_PHOTOS} photos on a digest; remove some first."
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
        let blob = match session
            .upload_blob(prepared.jpeg.clone(), "image/jpeg")
            .await
        {
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
        if let Err(err) = state
            .cache_uploaded_photo(identity, blob.cid(), prepared.jpeg)
            .await
        {
            tracing::warn!(error = %err, "photo preview preparation failed");
            problems.push(format!(
                "{name} could not be prepared for preview. Please try again."
            ));
            return Err(Refused::Repo(problems));
        }
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
