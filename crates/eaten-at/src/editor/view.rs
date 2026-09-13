//! The editor page: two panes where width allows, the write-up on the
//! left and the visit on the right, every control a plain form element.

use eaten_at_atproto::lexicon::{KnownValue, Rating};
use eaten_at_web::components::{tag_links, visit_card, visit_links, Link};
use eaten_at_web::markdown;
use maud::{html, Markup, PreEscaped};

use super::form::{Action, Choice, EditorForm, PlaceMode, RowKind, PUBLICATION_NEW};
use super::{default_post_text, DocumentDraft, FieldErrors};
use crate::places::Hit;
use crate::publish::MAX_POST_GRAPHEMES;

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

/// A publication the author can write to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationOption {
    pub uri: String,
    pub name: String,
}

/// What the choosing state shows under the search box.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum SearchState {
    /// Nothing searched yet.
    #[default]
    Idle,
    /// There was no point to search near: the browser gave none and
    /// the author has no visit with coordinates.
    NoPoint,
    /// What the search found, possibly nothing.
    Results(Vec<Hit>),
    /// Why the search did not run, in the author's words.
    Failed(String),
}

/// Where the search point came from, for the status line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Located {
    #[default]
    Unknown,
    Browser,
    LastVisit,
}

/// Everything the page needs.
#[derive(Debug)]
pub struct EditorPage<'a> {
    pub form: &'a EditorForm,
    pub errors: &'a FieldErrors,
    pub publications: &'a [PublicationOption],
    /// Where the form posts.
    pub action_path: &'a str,
    /// Set when editing an existing document.
    pub editing: bool,
    /// The heading: the write-up's title while editing, the place's name
    /// for a new one once there is a place.
    pub heading: Option<&'a str>,
    /// A rendered preview to show above the form.
    pub preview: Option<Markup>,
    /// Why the last publish attempt failed, when it did.
    pub publish_error: Option<&'a str>,
    /// Whether the document can be, or has been, posted to Bluesky.
    pub crosspost: CrosspostState,
    /// Whether the site has a place search at all.
    pub search_enabled: bool,
    pub search: SearchState,
    pub located: Located,
}

