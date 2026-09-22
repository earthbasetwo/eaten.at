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

/// The author's verdict on the house scale, ready to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RatingView {
    /// The verdict in words, e.g. "Recommended". The stylesheet sets it
    /// in capitals; the text stays as written so it reads naturally.
    pub word: String,
}

/// Everything known about the visit a write-up describes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VisitCard {
    pub place_name: String,
    pub address: Option<String>,
    /// The price band as currency signs, e.g. "$$".
    pub price: Option<String>,
    /// The visit date, `YYYY-MM-DD`.
    pub visited_on: String,
    /// Which meal, in words.
    pub meal: Option<String>,
    pub rating: Option<RatingView>,
    /// Links out, in the author's order. Rendered by
    /// [`visit_links`], not by the card.
    pub links: Vec<Link>,
}

/// The visit shown above a write-up: the place's name, a metadata line
/// of date, meal, and price, the address, and the rating.
pub fn visit_card(card: &VisitCard) -> Markup {
    html! {
        aside.visit-card aria-label="Visit" {
            div.visit-card-body {
                p.place-name { (card.place_name) }
                p.visit-meta {
                    time datetime=(card.visited_on) { (human_date(&card.visited_on)) }
                    @if let Some(meal) = &card.meal { span.visit-meal { (meal) } }
                    @if let Some(price) = &card.price {
                        span.price aria-label=(format!("price band {} of 4", price.chars().count())) { (price) }
                    }
                }
                @if let Some(address) = &card.address {
                    p.place-address { (address) }
                }
                @if let Some(rating) = &card.rating {
                    (rating_line(rating))
                }
            }
        }
    }
}

/// The rating as its word alone. No marks, stars, or numbers: the
/// stylesheet sets the word in tracked mono capitals, in the accent.
pub fn rating_line(rating: &RatingView) -> Markup {
    html! {
        p.rating { (rating.word) }
    }
}

/// Where else to read about the place. Nothing is rendered for
/// an empty list.
pub fn visit_links(links: &[Link]) -> Markup {
    html! {
        @if !links.is_empty() {
            nav.visit-links aria-label="Links" {
                span.visit-links-label { "Links" }
                ul.visit-links-list {
                    @for link in links {
                        li { a href=(link.href) rel="ugc nofollow noopener" { (link.label) } }
                    }
                }
            }
        }
    }
}

/// One photo of a visit, ready to show.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoView {
    /// The square thumbnail.
    pub thumb_src: String,
    /// The full rendition the thumbnail links to.
    pub full_src: String,
    /// Alt text as the author wrote it; empty stays empty.
    pub alt: String,
}

/// Side of the square thumbnails the grid shows.
pub const THUMB_SIDE: u32 = 400;

