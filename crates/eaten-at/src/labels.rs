//! Self-applied content labels (plan §8): labeled documents and
//! publications render behind an interstitial until the reader chooses
//! to continue. The choice is a session cookie; nothing is stored.

use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue};
use eaten_at_atproto::lexicon::SelfLabels;

/// Cookie set once the reader has chosen to see labeled content.
pub const ACK_COOKIE: &str = "ea_show_labeled";

/// Label values from a record, trimmed and non-empty, in record order.
pub fn values(labels: Option<&SelfLabels>) -> Vec<String> {
    labels
        .map(|l| {
            l.values
                .iter()
                .map(|v| v.val.trim().to_owned())
                .filter(|v| !v.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Whether the request carries the acknowledgement cookie.
pub fn acknowledged(headers: &HeaderMap) -> bool {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|line| line.split(';'))
        .any(|pair| {
            let mut parts = pair.trim().splitn(2, '=');
            parts.next() == Some(ACK_COOKIE) && parts.next().map(str::trim) == Some("1")
        })
}

/// The `Set-Cookie` header that records the acknowledgement for the
/// session. `Secure` is added when the request arrived over HTTPS, which
/// a reverse proxy signals with `X-Forwarded-Proto`.
pub fn ack_cookie(headers: &HeaderMap) -> HeaderValue {
    let https = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("https"));
    if https {
        HeaderValue::from_static("ea_show_labeled=1; Path=/; HttpOnly; SameSite=Lax; Secure")
    } else {
        HeaderValue::from_static("ea_show_labeled=1; Path=/; HttpOnly; SameSite=Lax")
    }
}

/// Header name to set the cookie on.
pub const SET_COOKIE_HEADER: axum::http::HeaderName = SET_COOKIE;

/// A human phrase for a label value. Known values from the Bluesky
/// vocabulary get words; anything else is shown as written.
pub fn describe(value: &str) -> String {
    match value {
        "porn" | "sexual" | "nudity" => "adult content".to_owned(),
        "graphic-media" | "gore" => "graphic media".to_owned(),
        "!warn" => "a content warning".to_owned(),
        "!no-unauthenticated" => "content the author restricted".to_owned(),
        other => other.replace(['-', '_'], " "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_cookie_among_others() {
        let mut headers = HeaderMap::new();
        headers.insert(
            COOKIE,
            HeaderValue::from_static("a=b; ea_show_labeled=1; c=d"),
        );
        assert!(acknowledged(&headers));
        headers.insert(COOKIE, HeaderValue::from_static("ea_show_labeled=0"));
        assert!(!acknowledged(&headers));
        headers.remove(COOKIE);
        assert!(!acknowledged(&headers));
    }

    #[test]
    fn cookie_is_secure_only_behind_https() {
        let plain = HeaderMap::new();
        assert_eq!(
            ack_cookie(&plain).to_str().unwrap(),
            "ea_show_labeled=1; Path=/; HttpOnly; SameSite=Lax"
        );
        let mut https = HeaderMap::new();
        https.insert("x-forwarded-proto", HeaderValue::from_static("https"));
        assert!(ack_cookie(&https).to_str().unwrap().ends_with("; Secure"));
    }

    #[test]
    fn label_values_and_descriptions() {
        let labels: SelfLabels = serde_json::from_value(serde_json::json!({
            "$type": "com.atproto.label.defs#selfLabels",
            "values": [{"val": "porn"}, {"val": "  "}, {"val": "spoilers-ahead"}]
        }))
        .unwrap();
        assert_eq!(values(Some(&labels)), vec!["porn", "spoilers-ahead"]);
        assert!(values(None).is_empty());
        assert_eq!(describe("porn"), "adult content");
        assert_eq!(describe("spoilers-ahead"), "spoilers ahead");
    }
}
