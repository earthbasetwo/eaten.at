//! The editor page: two panes where width allows, the write-up on the
//! left and the subject on the right, every control a plain form element.

use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use eaten_at_atproto::lexicon::KnownService;
use eaten_at_web::components::{subject_card, subject_links, tag_links, Link, SubjectCard};
use eaten_at_web::markdown;
use maud::{html, Markup, PreEscaped};

use super::form::{
    Action, EditorForm, LinkField, ServiceChoice, PUBLICATION_NEW, SERVICE_NONE, SERVICE_OTHER,
};
use super::{default_post_text, DocumentDraft, FieldErrors, MAX_LINKS};
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
    /// Whether the document already has a cover that a blank file field
    /// keeps.
    pub has_cover: bool,
    /// A rendered preview to show above the form.
    pub preview: Option<Markup>,
    /// Why the last publish attempt failed, when it did.
    pub publish_error: Option<&'a str>,
    /// Whether the document can be, or has been, posted to Bluesky.
    pub crosspost: CrosspostState,
}

/// The `<main>` content of the editor page.
pub fn page(page: &EditorPage<'_>) -> Markup {
    let form = page.form;
    let errors = page.errors;
    html! {
        div.page-head {
            p.kicker { @if page.editing { "Edit" } @else { "Write" } }
            h1 { @if page.editing { "Edit this write-up" } @else { "A new write-up" } }
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
        @if let Some(preview) = &page.preview {
            section.preview aria-label="Preview" {
                p.kicker { "Preview" }
                (preview)
            }
        }
        form.editor method="post" action=(page.action_path) enctype="multipart/form-data" novalidate {
            div.editor-panes {
                div.editor-pane.editor-writing {
                    (field("title", "Title", errors, &html! {
                        input #title name="title" type="text" value=(form.title) required
                            aria-describedby=[described(errors, "title")];
                    }))
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
                div.editor-pane.editor-subject {
                    fieldset.editor-group {
                        legend.kicker { "Subject" }
                        (field("subject_title", "Subject title", errors, &html! {
                            input #subject_title name="subject_title" type="text" value=(form.subject_title) required
                                aria-describedby=[described(errors, "subject_title")];
                        }))
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
                        (field("cover", "Cover image (optional)", errors, &html! {
                            input #cover name="cover" type="file" accept="image/*"
                                aria-describedby=[described(errors, "cover")];
                        }))
                        @if page.has_cover {
                            p.meta.field-hint { "The current cover stays unless you choose a new file." }
                        } @else {
                            p.meta.field-hint { "Under 5 MB. Without one, a placeholder is generated from the subject's title." }
                        }
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

/// The crosspost toggle and its text (plan §5.7), or the link to the
/// post once there is one.
fn bluesky(form: &EditorForm, errors: &FieldErrors, state: &CrosspostState) -> Markup {
    let placeholder = default_post_text(&form.subject_title);
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
                    p.meta.field-hint { "A link card with the title, excerpt, and cover goes with it. The first time, your account's server will ask you to allow posting." }
                } @else {
                    p.meta.field-hint { "A link card with the title, excerpt, and cover goes with it. Comments on Bluesky then show under the write-up." }
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
                    p.meta.field-hint { "A link card with the title, excerpt, and cover goes with it." }
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
                    (service_select(i, link))
                    (field(&format!("link_label_{i}"), "Label (optional)", errors, &html! {
                        input id=(format!("link_label_{i}")) name=(format!("link_label_{i}")) type="text" value=(link.label)
                            aria-describedby=[described(errors, &format!("link_label_{i}"))];
                    }))
                    @if form.links.len() > 1 {
                        button.link-button.row-remove type="submit" name="action" value=(Action::RemoveLink(i).value()) {
                            "Remove"
                        }
                    }
                }
            }
            @if form.links.len() < MAX_LINKS {
                p { button.link-button type="submit" name="action" value=(Action::AddLink.value()) { "+ Add a link" } }
            }
        }
    }
}

fn service_select(i: usize, link: &LinkField) -> Markup {
    let name = format!("link_service_{i}");
    let other_name = format!("link_service_other_{i}");
    html! {
        div.field {
            label.kicker for=(name) { "Service" }
            select id=(name) name=(name) {
                option value="" selected[link.service == ServiceChoice::Unset] { "Choose…" }
                @for service in KnownService::ALL {
                    option value=(service.as_str()) selected[link.service == ServiceChoice::Known(service)] {
                        (service.display_name())
                    }
                }
                option value=(SERVICE_OTHER) selected[link.service == ServiceChoice::Other] { "Other" }
                option value=(SERVICE_NONE) selected[link.service == ServiceChoice::None] { "No service" }
            }
        }
        div.field.field-other {
            label.kicker for=(other_name) { "Other service" }
            input id=(other_name) name=(other_name) type="text" value=(link.service_other) placeholder="as the link's own client names it";
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
/// links, tags. A new cover is shown inline.
pub fn preview(draft: &DocumentDraft) -> Markup {
    let card = SubjectCard {
        title: draft.subject.title.clone(),
        cover_src: draft
            .cover
            .as_ref()
            .map(|c| format!("data:{};base64,{}", c.mime, STANDARD.encode(&c.bytes))),
        links: draft
            .subject
            .external_urls
            .iter()
            .map(|u| Link {
                label: u.label.clone().unwrap_or_else(|| {
                    u.service
                        .as_deref()
                        .and_then(KnownService::from_value)
                        .map_or_else(|| host_of(&u.url), |s| s.display_name().to_owned())
                }),
                href: u.url.clone(),
            })
            .collect(),
    };
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
            (subject_card(&card))
            div.prose { (PreEscaped(markdown::render(&draft.markdown))) }
            footer.doc-footer {
                @if !card.links.is_empty() {
                    div.doc-footer-row { (subject_links(&card.links)) }
                }
                @if !tags.is_empty() {
                    div.doc-footer-row { (tag_links(&tags, None)) }
                }
            }
        }
    }
}

fn host_of(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|u| {
            u.host_str()
                .map(|h| h.trim_start_matches("www.").to_owned())
        })
        .unwrap_or_else(|| url.to_owned())
}
