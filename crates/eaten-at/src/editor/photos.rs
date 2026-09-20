//! The photos page (plan 07): what it posts, the pure reordering of a
//! visit's photos, and the markup. Every action that changes the record
//! writes it at once, so there is no staged state to lose.

use std::fmt;

use axum::extract::Multipart;
use eaten_at_atproto::lexicon::{Photo, MAX_PHOTOS};
use maud::{html, Markup};
use unicode_segmentation::UnicodeSegmentation;

use crate::img::MAX_PHOTO_UPLOAD_BYTES;

/// Most files one request may add.
pub const MAX_FILES_PER_REQUEST: usize = 12;
/// Request body cap for the photos route: the files plus the fields.
pub const MAX_REQUEST_BYTES: usize = 64 * 1024 * 1024;
/// Longest alt text, in graphemes (lexicon `maxGraphemes`).
pub const MAX_ALT_GRAPHEMES: usize = 1000;
/// Longest alt text, in bytes (lexicon `maxLength`).
pub const MAX_ALT_BYTES: usize = 2000;

/// A file from the form.
#[derive(Clone, PartialEq, Eq)]
pub struct Upload {
    pub file_name: String,
    pub bytes: Vec<u8>,
}

impl fmt::Debug for Upload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Upload")
            .field("file_name", &self.file_name)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// What the submit button asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PhotosAction {
    /// Upload the chosen files and append them.
    Add,
    /// Write the alt texts as typed.
    Save,
    Remove(usize),
    Up(usize),
    Down(usize),
    /// The photos in a new order, as the indexes of the current list:
    /// what a drag in the editor sends. Anything but a permutation of
    /// the list changes nothing.
    Order(Vec<usize>),
}

impl PhotosAction {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "add" => Some(Self::Add),
            "save" => Some(Self::Save),
            other => {
                let (kind, rest) = other.split_once(':')?;
                if kind == "order" {
                    return rest
                        .split(',')
                        .map(|i| i.trim().parse().ok())
                        .collect::<Option<Vec<usize>>>()
                        .map(Self::Order);
                }
                let index = rest.parse().ok()?;
                match kind {
                    "remove" => Some(Self::Remove(index)),
                    "up" => Some(Self::Up(index)),
                    "down" => Some(Self::Down(index)),
                    _ => None,
                }
            }
        }
    }

    pub fn value(&self) -> String {
        match self {
            Self::Add => "add".to_owned(),
            Self::Save => "save".to_owned(),
            Self::Remove(i) => format!("remove:{i}"),
            Self::Up(i) => format!("up:{i}"),
            Self::Down(i) => format!("down:{i}"),
            Self::Order(order) => format!(
                "order:{}",
                order
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        }
    }
}

/// The posted form: the alt texts by index, the files, and the action.
#[derive(Debug, Default)]
pub struct PhotosForm {
    pub alts: Vec<String>,
    pub files: Vec<Upload>,
    /// More files were chosen than one request may add; the extra ones
    /// were not read.
    pub too_many: bool,
    pub action: Option<PhotosAction>,
}

impl PhotosForm {
    /// Read a multipart body. A file over the upload limit is kept as a
    /// marker (its bytes dropped) so the page can name it.
    pub async fn from_multipart(mut multipart: Multipart) -> Result<Self, String> {
        let mut form = Self::default();
        while let Some(field) = multipart.next_field().await.map_err(|e| e.body_text())? {
            let Some(name) = field.name().map(str::to_owned) else {
                continue;
            };
            if name == "photos" {
                let file_name = field.file_name().unwrap_or_default().to_owned();
                let bytes = field.bytes().await.map_err(|e| e.body_text())?;
                if bytes.is_empty() {
                    continue;
                }
                if form.files.len() >= MAX_FILES_PER_REQUEST {
                    form.too_many = true;
                    continue;
                }
                form.files.push(Upload {
                    file_name,
                    bytes: bytes.to_vec(),
                });
                continue;
            }
            let value = field.text().await.map_err(|e| e.body_text())?;
            match name.as_str() {
                "action" => form.action = PhotosAction::parse(&value),
                other => {
                    if let Some(index) = other
                        .strip_prefix("alt_")
                        .and_then(|i| i.parse::<usize>().ok())
                    {
                        let index = index.min(MAX_PHOTOS);
                        while form.alts.len() <= index {
                            form.alts.push(String::new());
                        }
                        form.alts[index] = value;
                    }
                }
            }
        }
        Ok(form)
    }
}

/// Why the alt texts could not be taken.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AltError {
    pub index: usize,
    pub message: String,
}

