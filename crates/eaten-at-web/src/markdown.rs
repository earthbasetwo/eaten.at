//! Markdown → safe HTML, plain text, and excerpts.
//!
//! Every document body comes from someone else's PDS, so the threat model
//! is hostile input. Two independent layers apply: raw HTML is dropped
//! from the event stream before rendering, and the rendered HTML is then
//! passed through an allowlist sanitizer. Either alone would do; both
//! together mean a bug in one is not a hole.

use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

use pulldown_cmark::{html, CowStr, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use unicode_segmentation::UnicodeSegmentation;

/// `rel` applied to every content link.
pub const LINK_REL: &str = "ugc nofollow noopener";
/// Target length of a derived excerpt, in graphemes.
pub const EXCERPT_TARGET: usize = 300;
/// Class on the hostname hint that follows a prose link.
const HOST_CLASS: &str = "host";

static SANITIZER: LazyLock<ammonia::Builder<'static>> = LazyLock::new(|| {
    let mut builder = ammonia::Builder::empty();
    builder
        .tags(HashSet::from([
            "p",
            "br",
            "h2",
            "h3",
            "h4",
            "h5",
            "h6",
            "strong",
            "em",
            "code",
            "pre",
            "blockquote",
            "ul",
            "ol",
            "li",
            "a",
            "hr",
            "small",
        ]))
        .generic_attributes(HashSet::new())
        .tag_attributes(HashMap::from([
            ("a", HashSet::from(["href"])),
            ("ol", HashSet::from(["start"])),
        ]))
        .allowed_classes(HashMap::from([("small", HashSet::from([HOST_CLASS]))]))
        .url_schemes(HashSet::from(["http", "https", "mailto"]))
        .url_relative(ammonia::UrlRelative::Deny)
        .link_rel(Some(LINK_REL))
        .strip_comments(true);
    builder
});

/// Render CommonMark to sanitized HTML.
///
/// - Raw HTML in the source is dropped, not shown.
/// - Headings are shifted down one level so content never competes with
///   the page's `<h1>`.
/// - Images become links to the image, labelled with their alt text.
/// - Links to a different host than their visible text get a hostname
///   hint, so the reader sees where a link really goes.
pub fn render(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len() * 2);
    html::push_html(
        &mut out,
        transform(Parser::new_ext(markdown, Options::empty())),
    );
    SANITIZER.clean(&out).to_string()
}

/// Rewrite the event stream per the rules in [`render`].
fn transform<'a>(events: impl Iterator<Item = Event<'a>>) -> impl Iterator<Item = Event<'a>> {
    let mut link: Option<LinkState> = None;
    let mut out = Vec::new();
    for event in events {
        match event {
            Event::Html(_) | Event::InlineHtml(_) => {}
            Event::Start(Tag::Heading { level, .. }) => out.push(Event::Start(Tag::Heading {
                level: demote(level),
                id: None,
                classes: Vec::new(),
                attrs: Vec::new(),
            })),
            Event::End(TagEnd::Heading(level)) => {
                out.push(Event::End(TagEnd::Heading(demote(level))));
            }
            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                out.push(Event::Start(Tag::Link {
                    link_type: pulldown_cmark::LinkType::Inline,
                    dest_url: dest_url.clone(),
                    title,
                    id: CowStr::Borrowed(""),
                }));
                link = Some(LinkState::new(&dest_url, true));
            }
            Event::End(TagEnd::Image | TagEnd::Link) => match link.take() {
                Some(state) => state.finish(&mut out),
                None => out.push(Event::End(TagEnd::Link)),
            },
            Event::Start(Tag::Link { ref dest_url, .. }) => {
                link = Some(LinkState::new(dest_url, false));
                out.push(event);
            }
            Event::Text(ref text) | Event::Code(ref text) => {
                if let Some(state) = link.as_mut() {
                    state.text.push_str(text);
                }
                out.push(event);
            }
            other => out.push(other),
        }
    }
    out.into_iter()
}