/// The photos of a visit, a grid of square thumbnails each linking to
/// its full rendition. Nothing is rendered for none.
pub fn photo_grid(photos: &[PhotoView]) -> Markup {
    html! {
        @if !photos.is_empty() {
            section.photos aria-label="Photos" {
                ul.photo-grid {
                    @for photo in photos {
                        li {
                            a href=(photo.full_src) aria-label=[photo.alt.is_empty().then_some("View photo")] {
                                figure {
                                    // The visible caption names the link without repeating the image description.
                                    img src=(photo.thumb_src) alt=""
                                        width=(THUMB_SIDE) height=(THUMB_SIDE) loading="lazy";
                                    @if !photo.alt.is_empty() { figcaption { (photo.alt) } }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The photo that leads a listing card (plan 13): the visit's first,
/// as the card rendition (up to 960 wide, 3:2), and how many more
/// there are.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingPhoto {
    pub src: String,
    /// Alt text as the author wrote it; empty stays empty.
    pub alt: String,
    /// Photos beyond this one.
    pub more: usize,
}

/// The card rendition's proportions, so the card reserves its space
/// before the image loads.
pub const CARD_PHOTO_WIDTH: u32 = 960;
pub const CARD_PHOTO_HEIGHT: u32 = 640;

/// One row of a publication's document list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListingItem {
    pub href: String,
    pub title: String,
    /// The place, when it is not already the title.
    pub place_name: Option<String>,
    pub rating: Option<RatingView>,
    pub published: String,
    pub excerpt: String,
    /// The photo that leads the card, if the visit has any.
    pub photo: Option<ListingPhoto>,
}

/// A list of write-ups: rows, the title left and the verdict right, then
/// the date and the place, then the excerpt. A row is led by its first
/// photo when the visit has one and is text alone when it has none
/// (plan 13).
pub fn listing(items: &[ListingItem]) -> Markup {
    html! {
        ol.listing {
            @for item in items {
                li.listing-item.has-photos[item.photo.is_some()].no-photos[item.photo.is_none()] {
                    article {
                        @if let Some(photo) = &item.photo {
                            // The same target as the title; not a second tab stop.
                            a.listing-photo href=(item.href) tabindex="-1" {
                                img src=(photo.src) alt=(photo.alt)
                                    width=(CARD_PHOTO_WIDTH) height=(CARD_PHOTO_HEIGHT) loading="lazy";
                                @if photo.more > 0 {
                                    span.listing-photo-count {
                                        "+" (photo.more) " " @if photo.more == 1 { "photo" } @else { "photos" }
                                    }
                                }
                            }
                        }
                        div.listing-body {
                            div.listing-head {
                                h2.listing-title { a href=(item.href) { (item.title) } }
                                @if let Some(rating) = &item.rating {
                                    p.rating { (rating.word) }
                                }
                            }
                            p.listing-meta {
                                time datetime=(item.published) { (human_date(&item.published)) }
                                @if let Some(name) = &item.place_name {
                                    span.listing-place-name { (name) }
                                }
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

/// The handle lookup form, as the landing page and the lookup error
/// page show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LookupForm<'a> {
    /// The handle as typed, kept on an error.
    pub value: &'a str,
    pub error: Option<&'a str>,
    /// The label over the field.
    pub label: &'a str,
    /// The submit button's text.
    pub button: &'a str,
    /// Whether the button is the page's primary action. The landing
    /// page's is not (sign-in is); the lookup error page's is.
    pub primary: bool,
    /// The `AppView` origin the handle island suggests from (plan 10),
    /// carried on the input as `data-typeahead`. `None` means no
    /// suggestions, and the placeholder says so.
    pub typeahead: Option<&'a str>,
}

impl Default for LookupForm<'_> {
    fn default() -> Self {
        Self {
            value: "",
            error: None,
            label: "Read someone's write-ups by handle",
            button: "Go",
            primary: true,
            typeahead: None,
        }
    }
}

/// The handle lookup form.
pub fn lookup_form(form: &LookupForm<'_>) -> Markup {
    html! {
        form.lookup action="/lookup" method="get" {
            label.kicker.lookup-label for="handle" { (form.label) }
            div.lookup-row {
                input #handle name="handle" type="text" inputmode="url" autocomplete="off"
                    placeholder=(if form.typeahead.is_some() { "Start typing a handle…" } else { "alice.bsky.social" })
                    value=(form.value) data-typeahead=[form.typeahead]
                    aria-describedby=[form.error.map(|_| "handle-error")] required;
                button.button-secondary[!form.primary] type="submit" { (form.button) }
            }
            @if let Some(error) = form.error {
                p.form-error #handle-error role="alert" { (error) }
            }
        }
    }
}

/// Which way in a [`connect`] block opens. Both of the signed-out
/// landing page's actions are drawn this way; only the destination, the
/// voice of the field, and whether the action is the page's primary
/// differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectWay {
    /// Sign in to write: the reader's own handle, posted where the
    /// sign-in page posts.
    Write,
    /// Read someone else: their handle, sent to the lookup route as the
    /// plain GET that form has always been.
    Read,
}

impl ConnectWay {
    /// Where the button leads without script, and where the field is
    /// sent with it.
    const fn action(self) -> &'static str {
        match self {
            Self::Write => "/login",
            Self::Read => "/lookup",
        }
    }

    const fn method(self) -> &'static str {
        match self {
            Self::Write => "post",
            Self::Read => "get",
        }
    }

    /// The field's id. Both blocks share the page, so neither may take
    /// the bare `handle` the lookup page's own form uses.
    const fn field_id(self) -> &'static str {
        match self {
            Self::Write => "connect-handle",
            Self::Read => "lookup-handle",
        }
    }

    /// The field's label, read by assistive technology alone: the button
    /// above it has already said what this is for.
    const fn field_label(self) -> &'static str {
        match self {
            Self::Write => "Your handle",
            Self::Read => "Their handle",
        }
    }

    const fn placeholder(self) -> &'static str {
        match self {
            Self::Write => "Start typing your handle…",
            Self::Read => "Start typing their handle…",
        }
    }

    /// Only the reader's own handle is theirs to fill in.
    const fn autocomplete(self) -> &'static str {
        match self {
            Self::Write => "username",
            Self::Read => "off",
        }
    }

    /// The block's classes. Reading is drawn as the secondary way in,
    /// which the moving rule reads as well: it sets out from 1px of ink
    /// rather than the primary's 2px of vermilion.
    const fn block_class(self) -> &'static str {
        match self {
            Self::Write => "connect",
            Self::Read => "connect connect-read",
        }
    }

    /// The button's classes. At most one action on a page is the primary
    /// (see the actions rule in `app.css`); writing wins it.
    const fn button_class(self) -> &'static str {
        match self {
            Self::Write => "button connect-button",
            Self::Read => "button-secondary connect-button",
        }
    }
}

/// A way in on the signed-out landing page (plan 09): one button that,
/// pressed, becomes a handle field in place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Connect<'a> {
    /// The `AppView` origin the handle field suggests from (D42).
    pub appview: &'a str,
    /// Which way in this one opens.
    pub way: ConnectWay,
    /// A line above the button, in the lede's voice, for a block whose
    /// button cannot carry its whole reason. `None` leaves the button
    /// to speak for itself.
    pub intro: Option<&'a str>,
    /// What the action says: the verb and what pressing it is for, in
    /// the one phrase, so the line carries its own reason.
    pub label: &'a str,
}

/// The connect component.
///
/// Without script it is a link to the page the field would send to; the
/// form is in the markup but hidden. The island hides the link, shows
/// the form, and focuses the field, so the swap happens where the button
/// stood and the caret is already in the field. The form is sent where
/// that page's own form sends, and an error comes back on that page with
/// the handle kept.
///
/// The intro, when there is one, is a sibling of the block rather than
/// part of it: the island lifts the button's row out of the flow to the
/// block's top corner while the form takes its place, and a line inside
/// the block would be what the row landed on.
///
/// The field carries no button: it is sent with Return, which the faint
/// mark at the end of its rule says. That is the browser's own implicit
/// submission, so it holds with the island's script and without it.
pub fn connect(connect: &Connect<'_>) -> Markup {
    let way = connect.way;
    html! {
        @if let Some(intro) = connect.intro {
            p.lede.connect-intro { (intro) }
        }
        div class=(way.block_class()) data-connect {
            div.actions.landing-actions.connect-idle {
                a class=(way.button_class()) href=(way.action()) { (connect.label) }
            }
            form.lookup.connect-form action=(way.action()) method=(way.method()) hidden {
                label.visually-hidden for=(way.field_id()) { (way.field_label()) }
                div.lookup-row {
                    span.return-rule {
                        input id=(way.field_id()) name="handle" type="text" inputmode="url"
                            autocomplete=(way.autocomplete())
                            placeholder=(way.placeholder())
                            data-typeahead=(connect.appview) required;
                    }
                }
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
    fn card_shows_place_meta_and_rating() {
        let out = visit_card(&VisitCard {
            place_name: "Sample Place".into(),
            address: Some("1 Example St".into()),
            price: Some("$$".into()),
            visited_on: "2026-09-12".into(),
            meal: Some("Dinner".into()),
            rating: Some(RatingView {
                word: "Recommended".into(),
            }),
            links: vec![link("Elsewhere", "https://example.com/x")],
        })
        .into_string();
        assert!(
            out.contains("<p class=\"place-name\">Sample Place</p>"),
            "{out}"
        );
        assert!(
            out.contains("<p class=\"visit-meta\"><time datetime=\"2026-09-12\">September 12, 2026</time><span class=\"visit-meal\">Dinner</span><span class=\"price\" aria-label=\"price band 2 of 4\">$$</span></p>"),
            "{out}"
        );
        assert!(
            out.contains("<p class=\"place-address\">1 Example St</p>"),
            "{out}"
        );
        assert!(out.contains("<p class=\"rating\">Recommended</p>"), "{out}");
        // Links are the footer's business, not the card's.
        assert!(!out.contains("example.com"), "{out}");
        let bare = visit_card(&VisitCard {
            place_name: "Somewhere".into(),
            visited_on: "2026-09-12".into(),
            ..VisitCard::default()
        })
        .into_string();
        assert!(!bare.contains("<img"), "{bare}");
        assert!(!bare.contains("rating"), "{bare}");
        assert!(!bare.contains("place-address"), "{bare}");
    }

    #[test]
    fn listing_rows_carry_place_and_rating() {
        let out = listing(&[ListingItem {
            href: "/d".into(),
            title: "A night out".into(),
            place_name: Some("Sample Place".into()),
            rating: Some(RatingView {
                word: "Can’t Miss".into(),
            }),
            published: "2026-09-07T12:00:00.000Z".into(),
            excerpt: String::new(),
            photo: Some(ListingPhoto {
                src: "/img/did/rk/bafy?size=card".into(),
                alt: "The <room>".into(),
                more: 2,
            }),
        }])
        .into_string();
        assert!(
            out.contains("<li class=\"listing-item has-photos\">"),
            "{out}"
        );
        assert!(
            out.contains(
                "<a class=\"listing-photo\" href=\"/d\" tabindex=\"-1\"><img src=\"/img/did/rk/bafy?size=card\" alt=\"The &lt;room&gt;\" width=\"960\" height=\"640\" loading=\"lazy\"><span class=\"listing-photo-count\">+2 photos</span></a>"
            ),
            "{out}"
        );
        // The verdict sits beside the title; the date and the place make
        // the metadata line.
        assert!(
            out.contains("<div class=\"listing-head\"><h2 class=\"listing-title\"><a href=\"/d\">A night out</a></h2><p class=\"rating\">Can’t Miss</p></div><p class=\"listing-meta\"><time datetime=\"2026-09-07T12:00:00.000Z\">September 7, 2026</time><span class=\"listing-place-name\">Sample Place</span></p>"),
            "{out}"
        );
        assert!(!out.contains("listing-excerpt"), "{out}");
        // The title is the place's name: the metadata line is the date
        // alone, and without a rating the head is the title alone.
        let item = ListingItem {
            href: "/d".into(),
            title: "Sample Place".into(),
            place_name: None,
            rating: Some(RatingView {
                word: "Solid".into(),
            }),
            published: "2026-09-07T12:00:00.000Z".into(),
            excerpt: String::new(),
            photo: None,
        };
        let out = listing(std::slice::from_ref(&item)).into_string();
        assert!(
            out.contains("<li class=\"listing-item no-photos\">"),
            "{out}"
        );
        assert!(
            !out.contains("<img"),
            "nothing stands in for a photo: {out}"
        );
        // One more photo is singular; none is no chip at all.
        let one_more = listing(&[ListingItem {
            photo: Some(ListingPhoto {
                src: "/p".into(),
                alt: String::new(),
                more: 1,
            }),
            ..item.clone()
        }])
        .into_string();
        assert!(one_more.contains(">+1 photo</span>"), "{one_more}");
        let alone = listing(&[ListingItem {
            photo: Some(ListingPhoto {
                src: "/p".into(),
                alt: String::new(),
                more: 0,
            }),
            ..item.clone()
        }])
        .into_string();
        assert!(!alone.contains("listing-photo-count"), "{alone}");
        assert!(alone.contains("alt=\"\""), "{alone}");
        assert!(
            out.contains("</h2><p class=\"rating\">Solid</p></div><p class=\"listing-meta\"><time datetime=\"2026-09-07T12:00:00.000Z\">September 7, 2026</time></p>"),
            "{out}"
        );
        assert!(!out.contains("listing-place-name"), "{out}");
        let out = listing(&[ListingItem {
            rating: None,
            ..item
        }])
        .into_string();
        assert!(!out.contains("rating"), "{out}");
        assert!(
            out.contains("</h2></div><p class=\"listing-meta\">"),
            "{out}"
        );
    }

    #[test]
    fn photo_grid_links_thumbnails_to_full_renditions() {
        assert_eq!(photo_grid(&[]).into_string(), "");
        let out = photo_grid(&[PhotoView {
            thumb_src: "/img/d/r/c?size=thumb".into(),
            full_src: "/img/d/r/c?size=full".into(),
            alt: "The <room>".into(),
        }])
        .into_string();
        assert!(
            out.starts_with(
                "<section class=\"photos\" aria-label=\"Photos\"><ul class=\"photo-grid\">"
            ),
            "{out}"
        );
        assert!(
            out.contains("<a href=\"/img/d/r/c?size=full\"><figure><img src=\"/img/d/r/c?size=thumb\" alt=\"\" width=\"400\" height=\"400\" loading=\"lazy\"><figcaption>The &lt;room&gt;</figcaption></figure></a>"),
            "{out}"
        );
    }

    #[test]
    fn visit_links_are_labelled_and_marked_ugc() {
        assert_eq!(visit_links(&[]).into_string(), "");
        let out = visit_links(&[link("Elsewhere", "https://example.com/x")]).into_string();
        assert!(
            out.starts_with("<nav class=\"visit-links\" aria-label=\"Links\">"),
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
