//! The editor page, set as prose (the Write Pages handoff,
//! `docs/design-handoff/write-pages/`): no boxed inputs, no field
//! labels, no buttons that look like buttons. Every editable value sits
//! inline inside a readable sentence, underlined; the page reads like a
//! typeset paragraph that happens to be editable.
//!
//! Every control here is still a plain form element the server reads:
//! the inline inputs, the date input, the rating radios, the meal and
//! price selects, the link rows. The islands dress them (a calendar, a
//! paper menu, chips, a live markdown editor, photos in place) and fall
//! away without JavaScript, when the same form still does the work.

use eaten_at_atproto::lexicon::{Meal, Rating};
use eaten_at_web::markdown;
use maud::{html, Markup};

use super::form::{Action, Choice, EditorForm, PlaceMode, RowKind};
use super::{default_post_text, FieldErrors};
use crate::publish::MAX_POST_GRAPHEMES;

/// How long the teaser the listings would draw themselves is shown as,
/// in characters.
const TEASER_PREVIEW_CHARS: usize = 160;

/// What the editor can offer about Bluesky for this document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrosspostState {
    /// The document already names a post, at this URL on bsky.app.
    Posted(String),
    /// The session may create posts.
    Ready,
    /// The session has not been granted posting yet; the first
    /// crosspost asks for it.
    NeedsPermission,
}

/// Where the search point came from (plan 12). The choosing screen no
/// longer says so; it decides whether suggestions are offered at all,
/// and the suggest endpoint names the city.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Located {
    /// No point at all: nothing to suggest near, so the place is typed.
    #[default]
    Unknown,
    /// The request's IP, with the city when the database names one.
    Ip(Option<String>),
    /// The author's most recent visit with coordinates.
    LastVisit,
}

/// Everything the page needs.
#[derive(Debug)]
pub struct EditorPage<'a> {
    pub form: &'a EditorForm,
    pub errors: &'a FieldErrors,
    /// Where the form posts.
    pub action_path: &'a str,
    /// Set when editing an existing document.
    pub editing: bool,
    /// Why the last publish attempt failed, when it did.
    pub publish_error: Option<&'a str>,
    /// Why a picked suggestion could not be taken, when it could not.
    pub pick_error: Option<&'a str>,
    /// Whether the document can be, or has been, posted to Bluesky.
    pub crosspost: CrosspostState,
    /// Whether the choosing screen suggests places as the author types:
    /// the site has a search and the request could be located.
    pub suggesting: bool,
    /// The photos page of the write-up being edited, the way photos are
    /// managed without script; a new write-up has none yet.
    pub photos_page: Option<&'a str>,
}

/// The `<main>` content of the editor page: the choosing screen until a
/// place is picked or typed, then the editing screen.
pub fn page(page: &EditorPage<'_>) -> Markup {
    if page.form.place_mode == PlaceMode::Choosing {
        return choosing(page);
    }
    let form = page.form;
    let errors = page.errors;
    let has_title = has_custom_title(form);
    html! {
        form.editor.editor-write method="post" action=(page.action_path) novalidate {
            (alerts(page))
            // Return in a text field presses the form's first button;
            // this one only re-renders the page, so nothing typed is ever
            // sent by accident.
            button.visually-hidden type="submit" name="action" value=(Action::Keep.value()) tabindex="-1" aria-hidden="true" { "Keep editing" }
            (title_and_place(form, errors, has_title))
            div.visit-row {
                (date_line(form, errors))
                (tags_line(form, errors))
            }
            (digest(form, errors))
            (teaser(form, errors))
            div.verdict-row {
                (rating_control(form, errors))
                div.notes-row {
                    (meal_field(form, errors))
                    (price_field(form, errors))
                }
            }
            (photos(page))
            div.links-row {
                (bluesky_line(form, errors, &page.crosspost))
                (elsewhere(form, errors))
            }
            div.actions.editor-actions {
                button type="submit" name="action" value=(Action::Publish.value()) {
                    @if page.editing { "Save changes" } @else { "Publish" }
                }
                @if page.editing {
                    (delete_confirm(page.action_path))
                }
            }
        }
    }
}

/// Whether the title is the author's own rather than the place's name
/// standing in (D29).
fn has_custom_title(form: &EditorForm) -> bool {
    let title = form.title.trim();
    !title.is_empty() && title != form.place_name.trim()
}