/// The photos with the typed alt texts applied: trimmed, blank meaning
/// none, and bounded by the lexicon.
pub fn with_alts(mut photos: Vec<Photo>, alts: &[String]) -> Result<Vec<Photo>, AltError> {
    for (index, photo) in photos.iter_mut().enumerate() {
        let Some(alt) = alts.get(index) else {
            continue;
        };
        let alt = alt.trim();
        if alt.graphemes(true).count() > MAX_ALT_GRAPHEMES || alt.len() > MAX_ALT_BYTES {
            return Err(AltError {
                index,
                message: format!("Keep alt text under {MAX_ALT_GRAPHEMES} characters."),
            });
        }
        photo.alt = (!alt.is_empty()).then(|| alt.to_owned());
    }
    Ok(photos)
}

/// Reorder or drop one photo, or reorder them all. An index past the
/// end, or an order that is not a permutation of the list, changes
/// nothing.
pub fn rearranged(mut photos: Vec<Photo>, action: &PhotosAction) -> Vec<Photo> {
    match action {
        PhotosAction::Remove(i) if *i < photos.len() => {
            photos.remove(*i);
        }
        PhotosAction::Up(i) if *i > 0 && *i < photos.len() => photos.swap(i - 1, *i),
        PhotosAction::Down(i) if i + 1 < photos.len() => photos.swap(*i, i + 1),
        PhotosAction::Order(order) if is_permutation(order, photos.len()) => {
            photos = order.iter().map(|&i| photos[i].clone()).collect();
        }
        _ => {}
    }
    photos
}

/// Whether `order` names each of `0..len` exactly once.
fn is_permutation(order: &[usize], len: usize) -> bool {
    if order.len() != len {
        return false;
    }
    let mut seen = vec![false; len];
    for &i in order {
        if i >= len || seen[i] {
            return false;
        }
        seen[i] = true;
    }
    true
}

/// One photo as the page manages it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoRow {
    pub thumb_src: String,
    pub full_src: String,
    pub alt: String,
}

/// Everything the page needs.
#[derive(Debug)]
pub struct PhotosPage<'a> {
    pub rkey: &'a str,
    pub place_name: &'a str,
    /// Where the form posts, with the handover's query kept.
    pub action_path: &'a str,
    /// The document's own page.
    pub document_path: &'a str,
    /// Where "Skip for now" or "Done" goes after a first publish.
    pub then: Option<&'a str>,
    pub photos: &'a [PhotoRow],
    /// Problems with the files just chosen, by name.
    pub problems: &'a [String],
    /// An alt text that was refused, by index.
    pub alt_error: Option<&'a AltError>,
    /// Why the last write failed, when it did.
    pub error: Option<&'a str>,
}

