//! `/static/{file}` — embedded assets: the stylesheet and the fonts.

use axum::extract::Path;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use eaten_at_web::assets;

pub async fn static_file(Path(file): Path<String>) -> Response {
    match assets::lookup(&file) {
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
