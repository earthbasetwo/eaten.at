//! The session cookie (plan §5.4): `HttpOnly`, `SameSite=Lax`, `Secure`
//! on https, and scoped to the site's domain so publication subdomains
//! can read it later. On a loopback development origin there is no
//! domain to scope to and no TLS, so those two attributes are dropped.

use axum::http::header::COOKIE;
use axum::http::{HeaderMap, HeaderValue};
use url::Url;

use super::SESSION_LIFETIME;

/// Name of the session cookie.
pub const COOKIE_NAME: &str = "ea_session";

/// Builds and reads the session cookie for one origin.
#[derive(Debug, Clone)]
pub struct SessionCookie {
    domain: Option<String>,
    secure: bool,
}

impl SessionCookie {
    /// Attributes derived from the site's public URL.
    pub fn for_origin(public_url: &Url) -> Self {
        let domain = match public_url.host() {
            Some(url::Host::Domain(name)) if name != "localhost" => Some(name.to_owned()),
            _ => None,
        };
        Self {
            domain,
            secure: public_url.scheme() == "https",
        }
    }

    /// A `Set-Cookie` value establishing `token` for [`SESSION_LIFETIME`].
    pub fn set(&self, token: &str) -> HeaderValue {
        self.build(token, SESSION_LIFETIME.as_secs())
    }

    /// A `Set-Cookie` value that removes the cookie.
    pub fn clear(&self) -> HeaderValue {
        self.build("", 0)
    }

    fn build(&self, value: &str, max_age: u64) -> HeaderValue {
        let mut cookie =
            format!("{COOKIE_NAME}={value}; Path=/; Max-Age={max_age}; HttpOnly; SameSite=Lax");
        if let Some(domain) = &self.domain {
            cookie.push_str("; Domain=");
            cookie.push_str(domain);
        }
        if self.secure {
            cookie.push_str("; Secure");
        }
        // The token is hex and the domain a hostname: always a valid header.
        HeaderValue::from_str(&cookie).unwrap_or_else(|_| HeaderValue::from_static(""))
    }

    /// The session token from a request's `Cookie` header, if present.
    pub fn read(headers: &HeaderMap) -> Option<String> {
        headers
            .get_all(COOKIE)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .flat_map(|line| line.split(';'))
            .map(str::trim)
            .find_map(|pair| {
                pair.strip_prefix(COOKIE_NAME)
                    .and_then(|rest| rest.strip_prefix('='))
            })
            .filter(|token| !token.is_empty())
            .map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_cookie_is_scoped_and_secure() {
        let cookie = SessionCookie::for_origin(&Url::parse("https://eaten.at").unwrap());
        assert_eq!(
            cookie.set("abc").to_str().unwrap(),
            "ea_session=abc; Path=/; Max-Age=2592000; HttpOnly; SameSite=Lax; Domain=eaten.at; Secure"
        );
        assert_eq!(
            cookie.clear().to_str().unwrap(),
            "ea_session=; Path=/; Max-Age=0; HttpOnly; SameSite=Lax; Domain=eaten.at; Secure"
        );
    }

    #[test]
    fn loopback_cookie_has_no_domain_or_secure() {
        let cookie = SessionCookie::for_origin(&Url::parse("http://127.0.0.1:3000/").unwrap());
        assert_eq!(
            cookie.set("abc").to_str().unwrap(),
            "ea_session=abc; Path=/; Max-Age=2592000; HttpOnly; SameSite=Lax"
        );
        let local = SessionCookie::for_origin(&Url::parse("http://localhost:3000").unwrap());
        assert!(!local.set("x").to_str().unwrap().contains("Domain"));
    }

    #[test]
    fn reads_the_token_among_other_cookies() {
        let mut headers = HeaderMap::new();
        headers.append(
            COOKIE,
            HeaderValue::from_static("a=b; ea_session=tok123; c=d"),
        );
        assert_eq!(SessionCookie::read(&headers).as_deref(), Some("tok123"));
        let mut empty = HeaderMap::new();
        empty.append(COOKIE, HeaderValue::from_static("ea_session=; other=1"));
        assert_eq!(SessionCookie::read(&empty), None);
        let mut prefixed = HeaderMap::new();
        prefixed.append(COOKIE, HeaderValue::from_static("ea_session_old=1"));
        assert_eq!(SessionCookie::read(&prefixed), None);
        assert_eq!(SessionCookie::read(&HeaderMap::new()), None);
    }
}
