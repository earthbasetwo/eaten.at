//! Reusable page fragments. These take plain view structs, never protocol
//! types, so the presentation layer stays independent of atproto.

use maud::{html, Markup};

use crate::dates::human_date;

/// A link with a label.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Link {
    pub label: String,
    pub href: String,
}

/// Everything known about the subject a write-up is about.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SubjectCard {
    pub title: String,
    /// URL of the cover image to show, if any.
    pub cover_src: Option<String>,
    /// Links out, in the author's order. Rendered by
    /// [`subject_links`], not by the card.
    pub links: Vec<Link>,
}

/// The subject card shown above a write-up: cover beside the title.
pub fn subject_card(card: &SubjectCard) -> Markup {
    html! {
        aside.subject-card aria-label="Subject" {
            @if let Some(src) = &card.cover_src {
                img.subject-cover src=(src) alt="" width="120" height="120" loading="lazy";
            }
            div.subject-card-body {
                p.subject-title { (card.title) }
            }
        }
    }
}

/// Where else to read about the subject. Nothing is rendered for
/// an empty list.
pub fn subject_links(links: &[Link]) -> Markup {
    html! {
        @if !links.is_empty() {
            nav.subject-links aria-label="Links" {
                span.subject-links-label { "Links" }
                ul.subject-links-list {
                    @for link in links {
                        li { a href=(link.href) rel="ugc nofollow noopener" { (link.label) } }
                    }
                }
            }
        }
    }
}

/// One row of a publication's document list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingItem {
    pub href: String,
    pub title: String,
    pub subject_title: String,
    pub published: String,
    pub excerpt: String,
    pub cover_src: Option<String>,
}

/// A list of write-ups.
pub fn listing(items: &[ListingItem]) -> Markup {
    html! {
        ol.listing {
            @for item in items {
                li.listing-item {
                    article {
                        @if let Some(src) = &item.cover_src {
                            img.listing-cover src=(src) alt="" width="96" height="96" loading="lazy";
                        }
                        div.listing-body {
                            p.kicker { time datetime=(item.published) { (human_date(&item.published)) } }
                            h2.listing-title { a href=(item.href) { (item.title) } }
                            p.listing-subject {
                                span.listing-subject-title { (item.subject_title) }
                            }
                            @if !item.excerpt.is_empty() { p.listing-excerpt { (item.excerpt) } }
                        }
                    }
                }
            }
        }
    }
}

/// A publication in a chooser or header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationView {
    pub href: String,
    pub name: String,
    pub description: Option<String>,
    pub url: String,
}

/// Chooser shown when a repo has several publications and no preference:
/// one row per publication, in the order the repo lists them.
pub fn publication_chooser(publications: &[PublicationView]) -> Markup {
    html! {
        ol.chooser {
            @for publication in publications {
                li.chooser-item {
                    h2.chooser-name { a href=(publication.href) { (publication.name) } }
                    @if let Some(description) = &publication.description {
                        p.chooser-description { (description) }
                    }
                    p.meta.chooser-url { (publication.url.trim_start_matches("https://").trim_end_matches('/')) }
                }
            }
        }
    }
}

/// The handle lookup form on the landing page.
pub fn lookup_form(value: &str, error: Option<&str>) -> Markup {
    html! {
        form.lookup action="/lookup" method="get" {
            label.kicker.lookup-label for="handle" { "Read someone's write-ups by handle" }
            div.lookup-row {
                input #handle name="handle" type="text" inputmode="url" autocomplete="off"
                    placeholder="alice.bsky.social" value=(value)
                    aria-describedby=[error.map(|_| "handle-error")] required;
                button type="submit" { "Go" }
            }
            @if let Some(error) = error {
                p.form-error #handle-error role="alert" { (error) }
            }
        }
    }
}

/// Tag links. A tag page covers one publication, not everyone who used
/// the tag, so the landmark says so; `scope_note` adds visible copy where
/// a page wants it spelled out.
pub fn tag_links(tags: &[Link], scope_note: Option<&str>) -> Markup {
    html! {
        @if !tags.is_empty() {
            nav.tags aria-label="Tags in this publication" {
                ul.tag-list {
                    @for tag in tags {
                        li { a.tag href=(tag.href) { (tag.label) } }
                    }
                }
                @if let Some(note) = scope_note { p.muted.tags-note { (note) } }
            }
        }
    }
}