/// Bookkeeping for the link currently being emitted.
struct LinkState {
    host: Option<String>,
    text: String,
    is_image: bool,
}

impl LinkState {
    fn new(dest: &str, is_image: bool) -> Self {
        let host = url::Url::parse(dest)
            .ok()
            .filter(|u| matches!(u.scheme(), "http" | "https"))
            .and_then(|u| {
                u.host_str()
                    .map(|h| h.trim_start_matches("www.").to_owned())
            });
        Self {
            host,
            text: String::new(),
            is_image,
        }
    }

    /// Close the link, appending a hostname hint unless the visible text
    /// already names the host (bare URLs, or prose mentioning the domain).
    fn finish(self, out: &mut Vec<Event<'_>>) {
        let text = self.text.to_ascii_lowercase();
        if let (Some(host), true, true) = (&self.host, self.is_image, text.trim().is_empty()) {
            // An image with no alt text has no visible link text at all;
            // the hostname becomes the text rather than a hint.
            out.push(Event::Text(CowStr::from(host.clone())));
            out.push(Event::End(TagEnd::Link));
            return;
        }
        out.push(Event::End(TagEnd::Link));
        let Some(host) = self.host else { return };
        if text.contains(&host.to_ascii_lowercase()) {
            return;
        }
        let escaped = html_escape(&host);
        out.push(Event::Html(CowStr::from(format!(
            " <small class=\"{HOST_CLASS}\">{escaped}</small>"
        ))));
    }
}

fn demote(level: HeadingLevel) -> HeadingLevel {
    match level {
        HeadingLevel::H1 => HeadingLevel::H2,
        HeadingLevel::H2 => HeadingLevel::H3,
        HeadingLevel::H3 => HeadingLevel::H4,
        HeadingLevel::H4 => HeadingLevel::H5,
        HeadingLevel::H5 | HeadingLevel::H6 => HeadingLevel::H6,
    }
}

fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            c => out.push(c),
        }
    }
    out
}

/// Markdown → plain text, for `textContent` and for readers that render
/// neither markdown nor us. Blocks are separated by blank lines; inline
/// markup is dropped; raw HTML is dropped; image alt text is kept.
pub fn to_plaintext(markdown: &str) -> String {
    let mut out = String::with_capacity(markdown.len());
    for event in Parser::new_ext(markdown, Options::empty()) {
        match event {
            Event::Text(t) | Event::Code(t) => out.push_str(&t),
            Event::SoftBreak | Event::HardBreak => out.push('\n'),
            Event::End(
                TagEnd::Paragraph
                | TagEnd::Heading(_)
                | TagEnd::CodeBlock
                | TagEnd::BlockQuote(_)
                | TagEnd::List(_),
            )
            | Event::Rule => end_block(&mut out),
            Event::Start(Tag::Item) => out.push_str("- "),
            Event::End(TagEnd::Item) => end_line(&mut out),
            _ => {}
        }
    }
    collapse_blank_lines(out.trim())
}

/// A short summary derived from the body, for listings and meta tags when
/// the author wrote no `description`.
///
/// Takes the first paragraph (skipping any leading heading), collapses
/// whitespace, and truncates at a grapheme boundary, preferring a word
/// boundary when one falls late enough, with an ellipsis.
pub fn excerpt(markdown: &str, target: usize) -> String {
    let source = first_paragraph(markdown).unwrap_or_else(|| {
        to_plaintext(markdown)
            .lines()
            .find(|l| !l.trim().is_empty())
            .unwrap_or_default()
            .to_owned()
    });
    let collapsed = source.split_whitespace().collect::<Vec<_>>().join(" ");
    truncate_graphemes(&collapsed, target)
}

