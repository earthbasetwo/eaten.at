//! Presentation layer for eaten.at.
//!
//! Typed HTML components and the CSS layer. This crate has no app logic,
//! no atproto types, and no routing.

pub mod assets;
pub mod components;
pub mod dates;
pub mod layout;
pub mod markdown;
pub mod meta;
pub mod theme;

/// Human-readable application name used in page chrome.
pub const APP_NAME: &str = "eaten.at";

/// Separator between segments of a page title.
const TITLE_SEPARATOR: &str = " — ";

/// Join non-empty title segments, most specific first, into a `<title>` value.
///
/// Empty or whitespace-only segments are skipped so callers can pass optional
/// parts without checking them first.
pub fn page_title<'a, I>(segments: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    segments
        .into_iter()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(TITLE_SEPARATOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_segments_in_order() {
        assert_eq!(
            page_title(["Sample Subject", "Sample Publication", APP_NAME]),
            "Sample Subject — Sample Publication — eaten.at"
        );
    }

    #[test]
    fn skips_blank_segments() {
        assert_eq!(page_title(["", "  ", "Only"]), "Only");
    }

    #[test]
    fn empty_input_is_empty_title() {
        assert_eq!(page_title(std::iter::empty::<&str>()), "");
    }
}