/// The `<main>` content of the photos page.
pub fn page(page: &PhotosPage<'_>) -> Markup {
    let count = page.photos.len();
    let mb = MAX_PHOTO_UPLOAD_BYTES / (1024 * 1024);
    html! {
        div.page-head {
            p.kicker { "Photos" }
            h1 { "Photos of " (page.place_name) }
            @if page.then.is_some() {
                p.lede { "Published. Add photos now, or skip; you can always come back." }
            } @else {
                p.lede { "Add, caption, reorder, or remove. Every change is saved at once." }
            }
            @if let Some(message) = page.error {
                p.form-error role="alert" { (message) }
            }
        }
        form.photos-form method="post" action=(page.action_path) enctype="multipart/form-data" novalidate {
            @if count > 0 {
                ol.photo-manage aria-label="Your photos" {
                    @for (i, photo) in page.photos.iter().enumerate() {
                        @let alt_name = format!("alt_{i}");
                        @let invalid = page.alt_error.is_some_and(|e| e.index == i);
                        li.photo-row {
                            a.photo-row-image href=(photo.full_src) {
                                img src=(photo.thumb_src) alt=(photo.alt) width="120" height="120" loading="lazy";
                            }
                            div.photo-row-body {
                                div.field.field-invalid[invalid] {
                                    label.kicker for=(alt_name) { "Alt text (optional)" }
                                    input id=(alt_name) name=(alt_name) type="text" value=(photo.alt);
                                    @if invalid {
                                        @if let Some(error) = page.alt_error {
                                            p.field-error { (error.message) }
                                        }
                                    }
                                }
                                p.meta.photo-row-actions {
                                    @if i > 0 {
                                        button.link-button type="submit" name="action" value=(PhotosAction::Up(i).value()) { "Move up" }
                                    }
                                    @if i + 1 < count {
                                        button.link-button type="submit" name="action" value=(PhotosAction::Down(i).value()) { "Move down" }
                                    }
                                    button.link-button type="submit" name="action" value=(PhotosAction::Remove(i).value()) { "Remove" }
                                }
                            }
                        }
                    }
                }
            } @else {
                p.empty { "No photos yet." }
            }
            div.field {
                label.kicker for="photos" { "Add photos" }
                input #photos name="photos" type="file" accept="image/*" multiple;
            }
            p.meta.field-hint {
                "Up to " (MAX_FILES_PER_REQUEST) " at a time, " (mb) " MB each, "
                (MAX_PHOTOS) " on a visit. Photos are re-encoded and stripped of "
                "their metadata, location included, before they are uploaded."
            }
            @for problem in page.problems {
                p.field-error role="alert" { (problem) }
            }
            div.actions {
                button type="submit" name="action" value=(PhotosAction::Add.value()) { "Add photos" }
                @if count > 0 {
                    button.button-secondary type="submit" name="action" value=(PhotosAction::Save.value()) { "Save alt text" }
                }
                @match page.then {
                    Some(then) => { a.button-link href=(then) { @if count > 0 { "Done" } @else { "Skip for now" } } }
                    None => { a.button-link href=(page.document_path) { "← Back to the write-up" } }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn photo(cid: &str) -> Photo {
        serde_json::from_value(serde_json::json!({
            "image": {"$type": "blob", "ref": {"$link": cid}, "mimeType": "image/jpeg", "size": 5}
        }))
        .unwrap()
    }

    fn cids(photos: &[Photo]) -> Vec<&str> {
        photos.iter().map(|p| p.image.cid()).collect()
    }

    #[test]
    fn actions_round_trip() {
        for action in [
            PhotosAction::Add,
            PhotosAction::Save,
            PhotosAction::Remove(2),
            PhotosAction::Up(1),
            PhotosAction::Down(0),
            PhotosAction::Order(vec![2, 0, 1]),
        ] {
            assert_eq!(PhotosAction::parse(&action.value()), Some(action));
        }
        assert_eq!(PhotosAction::parse("up:x"), None);
        assert_eq!(PhotosAction::parse("sideways:1"), None);
        assert_eq!(PhotosAction::parse("publish"), None);
        assert_eq!(PhotosAction::parse("order:1,x"), None);
        assert_eq!(
            PhotosAction::parse("order: 1, 0"),
            Some(PhotosAction::Order(vec![1, 0]))
        );
    }

    #[test]
    fn rearranging_moves_or_drops_one_and_ignores_bad_indexes() {
        let three = || vec![photo("a"), photo("b"), photo("c")];
        assert_eq!(
            cids(&rearranged(three(), &PhotosAction::Remove(1))),
            ["a", "c"]
        );
        assert_eq!(
            cids(&rearranged(three(), &PhotosAction::Up(2))),
            ["a", "c", "b"]
        );
        assert_eq!(
            cids(&rearranged(three(), &PhotosAction::Down(0))),
            ["b", "a", "c"]
        );
        assert_eq!(
            cids(&rearranged(three(), &PhotosAction::Order(vec![2, 0, 1]))),
            ["c", "a", "b"]
        );
        for action in [
            PhotosAction::Up(0),
            PhotosAction::Down(2),
            PhotosAction::Remove(3),
            PhotosAction::Up(9),
            PhotosAction::Add,
            PhotosAction::Save,
            // Not permutations: a repeat, one short, one past the end.
            PhotosAction::Order(vec![0, 0, 1]),
            PhotosAction::Order(vec![1, 0]),
            PhotosAction::Order(vec![0, 1, 3]),
        ] {
            assert_eq!(
                cids(&rearranged(three(), &action)),
                ["a", "b", "c"],
                "{action:?}"
            );
        }
        assert!(rearranged(Vec::new(), &PhotosAction::Remove(0)).is_empty());
    }

    #[test]
    fn alt_texts_are_trimmed_blank_means_none_and_bounded() {
        let photos = with_alts(
            vec![photo("a"), photo("b"), photo("c")],
            &["  The room ".into(), "   ".into()],
        )
        .unwrap();
        assert_eq!(photos[0].alt.as_deref(), Some("The room"));
        assert_eq!(photos[1].alt, None);
        assert_eq!(photos[2].alt, None, "no field posted, nothing set");
        let err = with_alts(vec![photo("a")], &["x".repeat(1001)]).unwrap_err();
        assert_eq!(err.index, 0);
        assert!(err.message.contains("1000"));
    }
}
