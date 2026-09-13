//! Hostname validation shared by handles and `did:web`.
//!
//! Rules follow the atproto handle specification: ASCII DNS labels, at
//! least two of them, a TLD that starts with a letter, and a short list of
//! top-level domains that must never resolve to an identity.

/// Maximum total length of a hostname in bytes.
const MAX_HOSTNAME_LEN: usize = 253;
/// Maximum length of one DNS label in bytes.
const MAX_LABEL_LEN: usize = 63;

/// Top-level domains the atproto spec forbids for handles. `.test` is
/// deliberately absent: it is allowed so test suites can use it.
const FORBIDDEN_TLDS: &[&str] = &[
    "alt",
    "arpa",
    "example",
    "internal",
    "invalid",
    "local",
    "localhost",
    "onion",
];

/// Why a string is not a valid atproto hostname.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum HostnameError {
    #[error("hostname is empty")]
    Empty,
    #[error("hostname is longer than {MAX_HOSTNAME_LEN} bytes")]
    TooLong,
    #[error("hostname must contain at least two labels")]
    TooFewLabels,
    #[error("label {0:?} is empty, too long, or has a leading or trailing hyphen")]
    BadLabel(String),
    #[error("hostname contains a character outside a-z, 0-9, `-`, and `.`")]
    BadCharacter,
    #[error("top-level domain must start with a letter")]
    NumericTld,
    #[error("top-level domain `.{0}` is not allowed for atproto identities")]
    ForbiddenTld(String),
}

/// Validate `input` as a lowercase ASCII hostname. The caller is expected to
/// lowercase first; uppercase is rejected here so validation is strict.
pub fn validate_hostname(input: &str) -> Result<(), HostnameError> {
    if input.is_empty() {
        return Err(HostnameError::Empty);
    }
    if input.len() > MAX_HOSTNAME_LEN {
        return Err(HostnameError::TooLong);
    }
    if !input
        .bytes()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'.')
    {
        return Err(HostnameError::BadCharacter);
    }
    let labels: Vec<&str> = input.split('.').collect();
    if labels.len() < 2 {
        return Err(HostnameError::TooFewLabels);
    }
    for label in &labels {
        let ok = !label.is_empty()
            && label.len() <= MAX_LABEL_LEN
            && !label.starts_with('-')
            && !label.ends_with('-');
        if !ok {
            return Err(HostnameError::BadLabel((*label).to_owned()));
        }
    }
    let tld = labels[labels.len() - 1];
    if !tld.as_bytes()[0].is_ascii_alphabetic() {
        return Err(HostnameError::NumericTld);
    }
    if FORBIDDEN_TLDS.contains(&tld) {
        return Err(HostnameError::ForbiddenTld(tld.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_hostnames() {
        for h in [
            "example.com",
            "alice.bsky.social",
            "a.b.c.d.test",
            "xn--bcher-kva.example.org",
        ] {
            assert_eq!(validate_hostname(h), Ok(()), "{h}");
        }
    }

    #[test]
    fn rejects_bad_hostnames() {
        let cases = [
            ("", HostnameError::Empty),
            ("localhost", HostnameError::TooFewLabels),
            ("Example.com", HostnameError::BadCharacter),
            ("exa_mple.com", HostnameError::BadCharacter),
            ("-bad.example.com", HostnameError::BadLabel("-bad".into())),
            ("bad-.example.com", HostnameError::BadLabel("bad-".into())),
            ("a..com", HostnameError::BadLabel(String::new())),
            ("example.123", HostnameError::NumericTld),
            ("foo.local", HostnameError::ForbiddenTld("local".into())),
            ("foo.arpa", HostnameError::ForbiddenTld("arpa".into())),
        ];
        for (input, expected) in cases {
            assert_eq!(validate_hostname(input), Err(expected), "input {input:?}");
        }
        let long_label = format!("{}.com", "a".repeat(MAX_LABEL_LEN + 1));
        assert!(matches!(
            validate_hostname(&long_label),
            Err(HostnameError::BadLabel(_))
        ));
        let long = format!("{}.com", ["a"; 130].join("."));
        assert_eq!(validate_hostname(&long), Err(HostnameError::TooLong));
    }
}
