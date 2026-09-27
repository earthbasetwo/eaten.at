//! `/static/{file}` — embedded assets: the stylesheet, the fonts and the
//! icons; and the two icon paths browsers ask for unbidden.

use axum::extract::Path;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use eaten_at_web::assets;

pub async fn static_file(Path(file): Path<String>) -> Response {
    serve(&file)
}

/// `/favicon.ico`: what a browser asks for with no `<link rel="icon">`
/// to go by (a bookmark, a feed reader, an old tab). PNG is accepted
/// there by every current browser.
pub async fn favicon_ico() -> Response {
    serve("icon-32.png")
}

/// `/apple-touch-icon.png`: where iOS looks when a page carries no link.
pub async fn apple_touch_icon() -> Response {
    serve("apple-touch-icon.png")
}

fn serve(file: &str) -> Response {
    match assets::lookup(file) {
        Some(served) => {
            let cache = if served.immutable {
                "public, max-age=31536000, immutable"
            } else {
                "public, max-age=300"
            };
            (
                [
                    (
                        header::CONTENT_TYPE,
                        HeaderValue::from_static(served.asset.content_type),
                    ),
                    (header::CACHE_CONTROL, HeaderValue::from_static(cache)),
                ],
                served.asset.body,
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