fn first_paragraph(markdown: &str) -> Option<String> {
    let mut depth = 0usize;
    let mut text = String::new();
    for event in Parser::new_ext(markdown, Options::empty()) {
        match event {
            Event::Start(Tag::Paragraph) => depth += 1,
            Event::End(TagEnd::Paragraph) if depth > 0 => {
                if !text.trim().is_empty() {
                    return Some(text);
                }
                depth -= 1;
            }
            Event::Text(t) | Event::Code(t) if depth > 0 => text.push_str(&t),
            Event::SoftBreak | Event::HardBreak if depth > 0 => text.push(' '),
            _ => {}
        }
    }
    None
}

/// Cut `s` to at most `target` graphemes. When cutting, back off to the
/// last space if it is past 60% of the target, then append `…`.
fn truncate_graphemes(s: &str, target: usize) -> String {
    let graphemes: Vec<&str> = s.graphemes(true).collect();
    if graphemes.len() <= target {
        return s.to_owned();
    }
    let keep = target.saturating_sub(1).max(1);
    let mut cut: String = graphemes[..keep].concat();
    if let Some(space) = cut.rfind(' ') {
        if space >= cut.len() * 3 / 5 {
            cut.truncate(space);
        }
    }
    let trimmed =
        cut.trim_end_matches(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | ':' | '-'));
    format!("{trimmed}…")
}

/// Terminate the current line without opening a blank one; used between
/// list items so a list reads as a list rather than a run of paragraphs.
fn end_line(out: &mut String) {
    let trimmed = out.trim_end_matches(' ').len();
    out.truncate(trimmed);
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
}

fn end_block(out: &mut String) {
    let trimmed = out.trim_end_matches(' ').len();
    out.truncate(trimmed);
    if !out.is_empty() && !out.ends_with("\n\n") {
        if out.ends_with('\n') {
            out.push('\n');
        } else {
            out.push_str("\n\n");
        }
    }
}