/// The headline and the place line under it. The title input's
/// placeholder is the place's name; while that stands in, the line
/// under it is the address alone and the change-place mark sits after
/// the headline. With a title of the author's own, the place's name
/// joins the line (`at Noodle House, 12 Example Lane.`) and the mark
/// moves down beside it. The reset mark is the island's to show.
fn title_and_place(form: &EditorForm, errors: &FieldErrors, has_title: bool) -> Markup {
    html! {
        div.title-row data-has-title[has_title] {
            input #title.headline name="title" type="text" value=(form.title)
                placeholder=(form.place_name.trim()) autocomplete="off"
                aria-label="Title" aria-describedby=[described(errors, "title")];
            button.mark.title-reset type="button" hidden title="Use the place's name as the title" aria-label="Use the place's name as the title" {
                (cross_icon(13))
            }
            @if !has_title { (change_place_button()) }
        }
        p.place-line {
            span.soft { "at" } " "
            span.place-name-slot hidden[!has_title] {
                span.inline-field {
                    input #place_name name="place_name" type="text" value=(form.place_name) required
                        autocomplete="off" aria-label="Place"
                        aria-describedby=[described(errors, "place_name")];
                }
                span.soft { "," } " "
            }
            span.inline-field {
                input #place_address name="place_address" type="text" value=(form.place_address)
                    placeholder="somewhere" autocomplete="off" aria-label="Address"
                    aria-describedby=[described(errors, "place_address")];
            }
            span.soft { "." }
            @if has_title { (change_place_button()) }
        }
        @for field in ["title", "place_name", "place_address", "gers_id"] {
            (field_error(errors, field))
        }
        input type="hidden" name="place_mode" value=(form.place_mode.value());
        input type="hidden" name="gers_id" value=(form.gers_id);
        input type="hidden" name="lat_e6" value=(form.lat_e6);
        input type="hidden" name="lon_e6" value=(form.lon_e6);
    }
}

/// Back to choosing, as the folded-map mark. Revealed under the pointer
/// by the stylesheet; a submit, so it works without script.
fn change_place_button() -> Markup {
    html! {
        button.mark.change-place type="submit" name="action" value=(Action::ChangePlace.value())
            title="Change place" aria-label="Change place" formnovalidate {
            (map_icon())
        }
    }
}

/// `From a visit on [date].` The date input is the carrier; the island
/// draws the calendar over it.
fn date_line(form: &EditorForm, errors: &FieldErrors) -> Markup {
    html! {
        div.date-line {
            p.prose-line {
                span.soft { "From a visit on" } " "
                span.date-field {
                    input #visited_on name="visited_on" type="date" value=(form.visited_on) required
                        aria-label="Date of the visit" aria-describedby=[described(errors, "visited_on")];
                }
                "."
            }
            (field_error(errors, "visited_on"))
        }
    }
}

/// `Filed under [tags].` Without script the blank is the comma list on a
/// hairline; the tags island files what is typed as chips before it.
fn tags_line(form: &EditorForm, errors: &FieldErrors) -> Markup {
    html! {
        div.tags-line.field-invalid[errors.get("tags").is_some()] {
            p.tags-sentence {
                label for="tags" { "Filed under" }
                " "
                span.tag-field {
                    input #tags name="tags" type="text" value=(form.tags)
                        placeholder="a tag" autocomplete="off"
                        aria-describedby=[described(errors, "tags")];
                }
                "."
            }
            (field_error(errors, "tags"))
        }
    }
}

/// The write-up, under its kicker. The textarea is the carrier; the
/// island puts a live markdown editor in its place.
fn digest(form: &EditorForm, errors: &FieldErrors) -> Markup {
    html! {
        div.digest.field-invalid[errors.get("body").is_some()] {
            p.kicker #body-label { "Digest" }
            textarea #body.editor-body name="body" rows="18" required
                aria-labelledby="body-label" aria-describedby=[described(errors, "body")] { (form.body) }
            (field_error(errors, "body"))
        }
    }
}