/// The `<main>` content of the editor page: the choosing state until a
/// place is picked or entered by hand, then the writing state.
pub fn page(page: &EditorPage<'_>) -> Markup {
    if page.form.place_mode == PlaceMode::Choosing {
        return choosing(page);
    }
    let form = page.form;
    let errors = page.errors;
    html! {
        div.page-head {
            @if let Some(heading) = page.heading {
                p.kicker { @if page.editing { "Edit" } @else { "Write" } }
                h1 { (heading) }
            }
            (alerts(page))
        }
        @if let Some(preview) = &page.preview {
            section.preview aria-label="Preview" {
                p.kicker { "Preview" }
                (preview)
            }
        }
        form.editor method="post" action=(page.action_path) novalidate {
            div.editor-panes {
                (writing_pane(form, errors))
                div.editor-pane.editor-visit {
                    fieldset.editor-group {
                        legend.kicker { "Place" }
                        (place_source(form))
                        (field("place_name", "Name", errors, &html! {
                            input #place_name name="place_name" type="text" value=(form.place_name) required
                                aria-describedby=[described(errors, "place_name")];
                        }))
                        (field("place_address", "Address (optional)", errors, &html! {
                            input #place_address name="place_address" type="text" value=(form.place_address)
                                autocomplete="off" aria-describedby=[described(errors, "place_address")];
                        }))
                        (field("place_price", "Price (optional)", errors, &html! {
                            select #place_price name="place_price" aria-describedby=[described(errors, "place_price")] {
                                option value="" selected[form.place_price.trim().is_empty()] { "Not said" }
                                @for band in 1..=4u8 {
                                    option value=(band) selected[form.place_price.trim() == band.to_string()] {
                                        ("$".repeat(usize::from(band)))
                                    }
                                }
                            }
                        }))
                        @if let Some(message) = errors.get("gers_id") {
                            p.field-error id="gers_id-error" { (message) }
                        }
                        input type="hidden" name="place_mode" value=(form.place_mode.value());
                        input type="hidden" name="gers_id" value=(form.gers_id);
                        input type="hidden" name="lat_e6" value=(form.lat_e6);
                        input type="hidden" name="lon_e6" value=(form.lon_e6);
                        input type="hidden" name="near_lat" value=(form.near_lat);
                        input type="hidden" name="near_lon" value=(form.near_lon);
                    }
                    fieldset.editor-group {
                        legend.kicker { "Visit" }
                        (field("visited_on", "Date", errors, &html! {
                            input #visited_on name="visited_on" type="date" value=(form.visited_on) required
                                aria-describedby=[described(errors, "visited_on")];
                        }))
                        (meal_select(form, errors))
                        (rating_choice(form, errors))
                    }
                    fieldset.editor-group {
                        legend.kicker { "Links" }
                        (links(form, errors))
                    }
                    fieldset.editor-group {
                        legend.kicker { "Details" }
                        (field("tags", "Tags (optional)", errors, &html! {
                            input #tags name="tags" type="text" value=(form.tags)
                                placeholder="notes, short, one long sit"
                                aria-describedby=[described(errors, "tags")];
                        }))
                        p.meta.field-hint { "Separate tags with commas." }
                        (publication(form, page.publications, errors))
                    }
                    fieldset.editor-group {
                        legend.kicker { "Bluesky" }
                        (bluesky(form, errors, &page.crosspost))
                    }
                }
            }
            div.actions {
                button type="submit" name="action" value=(Action::Publish.value()) {
                    @if page.editing { "Save changes" } @else { "Publish" }
                }
                button.link-button type="submit" name="action" value=(Action::Preview.value()) { "Preview" }
                @if page.editing {
                    a.button-link href=(format!("{}/delete", page.action_path)) { "Delete" }
                }
            }
        }
    }
}

/// The form-error summary and the publish error, in the page head.
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

/// Where the place came from, with the way back to choosing.
fn place_source(form: &EditorForm) -> Markup {
    html! {
        p.meta.place-source {
            @match form.place_mode {
                PlaceMode::Picked => { span { "Matched to an Overture Maps listing." } }
                PlaceMode::Manual => { span { "Entered by hand, not matched to a listing." } }
                PlaceMode::Choosing => {}
            }
            button.link-button type="submit" name="action" value=(Action::ChangePlace.value()) {
                @if form.place_mode == PlaceMode::Picked { "Change place" } @else { "Search for it" }
            }
        }
    }
}

/// The choosing state: a search box over a point the browser supplies,
/// the results as cards, and the way out by hand. Everything else the
/// form holds rides along as hidden fields so changing the place on an
/// edit loses nothing.
fn choosing(page: &EditorPage<'_>) -> Markup {
    let form = page.form;
    let query = form.place_query.trim();
    html! {
        div.page-head {
            p.kicker { @if page.editing { "Edit" } @else { "Write" } }
            h1 { "Where did you eat?" }
            (alerts(page))
        }
        form.editor.editor-choosing method="post" action=(page.action_path) novalidate
            data-locate[page.search_enabled] {
            (carried(form))
            input type="hidden" name="place_mode" value=(PlaceMode::Choosing.value());
            @if page.search_enabled {
                div.field.search-field {
                    label.kicker for="place_query" { "Name of the place" }
                    div.lookup-row {
                        input #place_query name="place_query" type="search" value=(form.place_query)
                            autocomplete="off" autofocus;
                        button type="submit" name="action" value=(Action::Search.value()) { "Search" }
                    }
                }
                input type="hidden" name="near_lat" value=(form.near_lat);
                input type="hidden" name="near_lon" value=(form.near_lon);
                p.meta.locate-status {
                    @match page.located {
                        Located::Browser => "Searching near you."
                        Located::LastVisit => "Searching near your last visit."
                        Located::Unknown => ""
                    }
                }
                @match &page.search {
                    SearchState::Idle => {}
                    SearchState::NoPoint => {
                        p.notice { "Turn on location to search for the place, or enter it by hand." }
                    }
                    SearchState::Failed(message) => { p.form-error role="alert" { (message) } }
                    SearchState::Results(hits) => {
                        @if hits.is_empty() {
                            p.empty { "Nothing nearby called “" (query) "”." }
                        } @else {
                            (results(hits))
                        }
                    }
                }
                p.meta.field-hint.manual-entry {
                    "Not listed? "
                    button.link-button type="submit" name="action" value=(Action::Manual.value()) { "Enter it by hand" }
                }
                p.meta.attribution {
                    "Places from " a href="https://overturemaps.org/" rel="noopener" { "Overture Maps" } "."
                }
            } @else {
                p.notice { "Place search is not set up on this site." }
                div.actions {
                    button type="submit" name="action" value=(Action::Manual.value()) { "Enter the place by hand" }
                }
            }
        }
    }
}