fn collapse_blank_lines(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut blank_run = 0;
    for line in s.lines() {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run == 1 {
                out.push('\n');
            }
        } else {
            blank_run = 0;
            out.push_str(line.trim_end());
            out.push('\n');
        }
    }
    out.trim_end().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_basic_markdown_with_demoted_headings() {
        let out = render("# Title\n\nSome *em* and **strong** and `code`.\n\n## Sub\n\n- a\n- b");
        assert_eq!(
            out,
            "<h2>Title</h2>\n<p>Some <em>em</em> and <strong>strong</strong> and <code>code</code>.</p>\n<h3>Sub</h3>\n<ul>\n<li>a</li>\n<li>b</li>\n</ul>\n"
        );
    }

    #[test]
    fn raw_html_is_dropped_not_escaped() {
        let cases = [
            "<script>alert(1)</script>",
            "before <b onmouseover=\"x()\">bold</b> after",
            "<iframe src=\"https://evil.example\"></iframe>",
            "<style>body{display:none}</style>",
            "<img src=x onerror=alert(1)>",
            "<a href=\"javascript:alert(1)\">x</a>",
            "<!-- comment -->",
            "<p style=\"color:red\">styled</p>",
        ];
        for input in cases {
            let out = render(input);
            assert!(
                !out.contains('<') || out.starts_with("<p>"),
                "{input:?} -> {out:?}"
            );
            for needle in [
                "script",
                "iframe",
                "style",
                "onerror",
                "onmouseover",
                "javascript:",
                "<img",
                "<!--",
            ] {
                assert!(
                    !out.contains(needle),
                    "{input:?} -> {out:?} contains {needle}"
                );
            }
        }
        assert_eq!(
            render("before <b onmouseover=\"x()\">bold</b> after"),
            "<p>before bold after</p>\n"
        );
    }

    #[test]
    fn html_inside_code_is_shown_as_text() {
        let out = render("```\n<script>alert(1)</script>\n```\n\nInline `<b>` too.");
        assert!(
            out.contains("<pre><code>&lt;script&gt;alert(1)&lt;/script&gt;\n</code></pre>"),
            "{out}"
        );
        assert!(out.contains("<code>&lt;b&gt;</code>"), "{out}");
    }

    #[test]
    fn links_get_rel_and_hostname_hint() {
        let out = render("Read [here](https://www.example.com/page) now.");
        assert_eq!(
            out,
            "<p>Read <a href=\"https://www.example.com/page\" rel=\"ugc nofollow noopener\">here</a> <small class=\"host\">example.com</small> now.</p>\n"
        );
    }

    #[test]
    fn hint_is_omitted_when_text_names_the_host() {
        let bare = render("<https://example.com/x>");
        assert!(!bare.contains("<small"), "{bare}");
        assert!(bare.contains("rel=\"ugc nofollow noopener\""), "{bare}");
        let prose = render("[Example.com's page](https://example.com/page)");
        assert!(!prose.contains("<small"), "{prose}");
        let mail = render("[mail me](mailto:a@b.example)");
        assert!(!mail.contains("<small"), "{mail}");
        assert!(mail.contains("href=\"mailto:a@b.example\""), "{mail}");
    }

    #[test]
    fn unsafe_and_relative_link_targets_are_removed() {
        let js = render("[x](javascript:alert(1))");
        assert!(!js.contains("javascript"), "{js}");
        let rel = render("[x](/local/path)");
        assert!(!rel.contains("href"), "{rel}");
        let data = render("[x](data:text/html,hi)");
        assert!(!data.contains("data:"), "{data}");
    }

    #[test]
    fn images_become_links() {
        let out = render("![Cover art](https://cdn.example.com/cover.jpg)");
        assert_eq!(
            out,
            "<p><a href=\"https://cdn.example.com/cover.jpg\" rel=\"ugc nofollow noopener\">Cover art</a> <small class=\"host\">cdn.example.com</small></p>\n"
        );
        let no_alt = render("![](https://cdn.example.com/cover.jpg)");
        assert_eq!(
            no_alt,
            "<p><a href=\"https://cdn.example.com/cover.jpg\" rel=\"ugc nofollow noopener\">cdn.example.com</a></p>\n"
        );
    }

    #[test]
    fn plaintext_strips_markup_and_separates_blocks() {
        let text = to_plaintext("# Title\n\nPara one with *em* and [a link](https://x.example).\nSoft break.\n\n- one\n- two\n\n```\ncode\n```\n\n<b>raw</b> html gone\n\n> quote");
        assert_eq!(
            text,
            "Title\n\nPara one with em and a link.\nSoft break.\n\n- one\n- two\n\ncode\n\nraw html gone\n\nquote"
        );
    }

    #[test]
    fn excerpt_takes_first_paragraph_after_heading() {
        let e = excerpt(
            "# Heading\n\nFirst   paragraph\nwith a soft break.\n\nSecond paragraph.",
            EXCERPT_TARGET,
        );
        assert_eq!(e, "First paragraph with a soft break.");
    }

    #[test]
    fn excerpt_truncates_on_grapheme_and_word_boundary() {
        let words = "word ".repeat(100);
        let e = excerpt(&words, 50);
        assert!(e.ends_with('…'), "{e}");
        assert!(e.graphemes(true).count() <= 50, "{e}");
        assert!(!e.contains("wor…"), "{e}");

        // A family emoji is one grapheme of several code points; a cut
        // must never land inside it.
        let family = "👨‍👩‍👧‍👦";
        let s = format!("{}{}", "x".repeat(28), family.repeat(5));
        let e = excerpt(&s, 30);
        assert_eq!(e, format!("{}{family}…", "x".repeat(28)));
        assert_eq!(e.graphemes(true).count(), 30, "{e}");
    }

    #[test]
    fn excerpt_edge_cases() {
        assert_eq!(excerpt("", EXCERPT_TARGET), "");
        assert_eq!(
            excerpt("# Only a heading", EXCERPT_TARGET),
            "Only a heading"
        );
        assert_eq!(excerpt("- just\n- a list", EXCERPT_TARGET), "- just");
        assert_eq!(excerpt("short", EXCERPT_TARGET), "short");
    }
}
