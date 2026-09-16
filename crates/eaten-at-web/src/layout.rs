//! The page shell: document skeleton, landmarks, and head metadata.
//!
//! Styling arrives with the CSS layer; this module only fixes the
//! structure every page shares so accessibility is built in from the
//! first route rather than retrofitted.

use maud::{html, Markup, PreEscaped, DOCTYPE};

use crate::assets::css_path;
use crate::theme::Theme;
use crate::{page_title, APP_NAME};

/// Everything the shell needs to know about a page.
#[derive(Debug, Clone, Default)]
pub struct Page<'a> {
    /// Title segments, most specific first. The app name is appended.
    pub title: &'a [&'a str],
    /// Content of `<main>`.
    pub main: Markup,
    /// Extra `<head>` markup (meta tags, canonical links).
    pub head: Markup,
    /// What the page names at the top. Nothing, by default.
    pub masthead: Masthead<'a>,
    /// Publication theme, already clamped, applied to the whole page.
    pub theme: Option<Theme>,
    /// CSP nonce for the inline theme style block. Without one the theme
    /// is still emitted; a strict CSP will then drop it, which is the
    /// safe failure.
    pub nonce: Option<String>,
    /// Width of the main column.
    pub width: Width,
    /// Inline scripts (the islands), placed at the end of `<body>` and
    /// carrying the CSP nonce.
    pub scripts: Vec<&'static str>,
}

/// What, if anything, a page names above its content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Masthead<'a> {
    /// No masthead: the page opens with its own content. Every page but
    /// the signed-out landing page and a document works this way.
    #[default]
    None,
    /// The app's logotype alone, centred at the top of the page where a
    /// visitor first arrives. Not a masthead bar: no tagline, no rule.
    Logotype,
    /// A publication's name as a running head over a document, leading
    /// back to the publication's front page.
    RunningHead { name: &'a str, href: &'a str },
}

/// How wide `<main>`'s content column is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Width {
    /// Reading column, for documents.
    #[default]
    Measure,
    /// Wider, for listings and choosers.
    Wide,
}

/// Render a complete HTML document.
pub fn render(page: &Page<'_>) -> Markup {
    let title = page_title(page.title.iter().copied().chain([APP_NAME]));
    let column = match page.width {
        Width::Measure => "center",
        Width::Wide => "center-wide",
    };
    html! {
        (DOCTYPE)
        html lang="en" data-theme=[page.theme.as_ref().map(|_| "publication")] {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { (title) }
                link rel="stylesheet" href=(css_path());
                @if let Some(theme) = &page.theme {
                    style nonce=[page.nonce.as_deref()] { (PreEscaped(theme.css_rule())) }
                }
                (page.head)
            }
            body {
                a.skip-link href="#main" { "Skip to content" }
                @match page.masthead {
                    Masthead::None => {}
                    Masthead::Logotype => header.wordmark {
                        a.site-name.logotype href="/" { (APP_NAME) }
                    },
                    Masthead::RunningHead { name, href } => header.site-header {
                        a.site-name.running-head href=(href) { (name) }
                    },
                }
                main #main { div class=(column) { (page.main) } }
                footer.site-footer {
                    p {
                        a href="/" { (APP_NAME) }
                        " renders write-ups published on the AT Protocol. "
                        "Every document belongs to its author."
                    }
                }
                @for script in &page.scripts {
                    script nonce=[page.nonce.as_deref()] { (PreEscaped(script)) }
                }
            }
        }
    }
}

/// A form-only pagination control: a "newer" link back to the first
/// page and an "older" link carrying the next cursor.
pub fn pagination(base_path: &str, next_cursor: Option<&str>, is_first_page: bool) -> Markup {
    html! {
        @if !is_first_page || next_cursor.is_some() {
            nav.pagination aria-label="Pagination" {
                @if !is_first_page {
                    a href=(base_path) rel="first" { "← Newest" }
                }
                @if let Some(cursor) = next_cursor {
                    a href=(format!("{base_path}?cursor={}", urlencoding(cursor))) rel="next" { "Older →" }
                }
            }
        }
    }
}