/// One reply in a document's comment thread on Bluesky.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommentView {
    /// The author's display name, when they set one.
    pub author_name: Option<String>,
    /// The author's handle, without the `@`.
    pub author_handle: String,
    /// The author's profile page.
    pub author_href: String,
    /// The reply's own page, for a permalink.
    pub href: String,
    /// When the reply was written, as an RFC 3339 timestamp.
    pub published: String,
    /// The reply as plain text. Rendered as written, never as markup.
    /// Empty for a reply that is only an image or a quote; the byline
    /// still shows so the thread's shape stays honest.
    pub text: String,
    /// Nesting under the root post: `0` for a direct reply. Indentation
    /// stops at [`MAX_COMMENT_INDENT`].
    pub depth: u8,
}

/// Deepest indentation drawn for a nested reply; deeper ones sit level
/// with it.
pub const MAX_COMMENT_INDENT: u8 = 3;

/// A document's comment thread, read from Bluesky.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CommentsView {
    /// The thread on Bluesky, where a reader can answer.
    pub thread_href: String,
    /// Replies in reading order, each after the one it answers.
    pub comments: Vec<CommentView>,
    /// Whether replies were left out for length.
    pub truncated: bool,
}

/// The comment thread under a write-up: a kicker, the replies as rows,
/// and a link to answer on Bluesky. An empty thread says so rather than
/// disappearing, so the invitation to comment is still there.
pub fn comments(view: &CommentsView) -> Markup {
    html! {
        section.comments aria-labelledby="comments-heading" {
            h2.kicker #comments-heading { "Comments on Bluesky" }
            @if view.comments.is_empty() {
                p.empty.comments-empty { "No comments yet." }
            } @else {
                ol.comment-list {
                    @for comment in &view.comments {
                        @let depth = comment.depth.min(MAX_COMMENT_INDENT);
                        li class={ "comment" @if depth > 0 { " depth-" (depth) } } {
                            article {
                                p.comment-byline {
                                    @if let Some(name) = comment.author_name.as_deref().map(str::trim).filter(|n| !n.is_empty()) {
                                        a.comment-author href=(comment.author_href) rel="ugc nofollow noopener" { (name) }
                                        span.comment-handle { "@" (comment.author_handle) }
                                    } @else {
                                        a.comment-author href=(comment.author_href) rel="ugc nofollow noopener" { "@" (comment.author_handle) }
                                    }
                                    a.comment-time href=(comment.href) rel="ugc nofollow noopener" {
                                        time datetime=(comment.published) { (human_date(&comment.published)) }
                                    }
                                }
                                @if !comment.text.is_empty() {
                                    p.comment-text { (comment.text) }
                                }
                            }
                        }
                    }
                }
            }
            p.meta.comments-more {
                a href=(view.thread_href) rel="noopener" {
                    @if view.truncated { "More on Bluesky →" } @else { "Reply on Bluesky →" }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn link(label: &str, href: &str) -> Link {
        Link {
            label: label.into(),
            href: href.into(),
        }
    }

    #[test]
    fn card_shows_title_and_cover() {
        let out = subject_card(&SubjectCard {
            title: "Sample Subject".into(),
            cover_src: Some("/img/did/rk".into()),
            links: vec![link("Elsewhere", "https://example.com/x")],
        })
        .into_string();
        assert!(
            out.contains("<p class=\"subject-title\">Sample Subject</p>"),
            "{out}"
        );
        assert!(
            out.contains("class=\"subject-cover\" src=\"/img/did/rk\" alt=\"\""),
            "{out}"
        );
        // Links are the footer's business, not the card's.
        assert!(!out.contains("example.com"), "{out}");
        let bare = subject_card(&SubjectCard {
            title: "Untitled".into(),
            ..SubjectCard::default()
        })
        .into_string();
        assert!(!bare.contains("<img"), "{bare}");
    }

    #[test]
    fn subject_links_are_labelled_and_marked_ugc() {
        assert_eq!(subject_links(&[]).into_string(), "");
        let out = subject_links(&[link("Elsewhere", "https://example.com/x")]).into_string();
        assert!(
            out.starts_with("<nav class=\"subject-links\" aria-label=\"Links\">"),
            "{out}"
        );
        assert!(
            out.contains(
                "<a href=\"https://example.com/x\" rel=\"ugc nofollow noopener\">Elsewhere</a>"
            ),
            "{out}"
        );
    }

    #[test]
    fn tags_name_their_scope() {
        assert_eq!(tag_links(&[], Some("note")).into_string(), "");
        let out = tag_links(&[link("mpb", "/t/mpb")], None).into_string();
        assert!(
            out.contains("aria-label=\"Tags in this publication\""),
            "{out}"
        );
        assert!(
            out.contains("<a class=\"tag\" href=\"/t/mpb\">mpb</a>"),
            "{out}"
        );
        assert!(!out.contains("tags-note"), "{out}");
        let noted = tag_links(&[link("mpb", "/t/mpb")], Some("Only here.")).into_string();
        assert!(
            noted.contains("<p class=\"muted tags-note\">Only here.</p>"),
            "{noted}"
        );
    }
    #[test]
    fn comments_render_rows_in_order_with_plain_text() {
        let out = comments(&CommentsView {
            thread_href: "https://bsky.app/profile/alice.test/post/3k".into(),
            comments: vec![
                CommentView {
                    author_name: Some("Bob".into()),
                    author_handle: "bob.test".into(),
                    author_href: "https://bsky.app/profile/bob.test".into(),
                    href: "https://bsky.app/profile/bob.test/post/3r1".into(),
                    published: "2026-09-08T10:00:00.000Z".into(),
                    text: "Loved *this* <b>record</b>\nand the write-up.".into(),
                    depth: 0,
                },
                CommentView {
                    author_name: None,
                    author_handle: "carol.test".into(),
                    author_href: "https://bsky.app/profile/carol.test".into(),
                    href: "https://bsky.app/profile/carol.test/post/3r2".into(),
                    published: "2026-09-08T11:00:00.000Z".into(),
                    text: String::new(),
                    depth: 5,
                },
            ],
            truncated: false,
        })
        .into_string();
        assert!(
            out.starts_with("<section class=\"comments\" aria-labelledby=\"comments-heading\"><h2 class=\"kicker\" id=\"comments-heading\">Comments on Bluesky</h2>"),
            "{out}"
        );
        assert!(
            out.contains("<li class=\"comment\"><article><p class=\"comment-byline\"><a class=\"comment-author\" href=\"https://bsky.app/profile/bob.test\" rel=\"ugc nofollow noopener\">Bob</a><span class=\"comment-handle\">@bob.test</span><a class=\"comment-time\" href=\"https://bsky.app/profile/bob.test/post/3r1\" rel=\"ugc nofollow noopener\"><time datetime=\"2026-09-08T10:00:00.000Z\">September 8, 2026</time></a></p>"),
            "{out}"
        );
        // Text is escaped, never parsed: the markdown stays literal and
        // the tag is entity-encoded.
        assert!(
            out.contains("<p class=\"comment-text\">Loved *this* &lt;b&gt;record&lt;/b&gt;\nand the write-up.</p>"),
            "{out}"
        );
        // Without a display name the handle is the author; nesting is
        // capped at the deepest drawn indent.
        assert!(
            out.contains("<li class=\"comment depth-3\"><article><p class=\"comment-byline\"><a class=\"comment-author\" href=\"https://bsky.app/profile/carol.test\" rel=\"ugc nofollow noopener\">@carol.test</a><a class=\"comment-time\""),
            "{out}"
        );
        assert!(!out.contains("comment-handle\">@carol"), "{out}");
        // An image-only reply has no text paragraph, only its byline.
        assert_eq!(out.matches("comment-text").count(), 1, "{out}");
        assert!(
            out.ends_with("<p class=\"meta comments-more\"><a href=\"https://bsky.app/profile/alice.test/post/3k\" rel=\"noopener\">Reply on Bluesky →</a></p></section>"),
            "{out}"
        );
    }

    #[test]
    fn empty_and_truncated_threads_say_so() {
        let empty = comments(&CommentsView {
            thread_href: "https://bsky.app/profile/alice.test/post/3k".into(),
            ..CommentsView::default()
        })
        .into_string();
        assert!(
            empty.contains("<p class=\"empty comments-empty\">No comments yet.</p>"),
            "{empty}"
        );
        assert!(!empty.contains("comment-list"), "{empty}");
        assert!(empty.contains(">Reply on Bluesky →<"), "{empty}");
        let truncated = comments(&CommentsView {
            thread_href: "https://bsky.app/profile/alice.test/post/3k".into(),
            truncated: true,
            ..CommentsView::default()
        })
        .into_string();
        assert!(truncated.contains(">More on Bluesky →<"), "{truncated}");
    }
}