/// The search results, one card each.
fn results(hits: &[Hit]) -> Markup {
    html! {
        ol.results aria-label="Places found" {
            @for (i, hit) in hits.iter().enumerate() {
                li.result-item {
                    div.result-body {
                        p.result-name { (hit.name) }
                        p.meta.result-meta {
                            @if let Some(address) = &hit.address { (address) " · " }
                            (format!("{:.1} mi", hit.distance_mi))
                            @if let Some(category) = &hit.category { " · " (category) }
                        }
                    }
                    button.button-secondary type="submit" name="action" value=(Action::Pick(i).value()) {
                        "Write about this place"
                    }
                }
            }
        }
    }
}

/// Every field the choosing state does not show, as hidden inputs.
fn carried(form: &EditorForm) -> Markup {
    let hidden = |name: &str, value: &str| {
        html! { input type="hidden" name=(name) value=(value); }
    };
    html! {
        (hidden("title", &form.title))
        (hidden("body", &form.body))
        (hidden("description", &form.description))
        (hidden("place_name", &form.place_name))
        (hidden("place_address", &form.place_address))
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
        (hidden("publication", &form.publication))
        (hidden("new_publication_name", &form.new_publication_name))
        (hidden("new_publication_url", &form.new_publication_url))
        @if form.crosspost { (hidden("crosspost", "1")) }
        (hidden("post_text", &form.post_text))
    }
}

/// The left pane: title, write-up, excerpt.
fn writing_pane(form: &EditorForm, errors: &FieldErrors) -> Markup {
    html! {
        div.editor-pane.editor-writing {
            (field("title", "Title (optional)", errors, &html! {
                input #title name="title" type="text" value=(form.title)
                    placeholder=(form.place_name.trim())
                    aria-describedby=[described(errors, "title")];
            }))
            p.meta.field-hint { "Left blank, the place's name is the title." }
            (field("body", "Write-up", errors, &html! {
                textarea #body.editor-body name="body" rows="24" required
                    aria-describedby=[described(errors, "body")] { (form.body) }
            }))
            p.meta.field-hint { "Markdown. Headings, emphasis, lists, links, and quotes render; raw HTML does not." }
            (field("description", "Excerpt (optional)", errors, &html! {
                textarea #description name="description" rows="3"
                    aria-describedby=[described(errors, "description")] { (form.description) }
            }))
            p.meta.field-hint { "Shown in listings and link previews. Left blank, the first paragraph stands in." }
        }
    }
}

