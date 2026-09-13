//! `<head>` metadata: title, description, canonical, OpenGraph, Twitter
//! card, and feed discovery (plan §7.7).

use maud::{html, Markup};

/// What kind of thing the page is, for `og:type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Article,
    Website,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Article => "article",
            Self::Website => "website",
        }
    }
}

/// Everything the head needs. All URLs must be absolute.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageMeta {
    /// `og:title`; the `<title>` is assembled separately by the shell.
    pub title: String,
    pub description: String,
    /// The canonical address of what the page shows. Always the
    /// publication origin, even when rendered on a site route.
    pub canonical: String,
    pub kind: Kind,
    pub site_name: String,
    /// Absolute URL of a JPEG in the 1200×630 range.
    pub image: String,
    pub published: Option<String>,
    pub modified: Option<String>,
    /// Absolute URL of the publication's RSS feed.
    pub feed: Option<String>,
}

/// Render the tags. `description` is omitted when empty rather than
/// emitted blank, since unfurlers treat an empty description as "none".
pub fn head(meta: &PageMeta) -> Markup {
    let description = meta.description.trim();
    html! {
        @if !description.is_empty() {
            meta name="description" content=(description);
        }
        link rel="canonical" href=(meta.canonical);
        meta property="og:url" content=(meta.canonical);
        meta property="og:type" content=(meta.kind.as_str());
        meta property="og:title" content=(meta.title);
        @if !description.is_empty() {
            meta property="og:description" content=(description);
        }
        meta property="og:site_name" content=(meta.site_name);
        meta property="og:image" content=(meta.image);
        meta property="og:image:width" content="1200";
        meta property="og:image:height" content="630";
        @if let Some(published) = &meta.published {
            meta property="article:published_time" content=(published);
        }
        @if let Some(modified) = &meta.modified {
            meta property="article:modified_time" content=(modified);
        }
        meta name="twitter:card" content="summary_large_image";
        @if let Some(feed) = &meta.feed {
            link rel="alternate" type="application/rss+xml" title=(format!("{} feed", meta.site_name)) href=(feed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_full_set_and_omits_empties() {
        let out = head(&PageMeta {
            title: "Second Subject".into(),
            description: String::new(),
            canonical: "https://ross.eaten.at/2026/09/ok".into(),
            kind: Kind::Article,
            site_name: "Ross Writes".into(),
            image: "https://eaten.at/img/did:plc:x/abc?size=og".into(),
            published: Some("2026-09-07T12:00:00.000Z".into()),
            modified: None,
            feed: Some("https://eaten.at/at/did:plc:x/p/feed.xml".into()),
        })
        .into_string();
        assert!(!out.contains("name=\"description\""), "{out}");
        assert!(!out.contains("og:description"), "{out}");
        assert!(!out.contains("modified_time"), "{out}");
        assert!(
            out.contains("<link rel=\"canonical\" href=\"https://ross.eaten.at/2026/09/ok\">"),
            "{out}"
        );
        assert!(
            out.contains("<meta property=\"og:url\" content=\"https://ross.eaten.at/2026/09/ok\">"),
            "{out}"
        );
        assert!(out.contains("article:published_time"), "{out}");
        assert!(out.contains("type=\"application/rss+xml\""), "{out}");
    }
}