/// The teaser (the record's `description`, D19): folded to one line
/// while the first lines stand in, open once the author writes their
/// own. Without script the fold is a disclosure, which opens by itself.
fn teaser(form: &EditorForm, errors: &FieldErrors) -> Markup {
    let custom = !form.description.trim().is_empty();
    let drawn = markdown::excerpt(&form.body, TEASER_PREVIEW_CHARS);
    let auto = if drawn.is_empty() {
        "the first lines, once there are some".to_owned()
    } else {
        format!("“{drawn}”")
    };
    html! {
        details.teaser.field-invalid[errors.get("description").is_some()] open[custom] {
            summary.hint {
                "In listings, the piece opens with its first lines — or "
                span.hint-action { "write your own teaser" }
                "."
            }
            p.hint.teaser-hint { "In listings it opens:" }
            textarea #description.teaser-text name="description" rows="2" placeholder=(auto)
                aria-label="Teaser" aria-describedby=[described(errors, "description")] { (form.description) }
            p.hint.teaser-note.teaser-default hidden[custom] {
                "Drawn from the first lines — type to say it differently, or "
                button.hint-action type="button" data-teaser="fold" { "leave it be" }
                "."
            }
            p.hint.teaser-note.teaser-custom hidden[!custom] {
                button.hint-action type="button" data-teaser="reset" { "never mind — use the first lines" }
            }
            (field_error(errors, "description"))
        }
    }
}

/// The rating: a clear box, four plus signs, and the verdict's word. The
/// radios are the carrier and the stylesheet does the rest, filling the
/// pluses up to the checked one and previewing under the pointer; the
/// pluses are set in reverse so a sibling selector can reach the lower
/// steps from the checked one.
fn rating_control(form: &EditorForm, errors: &FieldErrors) -> Markup {
    let current = form.rating.trim();
    html! {
        fieldset.rating-control.field-invalid[errors.get("rating").is_some()] {
            legend.visually-hidden { "Rating" }
            input #rating_none.r0 name="rating" type="radio" value="" checked[current.is_empty()];
            label.rating-clear for="rating_none" title="Clear rating" { (cross_icon(10)) }
            span.pluses {
                @for rating in Rating::ALL.iter().rev() {
                    @let value = rating.value();
                    @let id = format!("rating_{value}");
                    input id=(id) class=(format!("r{value}")) name="rating" type="radio" value=(value)
                        checked[current == value.to_string()];
                    label class=(format!("plus plus-{value}")) for=(id) title=(rating.word()) { "+" }
                }
            }
            span.rating-word aria-hidden="true" {
                span.word.word-0 { "Unrated" }
                @for rating in Rating::ALL {
                    span class=(format!("word word-{}", rating.value())) { (rating.word()) }
                }
            }
            (field_error(errors, "rating"))
        }
    }
}

/// The meal note: a select the island redraws as a word on a hairline
/// over a paper menu. Blank reads as "a meal" in stone.
fn meal_field(form: &EditorForm, errors: &FieldErrors) -> Markup {
    html! {
        span.note-field.field-invalid[errors.get("meal").is_some()] data-note="meal" data-unset="a meal" {
            span.select-rule {
                select #meal name="meal" aria-label="Meal" aria-describedby=[described(errors, "meal")] {
                    option value="" selected[form.meal == Choice::None] { "a meal" }
                    @for meal in Meal::ALL {
                        option value=(meal.as_str()) selected[form.meal == Choice::Known(*meal)] {
                            (meal.display_name())
                        }
                    }
                    @if let Choice::Foreign(value) = &form.meal {
                        option value=(value) selected { (value) }
                    }
                }
            }
            (field_error(errors, "meal"))
        }
    }
}

/// The price band, `$` to `$$$$`; blank reads as `$?`.
fn price_field(form: &EditorForm, errors: &FieldErrors) -> Markup {
    let current = form.place_price.trim();
    html! {
        span.note-field.field-invalid[errors.get("place_price").is_some()] data-note="price" data-unset="$?" {
            span.select-rule {
                select #place_price name="place_price" aria-label="Price" aria-describedby=[described(errors, "place_price")] {
                    option value="" selected[current.is_empty()] { "$?" }
                    @for band in 1..=4u8 {
                        option value=(band) selected[current == band.to_string()] {
                            ("$".repeat(usize::from(band)))
                        }
                    }
                }
            }
            (field_error(errors, "place_price"))
        }
    }
}