/// The crosspost toggle and its text (plan §5.7), or the link to the
/// post once there is one.
fn bluesky(form: &EditorForm, errors: &FieldErrors, state: &CrosspostState) -> Markup {
    let placeholder = default_post_text(&form.place_name);
    html! {
        @match state {
            CrosspostState::Posted(url) => {
                p.meta.field-hint {
                    "Posted to Bluesky: " a href=(url) rel="noopener" { "see the thread" } "."
                }
            }
            CrosspostState::Ready | CrosspostState::NeedsPermission => {
                div.field {
                    label.choice for="crosspost" {
                        input #crosspost name="crosspost" type="checkbox" value="1" checked[form.crosspost];
                        " Also post to Bluesky"
                    }
                }
                (field("post_text", "Post text", errors, &html! {
                    input #post_text name="post_text" type="text" value=(form.post_text)
                        placeholder=(placeholder) maxlength=(MAX_POST_GRAPHEMES)
                        aria-describedby=[described(errors, "post_text")];
                }))
                @if *state == CrosspostState::NeedsPermission {
                    p.meta.field-hint { "A link card with the title and excerpt goes with it. The first time, your account's server will ask you to allow posting." }
                } @else {
                    p.meta.field-hint { "A link card with the title and excerpt goes with it. Comments on Bluesky then show under the write-up." }
                }
            }
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
                    (field("post_text", "Post text", &FieldErrors::default(), &html! {
                        input #post_text name="post_text" type="text" value=(page.post_text)
                            maxlength=(MAX_POST_GRAPHEMES) required;
                    }))
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

fn described(errors: &FieldErrors, field: &str) -> Option<String> {
    errors.get(field).map(|_| format!("{field}-error"))
}

/// A labelled control with its error, if any, beneath it.
fn field(name: &str, label: &str, errors: &FieldErrors, control: &Markup) -> Markup {
    html! {
        div.field.field-invalid[errors.get(name).is_some()] {
            label.kicker for=(name) { (label) }
            (control)
            @if let Some(message) = errors.get(name) {
                p.field-error id=(format!("{name}-error")) { (message) }
            }
        }
    }
}

/// The remove button for a row, shown only while there is more than one.
fn row_remove(kind: RowKind, i: usize, rows: usize) -> Markup {
    html! {
        @if rows > 1 {
            button.link-button.row-remove type="submit" name="action" value=(Action::RemoveRow(kind, i).value()) {
                "Remove"
            }
        }
    }
}

/// The add button for a row kind, shown only under its cap.
fn row_add(kind: RowKind, rows: usize, label: &str) -> Markup {
    html! {
        @if rows < kind.cap() {
            p { button.link-button type="submit" name="action" value=(Action::AddRow(kind).value()) { (label) } }
        }
    }
}

fn links(form: &EditorForm, errors: &FieldErrors) -> Markup {
    html! {
        div.rows aria-label="Links" {
            @if let Some(message) = errors.get("links") { p.field-error { (message) } }
            @for (i, link) in form.links.iter().enumerate() {
                div.row.row-link {
                    (field(&format!("link_url_{i}"), "URL", errors, &html! {
                        input id=(format!("link_url_{i}")) name=(format!("link_url_{i}")) type="url" value=(link.url)
                            placeholder="https://" spellcheck="false"
                            aria-describedby=[described(errors, &format!("link_url_{i}"))];
                    }))
                    (KnownSelect {
                        name: format!("link_service_{i}"),
                        label: "What it is",
                        choice: &link.service,
                        none_label: "Other",
                    }.render(errors))
                    (field(&format!("link_label_{i}"), "Label (optional)", errors, &html! {
                        input id=(format!("link_label_{i}")) name=(format!("link_label_{i}")) type="text" value=(link.label)
                            aria-describedby=[described(errors, &format!("link_label_{i}"))];
                    }))
                    (row_remove(RowKind::Link, i, form.links.len()))
                }
            }
            (row_add(RowKind::Link, form.links.len(), "+ Add a link"))
        }
    }
}

/// A select over a lexicon's known values: a blank option, the known
/// values, and, only while editing a record that carries one, the
/// foreign value as written, so it survives the round trip (D31).
struct KnownSelect<'a, K: KnownValue> {
    name: String,
    label: &'a str,
    choice: &'a Choice<K>,
    /// What the blank option is called.
    none_label: &'static str,
}

impl<K: KnownValue + PartialEq> KnownSelect<'_, K> {
    fn render(&self, errors: &FieldErrors) -> Markup {
        let name = self.name.as_str();
        html! {
            div.field.field-invalid[errors.get(name).is_some()] {
                label.kicker for=(name) { (self.label) }
                select id=(name) name=(name) aria-describedby=[described(errors, name)] {
                    option value="" selected[*self.choice == Choice::None] { (self.none_label) }
                    @for value in K::ALL {
                        option value=(value.as_str()) selected[*self.choice == Choice::Known(*value)] {
                            (value.display_name())
                        }
                    }
                    @if let Choice::Foreign(value) = self.choice {
                        option value=(value) selected { (value) }
                    }
                }
                @if let Some(message) = errors.get(name) {
                    p.field-error id=(format!("{name}-error")) { (message) }
                }
            }
        }
    }
}

fn meal_select(form: &EditorForm, errors: &FieldErrors) -> Markup {
    KnownSelect {
        name: "meal".to_owned(),
        label: "Meal (optional)",
        choice: &form.meal,
        none_label: "Not said",
    }
    .render(errors)
}

/// The rating as a radio group: unrated, then the four steps, each
/// labelled with its marks and its word.
fn rating_choice(form: &EditorForm, errors: &FieldErrors) -> Markup {
    let current = form.rating.trim();
    html! {
        fieldset.field.rating-choice.field-invalid[errors.get("rating").is_some()] {
            legend.kicker { "Rating (optional)" }
            label.choice for="rating_none" {
                input #rating_none name="rating" type="radio" value="" checked[current.is_empty()];
                " Unrated"
            }
            @for rating in Rating::ALL {
                @let id = format!("rating_{}", rating.value());
                label.choice for=(id) {
                    input id=(id) name="rating" type="radio" value=(rating.value()) checked[current == rating.value().to_string()];
                    " "
                    span.rating aria-hidden="true" { (rating.marks()) }
                    " " (rating.word())
                }
            }
            @if let Some(message) = errors.get("rating") {
                p.field-error id="rating-error" { (message) }
            }
        }
    }
}

fn publication(
    form: &EditorForm,
    publications: &[PublicationOption],
    errors: &FieldErrors,
) -> Markup {
    html! {
        (field("publication", "Publication", errors, &html! {
            select #publication name="publication" aria-describedby=[described(errors, "publication")] {
                @for publication in publications {
                    option value=(publication.uri) selected[form.publication == publication.uri] { (publication.name) }
                }
                option value=(PUBLICATION_NEW) selected[form.publication == PUBLICATION_NEW || publications.is_empty()] {
                    @if publications.is_empty() { "A new publication" } @else { "A new publication…" }
                }
            }
        }))
        div.field-group.new-publication {
            (field("new_publication_name", "New publication's name", errors, &html! {
                input #new_publication_name name="new_publication_name" type="text" value=(form.new_publication_name)
                    aria-describedby=[described(errors, "new_publication_name")];
            }))
            (field("new_publication_url", "New publication's address", errors, &html! {
                input #new_publication_url name="new_publication_url" type="url" value=(form.new_publication_url)
                    placeholder="https://" spellcheck="false"
                    aria-describedby=[described(errors, "new_publication_url")];
            }))
            p.meta.field-hint { "Only needed for a new publication. Hosting on eaten.at comes with settings." }
        }
    }
}

/// The draft as readers would see it: date line, title, card, prose,
/// links, tags.
pub fn preview(draft: &DocumentDraft) -> Markup {
    let card = crate::view::card_for(&draft.visit);
    let tags: Vec<Link> = draft
        .tags
        .iter()
        .map(|t| Link {
            label: t.clone(),
            href: "#".to_owned(),
        })
        .collect();
    html! {
        article.document.preview-document {
            p.kicker { "Today" }
            h1.doc-title { (draft.title) }
            (visit_card(&card))
            div.prose { (PreEscaped(markdown::render(&draft.markdown))) }
            footer.doc-footer {
                @if !card.links.is_empty() {
                    div.doc-footer-row { (visit_links(&card.links)) }
                }
                @if !tags.is_empty() {
                    div.doc-footer-row { (tag_links(&tags, None)) }
                }
            }
        }
    }
}