/// Minimal percent-encoding for a query value: everything outside the
/// unreserved set is escaped.
pub fn urlencoding(s: &str) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            write!(out, "%{b:02X}").expect("writing to a String cannot fail");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_has_landmarks_and_title() {
        let out = render(&Page {
            title: &["Doc", "Pub"],
            main: html! { p { "hi" } },
            ..Page::default()
        })
        .into_string();
        assert!(out.starts_with("<!DOCTYPE html>"));
        assert!(out.contains("<title>Doc — Pub — eaten.at</title>"));
        for landmark in ["<main id=\"main\">", "<footer", "href=\"#main\""] {
            assert!(out.contains(landmark), "{landmark}");
        }
        assert!(out.contains(&format!("href=\"{}\"", css_path())), "{out}");
        // Nothing above the content by default; the app is named once,
        // in the footer.
        assert!(!out.contains("<header"), "{out}");
        assert_eq!(out.matches("eaten.at").count(), 2, "{out}");
        assert!(!out.contains("data-theme"), "{out}");
        assert!(!out.contains("<style"), "{out}");
    }

    #[test]
    fn the_signed_out_landing_page_opens_with_the_logotype_alone() {
        let out = render(&Page {
            title: &[],
            main: html! {},
            masthead: Masthead::Logotype,
            ..Page::default()
        })
        .into_string();
        assert!(
            out.contains(
                "<header class=\"wordmark\"><a class=\"site-name logotype\" href=\"/\">eaten.at</a></header>"
            ),
            "{out}"
        );
        // The logotype stands on its own: no tagline, no masthead bar.
        assert!(!out.contains("site-header"), "{out}");
        assert!(!out.contains("tagline"), "{out}");
    }

    #[test]
    fn a_document_names_its_publication_as_a_running_head() {
        let out = render(&Page {
            title: &["Doc"],
            main: html! {},
            masthead: Masthead::RunningHead {
                name: "Heavy <Rotation>",
                href: "/at/did:plc:x/pub1/",
            },
            ..Page::default()
        })
        .into_string();
        assert!(
            out.contains(
                "<header class=\"site-header\"><a class=\"site-name running-head\" href=\"/at/did:plc:x/pub1/\">Heavy &lt;Rotation&gt;</a></header>"
            ),
            "{out}"
        );
        // The logotype is the landing page's alone; the app is still
        // named once, in the footer.
        assert!(!out.contains("logotype"), "{out}");
        assert_eq!(out.matches("eaten.at").count(), 2, "{out}");
    }

    #[test]
    fn theme_is_emitted_as_a_nonced_style_block() {
        use crate::theme::Rgb;
        let out = render(&Page {
            title: &["T"],
            main: html! {},
            theme: Some(Theme {
                background: Rgb::WHITE,
                foreground: Rgb::BLACK,
                accent: Rgb::new(0, 100, 160),
                accent_foreground: Rgb::WHITE,
            }),
            nonce: Some("abc123".into()),
            ..Page::default()
        })
        .into_string();
        assert!(
            out.contains("<html lang=\"en\" data-theme=\"publication\">"),
            "{out}"
        );
        assert!(
            out.contains("<style nonce=\"abc123\">:root[data-theme]{--theme-bg: 255 255 255;"),
            "{out}"
        );
    }

    #[test]
    fn pagination_links() {
        assert_eq!(pagination("/p/", None, true).into_string(), "");
        let mid = pagination("/p/", Some("3abc def"), false).into_string();
        assert!(mid.contains("href=\"/p/\" rel=\"first\""), "{mid}");
        assert!(
            mid.contains("href=\"/p/?cursor=3abc%20def\" rel=\"next\""),
            "{mid}"
        );
    }
}