/// The photos, under their kicker: tiles in the author's order, the
/// first the cover, an add tile last; or one dashed box. The blob
/// references ride in the form as hidden fields, uploaded by the island
/// the moment a file is picked and written with the record on save, so
/// a write-up has its photos before it exists (D37 amended). Without
/// script the tiles link to the photos page once there is a record.
fn photos(page: &EditorPage<'_>) -> Markup {
    let form = page.form;
    let errors = page.errors;
    html! {
        div.photos-block {
            p.kicker { "Photos" }
            @if form.photos.is_empty() {
                @match page.photos_page {
                    Some(path) => {
                        a.photo-empty href=(path) data-upload=(crate::paths::UPLOAD) {
                            span.photo-empty-idle { "Nothing to look at yet." }
                            span.photo-empty-hover aria-hidden="true" { "add a photo" }
                        }
                    }
                    None => {
                        span.photo-empty data-upload=(crate::paths::UPLOAD) {
                            span.photo-empty-idle { "Nothing to look at yet." }
                            span.photo-empty-hover aria-hidden="true" { "add a photo" }
                        }
                    }
                }
            } @else {
                div.photo-tiles data-upload=(crate::paths::UPLOAD) role="list" {
                    @for (i, photo) in form.photos.iter().enumerate() {
                        figure.photo-tile role="listitem" data-cid=(photo.cid) data-mime=(photo.mime) data-size=(photo.size)
                            data-width=(photo.width) data-height=(photo.height) data-alt=(photo.alt)
                            data-full=(crate::paths::own_photo(&photo.cid, "full")) {
                            img src=(crate::paths::own_photo(&photo.cid, "thumb")) alt=(photo.alt) width="400" height="400" loading="lazy" draggable="false";
                            @if i == 0 { span.cover-badge { "Cover" } }
                        }
                    }
                    @match page.photos_page {
                        Some(path) => { a.photo-add href=(path) title="Add photos" aria-label="Add photos" { "+" } }
                        None => { span.photo-add aria-hidden="true" { "+" } }
                    }
                }
                p.hint.photo-hint {
                    "Drag to reorder — the first photo is the cover."
                    @if let Some(path) = page.photos_page {
                        " " a href=(path) { "Captions and more" } "."
                    }
                }
            }
            div.photo-fields { (photo_fields(form)) }
            (field_error(errors, "photos"))
            @for i in 0..form.photos.len() {
                (field_error(errors, &format!("photo_cid_{i}")))
                (field_error(errors, &format!("photo_alt_{i}")))
            }
        }
    }
}

/// The photos as the form carries them.
fn photo_fields(form: &EditorForm) -> Markup {
    html! {
        @for (i, photo) in form.photos.iter().enumerate() {
            input type="hidden" name=(format!("photo_cid_{i}")) value=(photo.cid);
            input type="hidden" name=(format!("photo_mime_{i}")) value=(photo.mime);
            input type="hidden" name=(format!("photo_size_{i}")) value=(photo.size);
            input type="hidden" name=(format!("photo_alt_{i}")) value=(photo.alt);
            input type="hidden" name=(format!("photo_width_{i}")) value=(photo.width);
            input type="hidden" name=(format!("photo_height_{i}")) value=(photo.height);
        }
    }
}

/// `Bluesky: see the thread.` once there is a post; before one, the
/// toggle and the post's text in the same sentence.
fn bluesky_line(form: &EditorForm, errors: &FieldErrors, state: &CrosspostState) -> Markup {
    let placeholder = default_post_text(&form.place_name);
    html! {
        div.bluesky-line {
            p.prose-line {
                span.soft { "Bluesky:" } " "
                @match state {
                    CrosspostState::Posted(url) => {
                        a.quiet-link href=(url) rel="noopener" { "see the thread" }
                        span.soft { "." }
                    }
                    CrosspostState::Ready | CrosspostState::NeedsPermission => {
                        label.choice.choice-inline for="crosspost" {
                            input #crosspost name="crosspost" type="checkbox" value="1" checked[form.crosspost];
                            " post it too"
                        }
                        span.soft { ", saying" } " "
                        span.inline-field {
                            input #post_text name="post_text" type="text" value=(form.post_text)
                                placeholder=(placeholder) maxlength=(MAX_POST_GRAPHEMES)
                                aria-label="Post text" aria-describedby=[described(errors, "post_text")];
                        }
                        span.soft { "." }
                    }
                }
            }
            @if *state == CrosspostState::NeedsPermission {
                p.hint { "The first time, your account's server will ask you to allow posting." }
            }
            (field_error(errors, "post_text"))
        }
    }
}

