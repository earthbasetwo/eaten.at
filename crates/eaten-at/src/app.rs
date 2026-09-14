//! Router construction.

use axum::extract::DefaultBodyLimit;
use axum::{
    middleware,
    routing::{get, post},
    Router,
};

use crate::editor::MAX_REQUEST_BYTES;
use crate::hosting;
use crate::routes::{
    assets, auth, document, feed, image, interstitial, landing, lookup, photos, publication,
    settings, write,
};
use crate::state::AppState;

/// Build the application router over `state`.
///
/// Hosted-subdomain routing rewrites the request URI, which has to happen
/// before routing; a layer on the router itself would run after it. So
/// the routes are wrapped, and the outer router only forwards to them.
pub fn router(state: AppState) -> Router {
    let routes = routes(state.clone());
    let by_host = middleware::from_fn_with_state(state, hosting::by_host);
    Router::new().fallback_service(tower::Layer::layer(&by_host, routes))
}

fn routes(state: AppState) -> Router {
    // The editor accepts a long write-up, so its body cap is its own.
    let editor = Router::new()
        .route("/write", get(write::new_form).post(write::submit_new))
        .route("/write/suggest", get(write::suggest))
        .route(
            "/write/{rkey}",
            get(write::edit_form).post(write::submit_edit),
        )
        .route(
            "/write/{rkey}/delete",
            get(write::delete_form).post(write::delete_submit),
        )
        .route(
            "/write/{rkey}/crosspost",
            get(write::crosspost_form).post(write::crosspost_submit),
        )
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BYTES));
    // The photos page takes files, so its body cap is its own.
    let photos = Router::new()
        .route(
            "/write/{rkey}/photos",
            get(photos::photos_form).post(photos::photos_submit),
        )
        .layer(DefaultBodyLimit::max(
            crate::editor::photos::MAX_REQUEST_BYTES,
        ));
    Router::new()
        .merge(editor)
        .merge(photos)
        .route("/healthz", get(healthz))
        .route("/static/{file}", get(assets::static_file))
        .route("/img/{did}/{doc_rkey}", get(image::cover))
        .route("/img/{did}/{doc_rkey}/{cid}", get(image::photo))
        .route("/", get(landing::landing))
        .route("/login", get(auth::login_form).post(auth::login_start))
        .route("/login/bluesky", post(auth::authorize_bluesky))
        .route("/oauth/callback", get(auth::callback))
        .route("/logout", post(auth::logout))
        .route("/client-metadata.json", get(auth::client_metadata))
        .route("/settings", get(settings::settings).post(settings::save))
        .route(
            "/at/{did}/{pub_rkey}/publication.json",
            get(publication::publication_json),
        )
        .route("/labels/continue", post(interstitial::acknowledge))
        .route("/lookup", get(lookup::lookup))
        .route("/@{handle}", get(lookup::handle_root))
        .route("/@{handle}/", get(lookup::handle_root))
        .route("/@{handle}/{*rest}", get(lookup::handle_rest))
        .route("/at/{did}", get(publication::repo_index))
        .route("/at/{did}/", get(publication::repo_index))
        .route("/at/{did}/{pub_rkey}", get(publication::publication_page))
        .route("/at/{did}/{pub_rkey}/", get(publication::publication_page))
        .route(
            "/at/{did}/{pub_rkey}/tagged/{tag}",
            get(publication::tagged_page),
        )
        .route("/at/{did}/{pub_rkey}/feed.xml", get(feed::feed))
        .route(
            "/at/{did}/{pub_rkey}/{doc_rkey}",
            get(document::document_page),
        )
        .layer(middleware::from_fn(crate::security::headers))
        .with_state(state)
}

async fn healthz() -> &'static str {
    "ok"
}
