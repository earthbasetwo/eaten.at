//! Response security headers and the per-request CSP nonce (plan §8).
//!
//! Every response gets a strict Content-Security-Policy unless the handler
//! set its own (the image proxy does). Inline code is limited to the two
//! islands and the theme block, each carrying the request's nonce.

use axum::extract::{FromRequestParts, Request};
use axum::http::request::Parts;
use axum::http::{header, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;

/// A request's CSP nonce, 128 bits of randomness as hex.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nonce(pub String);

impl Nonce {
    fn generate() -> Self {
        use std::fmt::Write as _;
        let bytes: [u8; 16] = rand::random();
        Self(bytes.iter().fold(String::with_capacity(32), |mut out, b| {
            let _ = write!(out, "{b:02x}");
            out
        }))
    }

    /// The policy for a page carrying this nonce.
    pub fn csp(&self) -> String {
        format!(
            "default-src 'self'; \
             script-src 'nonce-{n}'; \
             style-src 'self' 'nonce-{n}'; \
             img-src 'self' data:; \
             frame-src 'none'; \
             frame-ancestors 'none'; \
             base-uri 'none'; \
             form-action 'self'; \
             object-src 'none'",
            n = self.0
        )
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Nonce {
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(parts: &mut Parts, _: &S) -> Result<Self, Self::Rejection> {
        // The middleware always inserts one; a missing nonce means the
        // router was built without it, and a fresh nonce is still correct
        // (it just won't match any header).
        Ok(parts
            .extensions
            .get::<Nonce>()
            .cloned()
            .unwrap_or_else(Nonce::generate))
    }
}

/// Middleware: attach a nonce to the request and security headers to the
/// response. Headers a handler already set are left alone.
pub async fn headers(mut request: Request, next: Next) -> Response {
    let nonce = Nonce::generate();
    request.extensions_mut().insert(nonce.clone());
    let mut response = next.run(request).await;
    let headers = response.headers_mut();
    if !headers.contains_key(header::CONTENT_SECURITY_POLICY) {
        if let Ok(value) = HeaderValue::from_str(&nonce.csp()) {
            headers.insert(header::CONTENT_SECURITY_POLICY, value);
        }
    }
    let defaults: [(header::HeaderName, &'static str); 4] = [
        (header::REFERRER_POLICY, "strict-origin-when-cross-origin"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        (
            header::HeaderName::from_static("permissions-policy"),
            "camera=(), microphone=(), geolocation=(self), payment=()",
        ),
        (header::X_FRAME_OPTIONS, "DENY"),
    ];
    for (name, value) in defaults {
        headers
            .entry(name)
            .or_insert_with(|| HeaderValue::from_static(value));
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nonce_is_32_hex_chars_and_unique() {
        let a = Nonce::generate();
        let b = Nonce::generate();
        assert_eq!(a.0.len(), 32);
        assert!(a.0.bytes().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn csp_has_no_unsafe_inline_and_names_the_nonce() {
        let nonce = Nonce("abc".into());
        let csp = nonce.csp();
        assert!(!csp.contains("unsafe-inline") && !csp.contains("unsafe-eval"));
        assert!(csp.contains("script-src 'nonce-abc'"));
        assert!(csp.contains("style-src 'self' 'nonce-abc'"));
        assert!(csp.contains("frame-src 'none'"));
        assert!(csp.contains("frame-ancestors 'none'"));
    }
}