/// `Elsewhere:` the place's links. Each link is a small paper card of
/// two prose rows; without script every card is open, with it the row
/// reads the labels and opens one card at a time. Blank rows are
/// skipped by the server, so a card left empty costs nothing.
fn elsewhere(form: &EditorForm, errors: &FieldErrors) -> Markup {
    let rows = form.links.len();
    html! {
        div.elsewhere {
            p.prose-line.elsewhere-line {
                span.soft { "Elsewhere:" } " "
                span.link-words { }
                span.nowhere hidden { "nowhere yet" }
                " "
                span.add-link-slot {
                    span.soft-stone { "— " }
                    @if rows < RowKind::Link.cap() {
                        button.hint-action.add-link type="submit" name="action" value=(Action::AddRow(RowKind::Link).value()) formnovalidate { "add a link" }
                    }
                }
            }
            (field_error(errors, "links"))
            div.link-cards {
                @for (i, link) in form.links.iter().enumerate() {
                    @let url_name = format!("link_url_{i}");
                    @let label_name = format!("link_label_{i}");
                    div.link-card.paper data-link=(i) {
                        p.prose-line {
                            span.soft { "shown as" } " "
                            span.inline-field {
                                input id=(label_name) name=(label_name) type="text" value=(link.label)
                                    placeholder="Website" autocomplete="off" aria-label="Shown as"
                                    aria-describedby=[described(errors, &label_name)];
                            }
                        }
                        p.prose-line {
                            span.soft { "pointing at" } " "
                            span.inline-field {
                                input id=(url_name) name=(url_name) type="url" value=(link.url)
                                    placeholder="https://…" autocomplete="off" spellcheck="false" aria-label="Pointing at"
                                    aria-describedby=[described(errors, &url_name)];
                            }
                            " "
                            a.open-link href=(if link.url.trim().is_empty() { "#" } else { link.url.trim() })
                                target="_blank" rel="noopener" title="Open in a new tab" aria-label="Open in a new tab" {
                                (link_icon())
                            }
                        }
                        input type="hidden" name=(format!("link_service_{i}")) value=(link.service.value());
                        (field_error(errors, &label_name))
                        (field_error(errors, &url_name))
                        p.hint.link-card-foot {
                            span.link-card-keep hidden {
                                button.hint-action type="button" data-link-save { "save it" }
                                " · "
                                button.hint-action type="button" data-link-cancel { "never mind" }
                            }
                            @if rows > 1 {
                                button.hint-action.danger type="submit" name="action" value=(Action::RemoveRow(RowKind::Link, i).value()) formnovalidate { "remove it" }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Delete, with the confirmation in the same slot: a disclosure whose
/// summary reads "Delete" closed and "Delete?" open, then Yes, which
/// posts to the delete route with the Bluesky post's deletion asked for
/// as the delete page asks it, and No, which the island closes.
fn delete_confirm(action_path: &str) -> Markup {
    html! {
        details.delete-confirm {
            summary.button-secondary {
                span.del-idle { "Delete" }
                span.del-ask aria-hidden="true" { "Delete?" }
            }
            span.del-choice {
                button.confirm-yes type="submit" formaction=(format!("{action_path}/delete")) formmethod="post" formnovalidate
                    name="delete_post" value="1" { "Yes" }
                button.confirm-no type="button" { "No" }
            }
        }
    }
}

/// The choosing screen: the place's name as the headline, the address
/// as the line under it, and "Start writing"; nothing else. Suggestions open under
/// the name as the author types; picking one fills both lines, and
/// Start writing then takes the pick. Anything else typed is a place by
/// hand. Everything else the form holds rides along hidden so changing
/// the place on an edit loses nothing.
fn choosing(page: &EditorPage<'_>) -> Markup {
    let form = page.form;
    let errors = page.errors;
    html! {
        form.editor.editor-choosing method="post" action=(page.action_path) novalidate {
            (alerts(page))
            @if let Some(message) = page.pick_error {
                p.form-error role="alert" { (message) }
            }
            (carried(form))
            input type="hidden" name="place_mode" value=(PlaceMode::Choosing.value());
            input type="hidden" name="place_query" value=(form.place_query);
            div.place-head {
                input #place_name.headline name="place_name" type="text" value=(form.place_name)
                    placeholder="St. John" autocomplete="off" autofocus
                    aria-label="Name of the place"
                    data-suggest=[page.suggesting.then_some("/write/suggest")]
                    aria-describedby=[described(errors, "place_name")];
            }
            p.place-line {
                span.soft { "at" } " "
                span.inline-field {
                    input #place_address name="place_address" type="text" value=(form.place_address)
                        placeholder="26 St John Street, London" autocomplete="off"
                        aria-label="Address" aria-describedby=[described(errors, "place_address")];
                }
                span.soft { "." }
            }
            (field_error(errors, "place_name"))
            (field_error(errors, "place_address"))
            div.actions.start-writing {
                button #start-writing type="submit" name="action" value=(Action::Manual.value()) { "Start writing" }
            }
        }
    }
}

/// Every field the choosing screen does not show, as hidden inputs.
fn carried(form: &EditorForm) -> Markup {
    let hidden = |name: &str, value: &str| {
        html! { input type="hidden" name=(name) value=(value); }
    };
    html! {
        (hidden("title", &form.title))
        (hidden("body", &form.body))
        (hidden("description", &form.description))
        (hidden("place_price", &form.place_price))
        (hidden("visited_on", &form.visited_on))
        (hidden("meal", form.meal.value()))
        (hidden("rating", &form.rating))
        @for (i, link) in form.links.iter().enumerate() {
            (hidden(&format!("link_url_{i}"), &link.url))
            (hidden(&format!("link_service_{i}"), link.service.value()))
            (hidden(&format!("link_label_{i}"), &link.label))
        }
        (hidden("tags", &form.tags))
        (photo_fields(form))
        @if form.crosspost { (hidden("crosspost", "1")) }
        (hidden("post_text", &form.post_text))
    }
}

/// The form-error summary and the publish error, above the form.
fn alerts(page: &EditorPage<'_>) -> Markup {
    let errors = page.errors;
    html! {
        @if !errors.is_empty() {
            p.form-error role="alert" {
                @if errors.len() == 1 { "One thing to fix below." }
                @else { (errors.len()) " things to fix below." }
            }
        }
        @if let Some(message) = page.publish_error {
            p.form-error role="alert" { (message) }
        }
    }
}

fn described(errors: &FieldErrors, field: &str) -> Option<String> {
    errors.get(field).map(|_| format!("{field}-error"))
}

/// A field's problem, under the line it belongs to.
fn field_error(errors: &FieldErrors, field: &str) -> Markup {
    html! {
        @if let Some(message) = errors.get(field) {
            p.field-error id=(format!("{field}-error")) { (message) }
        }
    }
}

/// Two crossing strokes: the reset and clear mark.
fn cross_icon(size: u8) -> Markup {
    html! {
        svg width=(size) height=(size) viewBox="0 0 10 10" aria-hidden="true" focusable="false" {
            path d="M 1 1 L 9 9 M 9 1 L 1 9" stroke="currentColor" stroke-width="1.4" fill="none" {}
        }
    }
}

/// A folded map: change place.
fn map_icon() -> Markup {
    html! {
        svg width="15" height="15" viewBox="0 0 24 24" aria-hidden="true" focusable="false" {
            path d="M3 6 L9 4 L15 6 L21 4 L21 18 L15 20 L9 18 L3 20 Z" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round" {}
            path d="M9 4 L9 18" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round" {}
            path d="M15 6 L15 20" stroke="currentColor" stroke-width="2" fill="none" stroke-linecap="round" stroke-linejoin="round" {}
        }
    }
}

/// A chain link: open the URL.
fn link_icon() -> Markup {
    html! {
        svg width="14" height="14" viewBox="0 0 24 24" aria-hidden="true" focusable="false" {
            path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71" stroke="currentColor" stroke-width="2.2" fill="none" stroke-linecap="round" stroke-linejoin="round" {}
        }
    }
}

/// The crosspost page (`/write/{rkey}/crosspost`): where a document
/// goes to Bluesky after publish when that could not happen inline, and
/// where a failed attempt is retried.
#[derive(Debug)]
pub struct CrosspostPage<'a> {
    pub rkey: &'a str,
    pub title: &'a str,
    /// The document's own page.
    pub document_path: &'a str,
    pub post_text: &'a str,
    pub error: Option<&'a str>,
    pub state: CrosspostState,
}

/// The `<main>` content of the crosspost page.
pub fn crosspost_page(page: &CrosspostPage<'_>) -> Markup {
    let action = format!("/write/{}/crosspost", page.rkey);
    let return_to = format!(
        "{action}?text={}",
        eaten_at_web::layout::urlencoding(page.post_text)
    );
    html! {
        div.page-head {
            p.kicker { "Bluesky" }
            @match &page.state {
                CrosspostState::Posted(_) => { h1 { "“" (page.title) "” is on Bluesky" } }
                CrosspostState::Ready => {
                    h1 { "Post “" (page.title) "” to Bluesky" }
                    p.lede { "The write-up is published. A post with a link card to it goes to your Bluesky account, and replies to it show under the write-up." }
                }
                CrosspostState::NeedsPermission => {
                    h1 { "Post “" (page.title) "” to Bluesky" }
                    p.lede { "The write-up is published. To post it to Bluesky, eaten.at needs your account's permission to create posts; your server asks once." }
                }
            }
            @if let Some(error) = page.error {
                p.form-error role="alert" { (error) }
            }
        }
        @match &page.state {
            CrosspostState::Posted(url) => {
                p.meta { a href=(url) rel="noopener" { "See the thread on Bluesky →" } }
                p.actions { a.button-link href=(page.document_path) { "← Back to the write-up" } }
            }
            CrosspostState::Ready => {
                form.crosspost-form method="post" action=(action) {
                    div.field {
                        label.kicker for="post_text" { "Post text" }
                        input #post_text name="post_text" type="text" value=(page.post_text)
                            maxlength=(MAX_POST_GRAPHEMES) required;
                    }
                    p.meta.field-hint { "A link card with the title and excerpt goes with it." }
                    div.actions {
                        button type="submit" { "Post to Bluesky" }
                        a.button-link href=(page.document_path) { "← Skip for now" }
                    }
                }
            }
            CrosspostState::NeedsPermission => {
                form.actions method="post" action="/login/bluesky" {
                    input type="hidden" name="return_to" value=(return_to);
                    button type="submit" { "Allow posting and continue" }
                    a.button-link href=(page.document_path) { "← Skip for now" }
                }
            }
        }
    }
}

/// The delete confirmation page.
#[derive(Debug)]
pub struct DeletePage<'a> {
    pub rkey: &'a str,
    pub title: &'a str,
    /// What happens to the Bluesky post the document names, if any.
    pub post: DeletePost,
}

/// The Bluesky post's fate on delete (plan §5.7: never silent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeletePost {
    /// The document names no post.
    None,
    /// The session may delete it; offered, ticked by default.
    Offered,
    /// The session may not delete it; said so.
    NotPermitted(String),
}

/// The `<main>` content of the delete page.
pub fn delete_page(page: &DeletePage<'_>) -> Markup {
    html! {
        div.page-head {
            p.kicker { "Delete" }
            h1 { "Delete “" (page.title) "”?" }
            p.lede { "The write-up is removed from your repository. Links to it stop working. There is no undo." }
        }
        form.actions.delete-form method="post" action=(format!("/write/{}/delete", page.rkey)) {
            @match &page.post {
                DeletePost::None => {}
                DeletePost::Offered => {
                    div.field {
                        label.choice for="delete_post" {
                            input #delete_post name="delete_post" type="checkbox" value="1" checked;
                            " Also delete the Bluesky post and its thread link"
                        }
                    }
                }
                DeletePost::NotPermitted(url) => {
                    p.meta.field-hint {
                        "The " a href=(url) rel="noopener" { "Bluesky post" }
                        " stays: this sign-in may not delete posts. Delete it on Bluesky if you want it gone."
                    }
                }
            }
            button type="submit" { "Delete it" }
            a.button-link href=(format!("/write/{}", page.rkey)) { "← Keep it" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_for(form: &EditorForm, photos_page: Option<&str>) -> String {
        page(&EditorPage {
            form,
            errors: &FieldErrors::default(),
            action_path: "/write/d1",
            editing: true,
            publish_error: None,
            pick_error: None,
            crosspost: CrosspostState::Ready,
            suggesting: true,
            photos_page,
        })
        .into_string()
    }

    #[test]
    fn the_place_line_follows_the_title() {
        let mut form = EditorForm::blank();
        form.place_mode = PlaceMode::Manual;
        form.place_name = "Noodle House".into();
        form.place_address = "12 Example Lane".into();
        // The place's name stands in as the title: the line under it is
        // the address alone, and the change-place mark sits by the title.
        let out = page_for(&form, None);
        assert!(out.contains("placeholder=\"Noodle House\""), "{out}");
        assert!(
            out.contains("<span class=\"place-name-slot\" hidden>"),
            "{out}"
        );
        let title_row = out.split("<p class=\"place-line\">").next().unwrap();
        assert!(title_row.contains("value=\"change_place\""), "{title_row}");
        // A title of the author's own brings the name onto the line.
        form.title = "Late at the Noodle House".into();
        let out = page_for(&form, None);
        assert!(out.contains("<span class=\"place-name-slot\">"), "{out}");
        let place_line = out.split("<p class=\"place-line\">").nth(1).unwrap();
        assert!(
            place_line.contains("value=\"change_place\""),
            "{place_line}"
        );
        // A title that only repeats the name is not one (D29).
        form.title = "Noodle House ".into();
        let out = page_for(&form, None);
        assert!(
            out.contains("<span class=\"place-name-slot\" hidden>"),
            "{out}"
        );
    }

    #[test]
    fn the_rating_is_set_in_reverse_so_the_stylesheet_can_fill_it() {
        let mut form = EditorForm::blank();
        form.place_mode = PlaceMode::Manual;
        form.place_name = "Cart".into();
        form.rating = "3".into();
        let out = page_for(&form, None);
        let pluses = out
            .split("<span class=\"pluses\">")
            .nth(1)
            .unwrap()
            .split("</span>")
            .next()
            .unwrap();
        let order: Vec<&str> = pluses
            .match_indices("id=\"rating_")
            .map(|(i, _)| &pluses[i + 11..i + 12])
            .collect();
        assert_eq!(order, ["4", "3", "2", "1"]);
        assert!(pluses.contains("value=\"3\" checked"), "{pluses}");
        assert!(
            out.contains("<span class=\"word word-3\">Strongly Recommended</span>"),
            "{out}"
        );
    }

    #[test]
    fn photos_are_tiles_carried_as_fields_whether_or_not_there_is_a_record() {
        use super::super::form::PhotoField;
        let mut form = EditorForm::blank();
        form.place_mode = PlaceMode::Picked;
        form.place_name = "Cart".into();
        form.photos = vec![
            PhotoField {
                cid: "bafya".into(),
                mime: "image/jpeg".into(),
                size: "10".into(),
                alt: "The room".into(),
                width: "4".into(),
                height: "6".into(),
            },
            PhotoField {
                cid: "bafyb".into(),
                mime: "image/jpeg".into(),
                size: "11".into(),
                ..PhotoField::default()
            },
        ];
        // A new write-up: the same tiles, with nowhere else to go.
        let out = page_for(&form, None);
        assert_eq!(out.matches("class=\"photo-tile\"").count(), 2, "{out}");
        assert_eq!(out.matches("class=\"cover-badge\"").count(), 1, "{out}");
        assert!(out.contains("data-upload=\"/write/upload\""), "{out}");
        assert!(
            out.contains("src=\"/write/photo/bafya?size=thumb\""),
            "{out}"
        );
        assert!(
            out.contains("name=\"photo_cid_1\" value=\"bafyb\""),
            "{out}"
        );
        assert!(
            out.contains("name=\"photo_alt_0\" value=\"The room\""),
            "{out}"
        );
        assert!(out.contains("<span class=\"photo-add\""), "{out}");
        assert!(!out.contains("href=\"/write/"), "{out}");
        // Editing: the photos page behind the add tile for readers
        // without script.
        let out = page_for(&form, Some("/write/d1/photos"));
        assert!(
            out.contains("<a class=\"photo-add\" href=\"/write/d1/photos\""),
            "{out}"
        );
        form.photos.clear();
        let out = page_for(&form, Some("/write/d1/photos"));
        assert!(out.contains("Nothing to look at yet."), "{out}");
        assert!(
            out.contains("<a class=\"photo-empty\" href=\"/write/d1/photos\""),
            "{out}"
        );
        let out = page_for(&form, None);
        assert!(
            out.contains("<span class=\"photo-empty\" data-upload=\"/write/upload\">"),
            "{out}"
        );
    }

    #[test]
    fn return_only_keeps_editing() {
        let mut form = EditorForm::blank();
        form.place_mode = PlaceMode::Manual;
        form.place_name = "Cart".into();
        let out = page_for(&form, None);
        let first_button = out.find("<button").unwrap();
        let first = &out[first_button..first_button + 160];
        assert!(first.contains("value=\"keep\""), "{first}");
    }
}
