//! The posted form, as strings, and the actions a JS-free form needs.

use std::fmt;

use axum::extract::Multipart;
use eaten_at_atproto::lexicon::{
    Document, ExternalId, ExternalUrl, KnownIdService, KnownService, KnownValue, Meal, Visit,
};

use super::{MAX_IDS, MAX_LINKS};
use crate::model::{body_of, Body};

/// Everything the editor form carries, exactly as posted or prefilled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorForm {
    pub title: String,
    pub body: String,
    pub description: String,
    pub place_name: String,
    pub place_address: String,
    /// The price band select: blank, or `1` to `4`.
    pub place_price: String,
    /// The visit date, `YYYY-MM-DD`, as the date input posts it.
    pub visited_on: String,
    pub meal: Choice<Meal>,
    /// The free-text meal when [`Choice::Other`] is chosen.
    pub meal_other: String,
    /// The rating radio: blank for unrated, or `1` to `4`.
    pub rating: String,
    pub ids: Vec<IdField>,
    pub links: Vec<LinkField>,
    /// Comma-separated, as typed.
    pub tags: String,
    /// An AT-URI of one of the author's publications, or `new`.
    pub publication: String,
    pub new_publication_name: String,
    pub new_publication_url: String,
    /// A file chosen in this submission. `None` keeps whatever the
    /// document already has.
    pub cover: Option<Upload>,
    /// Whether to post to Bluesky on publish (plan §5.7).
    pub crosspost: bool,
    /// The post's text, as typed. Blank means the default, the place's
    /// name.
    pub post_text: String,
}

/// One external id row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IdField {
    pub service: Choice<KnownIdService>,
    /// The free-text service when [`Choice::Other`] is chosen.
    pub service_other: String,
    pub id: String,
}

/// One external link row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkField {
    pub url: String,
    pub service: Choice<KnownService>,
    /// The free-text service when [`Choice::Other`] is chosen.
    pub service_other: String,
    pub label: String,
}

/// A select over a lexicon's `knownValues`. `Unset` (never chosen) is
/// distinct from `None` (chosen: no value) so a suggestion can fill the
/// first but never override the second.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Choice<K: KnownValue> {
    Unset,
    None,
    Known(K),
    Other,
}

impl<K: KnownValue> Default for Choice<K> {
    fn default() -> Self {
        Self::Unset
    }
}

/// The link service select, for callers that name the type.
pub type ServiceChoice = Choice<KnownService>;

/// The select value that stands for "no value".
pub const SERVICE_NONE: &str = "none";
/// The select value that reveals the free-text field.
pub const SERVICE_OTHER: &str = "other";
/// The publication select value that reveals the new-publication fields.
pub const PUBLICATION_NEW: &str = "new";

impl<K: KnownValue> Choice<K> {
    pub fn from_value(value: &str) -> Self {
        match value.trim() {
            "" => Self::Unset,
            SERVICE_NONE => Self::None,
            SERVICE_OTHER => Self::Other,
            known => K::from_value(known).map_or(Self::Other, Self::Known),
        }
    }

    /// A choice prefilled from a record's optional value: absent is
    /// `None`, a known value is `Known`, anything else is `Other` with
    /// the text returned beside it so it survives a round trip.
    pub fn from_record(value: Option<&str>) -> (Self, String) {
        match value {
            None => (Self::None, String::new()),
            Some(value) => match K::from_value(value) {
                Some(known) => (Self::Known(known), String::new()),
                None => (Self::Other, value.to_owned()),
            },
        }
    }

    /// The `<option>` value.
    pub fn value(self) -> &'static str {
        match self {
            Self::Unset => "",
            Self::None => SERVICE_NONE,
            Self::Known(known) => known.as_str(),
            Self::Other => SERVICE_OTHER,
        }
    }

    /// The record value this choice writes, given the free-text field
    /// that goes with `Other`.
    pub fn record_value(self, other: &str) -> Option<String> {
        match self {
            Self::Unset | Self::None => None,
            Self::Known(known) => Some(known.as_str().to_owned()),
            Self::Other => {
                let other = other.trim();
                (!other.is_empty()).then(|| other.to_owned())
            }
        }
    }
}

impl LinkField {
    /// A row prefilled from a record.
    pub fn from_external_url(link: &ExternalUrl) -> Self {
        let (service, service_other) = Choice::from_record(link.service.as_deref());
        Self {
            url: link.url.clone(),
            service,
            service_other,
            label: link.label.clone().unwrap_or_default(),
        }
    }

    /// The `service` value this row would write.
    pub fn service_value(&self) -> Option<String> {
        self.service.record_value(&self.service_other)
    }
}

impl IdField {
    /// A row prefilled from a record.
    pub fn from_external_id(id: &ExternalId) -> Self {
        let (service, service_other) = Choice::from_record(Some(id.service.as_str()));
        Self {
            service,
            service_other,
            id: id.id.clone(),
        }
    }

    /// The `service` value this row would write.
    pub fn service_value(&self) -> Option<String> {
        self.service.record_value(&self.service_other)
    }

    /// Whether the row is empty and can be skipped.
    pub fn is_blank(&self) -> bool {
        self.id.trim().is_empty() && self.service_value().is_none()
    }
}

/// A file from the form.
#[derive(Clone, PartialEq, Eq)]
pub struct Upload {
    pub file_name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

impl fmt::Debug for Upload {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Upload")
            .field("file_name", &self.file_name)
            .field("content_type", &self.content_type)
            .field("bytes", &self.bytes.len())
            .finish()
    }
}

/// The repeated rows the form has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Link,
    Id,
}

impl RowKind {
    fn name(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Id => "id",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "link" => Some(Self::Link),
            "id" => Some(Self::Id),
            _ => None,
        }
    }

    /// The most rows of this kind, from the lexicon's `maxLength`.
    pub fn cap(self) -> usize {
        match self {
            Self::Link => MAX_LINKS,
            Self::Id => MAX_IDS,
        }
    }
}

/// What the submit button asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    AddRow(RowKind),
    RemoveRow(RowKind, usize),
    Preview,
    Publish,
}

impl Action {
    /// The button's `value`.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "preview" => Some(Self::Preview),
            "publish" => Some(Self::Publish),
            other => {
                if let Some(kind) = other.strip_prefix("add_") {
                    return RowKind::from_name(kind).map(Self::AddRow);
                }
                let (kind, index) = other.strip_prefix("remove_")?.split_once(':')?;
                Some(Self::RemoveRow(
                    RowKind::from_name(kind)?,
                    index.parse().ok()?,
                ))
            }
        }
    }

    pub fn value(&self) -> String {
        match self {
            Self::AddRow(kind) => format!("add_{}", kind.name()),
            Self::RemoveRow(kind, i) => format!("remove_{}:{i}", kind.name()),
            Self::Preview => "preview".to_owned(),
            Self::Publish => "publish".to_owned(),
        }
    }
}

/// Why a posted body could not be read as the editor form.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct FormReadError(pub String);

impl EditorForm {
    /// An empty form for a new write-up dated today, with one row of
    /// each kind.
    pub fn blank(publication: &str) -> Self {
        let mut form = Self {
            publication: publication.to_owned(),
            visited_on: eaten_at_atproto::lexicon::VisitDate::today().as_string(),
            ..Self::default()
        };
        form.ensure_rows();
        form
    }

    /// The form prefilled from an existing visit document.
    pub fn from_document(doc: &Document, visit: &Visit) -> Self {
        let body = match body_of(doc) {
            Body::Markdown(text) | Body::Plain(text) => text.to_owned(),
            Body::Empty => String::new(),
        };
        let (meal, meal_other) = match visit.meal.as_deref() {
            None => (Choice::Unset, String::new()),
            some => Choice::from_record(some),
        };
        // A title that is only the place's name is the default, not a
        // choice: the field shows blank and the placeholder stands in.
        let title = if doc.title.trim() == visit.place.name.trim() {
            String::new()
        } else {
            doc.title.clone()
        };
        let mut form = Self {
            title,
            body,
            description: doc.description.clone().unwrap_or_default(),
            place_name: visit.place.name.clone(),
            place_address: visit.place.address.clone().unwrap_or_default(),
            place_price: visit
                .place
                .price
                .map(|p| p.value().to_string())
                .unwrap_or_default(),
            visited_on: visit.visited_on.as_string(),
            meal,
            meal_other,
            rating: visit
                .rating
                .map(|r| r.value().to_string())
                .unwrap_or_default(),
            ids: visit
                .place
                .ids
                .iter()
                .map(IdField::from_external_id)
                .collect(),
            links: visit
                .place
                .urls
                .iter()
                .map(LinkField::from_external_url)
                .collect(),
            tags: doc.tags.join(", "),
            publication: doc.site.clone(),
            new_publication_name: String::new(),
            new_publication_url: String::new(),
            cover: None,
            crosspost: false,
            post_text: String::new(),
        };
        form.ensure_rows();
        form
    }

    /// Read a multipart body. Repeated rows are addressed by index in the
    /// field name (`link_url_2`, `id_value_0`), so order on the wire
    /// does not matter. Returns the form and the action, if a button
    /// named one.
    pub async fn from_multipart(
        mut multipart: Multipart,
    ) -> Result<(Self, Option<Action>), FormReadError> {
        let mut form = Self::default();
        let mut action = None;
        while let Some(field) = multipart
            .next_field()
            .await
            .map_err(|e| FormReadError(e.body_text()))?
        {
            let Some(name) = field.name().map(str::to_owned) else {
                continue;
            };
            if name == "cover" {
                let file_name = field.file_name().unwrap_or_default().to_owned();
                let content_type = field.content_type().unwrap_or_default().to_owned();
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| FormReadError(e.body_text()))?;
                if !bytes.is_empty() {
                    form.cover = Some(Upload {
                        file_name,
                        content_type,
                        bytes: bytes.to_vec(),
                    });
                }
                continue;
            }
            let value = field
                .text()
                .await
                .map_err(|e| FormReadError(e.body_text()))?;
            match name.as_str() {
                "title" => form.title = value,
                "body" => form.body = value,
                "description" => form.description = value,
                "place_name" => form.place_name = value,
                "place_address" => form.place_address = value,
                "place_price" => form.place_price = value,
                "visited_on" => form.visited_on = value,
                "meal" => form.meal = Choice::from_value(&value),
                "meal_other" => form.meal_other = value,
                "rating" => form.rating = value,
                "tags" => form.tags = value,
                "publication" => form.publication = value,
                "new_publication_name" => form.new_publication_name = value,
                "new_publication_url" => form.new_publication_url = value,
                "crosspost" => form.crosspost = matches!(value.trim(), "1" | "on" | "true"),
                "post_text" => form.post_text = value,
                "action" => action = Action::parse(&value),
                other => {
                    if let Some((prefix, index)) = indexed(other) {
                        match prefix {
                            "link_url" => row(&mut form.links, index, MAX_LINKS).url = value,
                            "link_service" => {
                                row(&mut form.links, index, MAX_LINKS).service =
                                    Choice::from_value(&value);
                            }
                            "link_service_other" => {
                                row(&mut form.links, index, MAX_LINKS).service_other = value;
                            }
                            "link_label" => row(&mut form.links, index, MAX_LINKS).label = value,
                            "id_service" => {
                                row(&mut form.ids, index, MAX_IDS).service =
                                    Choice::from_value(&value);
                            }
                            "id_service_other" => {
                                row(&mut form.ids, index, MAX_IDS).service_other = value;
                            }
                            "id_value" => row(&mut form.ids, index, MAX_IDS).id = value,
                            _ => {}
                        }
                    }
                }
            }
        }
        form.ensure_rows();
        Ok((form, action))
    }

    /// Add or remove a row. Removing never empties a list: the last row
    /// stays, cleared.
    pub fn apply(&mut self, action: &Action) {
        match action {
            Action::AddRow(kind) => match kind {
                RowKind::Link => push_within(&mut self.links, kind.cap()),
                RowKind::Id => push_within(&mut self.ids, kind.cap()),
            },
            Action::RemoveRow(kind, i) => match kind {
                RowKind::Link => remove_if_present(&mut self.links, *i),
                RowKind::Id => remove_if_present(&mut self.ids, *i),
            },
            Action::Preview | Action::Publish => {}
        }
        self.ensure_rows();
    }

    fn ensure_rows(&mut self) {
        if self.links.is_empty() {
            self.links.push(LinkField::default());
        }
        if self.ids.is_empty() {
            self.ids.push(IdField::default());
        }
    }
}

fn push_within<T: Default>(rows: &mut Vec<T>, cap: usize) {
    if rows.len() < cap {
        rows.push(T::default());
    }
}

fn remove_if_present<T>(rows: &mut Vec<T>, index: usize) {
    if index < rows.len() {
        rows.remove(index);
    }
}

/// `prefix_N` → `(prefix, N)`.
fn indexed(name: &str) -> Option<(&str, usize)> {
    let (prefix, index) = name.rsplit_once('_')?;
    index.parse().ok().map(|i| (prefix, i))
}

/// The row at `index`, growing the list (within its cap) if needed.
fn row<T: Default>(rows: &mut Vec<T>, index: usize, cap: usize) -> &mut T {
    let index = index.min(cap);
    while rows.len() <= index {
        rows.push(T::default());
    }
    &mut rows[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn actions_round_trip_and_rows_never_empty() {
        for action in [
            Action::AddRow(RowKind::Link),
            Action::RemoveRow(RowKind::Link, 0),
            Action::AddRow(RowKind::Id),
            Action::RemoveRow(RowKind::Id, 1),
            Action::Preview,
        ] {
            assert_eq!(Action::parse(&action.value()), Some(action));
        }
        assert_eq!(Action::parse("remove_link:x"), None);
        assert_eq!(Action::parse("add_thing"), None);
        assert_eq!(Action::parse("remove_thing:1"), None);
        assert_eq!(Action::parse("publish"), Some(Action::Publish));

        let mut form = EditorForm::blank("new");
        assert_eq!(form.visited_on.len(), 10, "dated today");
        assert_eq!(form.links.len(), 1);
        assert_eq!(form.ids.len(), 1);
        form.apply(&Action::RemoveRow(RowKind::Link, 0));
        assert_eq!(form.links.len(), 1, "the last row stays");
        for _ in 0..40 {
            form.apply(&Action::AddRow(RowKind::Link));
            form.apply(&Action::AddRow(RowKind::Id));
        }
        assert_eq!(form.links.len(), MAX_LINKS);
        assert_eq!(form.ids.len(), MAX_IDS);
        form.apply(&Action::RemoveRow(RowKind::Link, 11));
        assert_eq!(form.links.len(), 11);
        form.apply(&Action::RemoveRow(RowKind::Link, 99));
        assert_eq!(form.links.len(), 11);
        form.apply(&Action::RemoveRow(RowKind::Id, 0));
        assert_eq!(form.ids.len(), MAX_IDS - 1);
    }

    #[test]
    fn choice_values() {
        assert_eq!(ServiceChoice::from_value(""), Choice::Unset);
        assert_eq!(ServiceChoice::from_value("none"), Choice::None);
        assert_eq!(ServiceChoice::from_value("other"), Choice::Other);
        assert_eq!(
            ServiceChoice::from_value("officialSite"),
            Choice::Known(KnownService::OfficialSite)
        );
        assert_eq!(ServiceChoice::from_value("OfficialSite"), Choice::Other);
        assert_eq!(
            Choice::<Meal>::from_value("lateNight"),
            Choice::Known(Meal::LateNight)
        );
        let foreign = LinkField::from_external_url(&ExternalUrl {
            url: "https://x.test".into(),
            service: Some("bc".into()),
            label: None,
            extra: serde_json::Map::new(),
        });
        assert_eq!(foreign.service, Choice::Other);
        assert_eq!(foreign.service_other, "bc");
        assert_eq!(foreign.service_value().as_deref(), Some("bc"));
        let blank_other = LinkField {
            service: Choice::Other,
            ..LinkField::default()
        };
        assert_eq!(blank_other.service_value(), None);
        let id = IdField::from_external_id(&ExternalId {
            service: "googlePlace".into(),
            id: "g1".into(),
            extra: serde_json::Map::new(),
        });
        assert_eq!(id.service, Choice::Known(KnownIdService::GooglePlace));
        assert!(!id.is_blank());
        assert!(IdField::default().is_blank());
    }

    #[test]
    fn indexed_field_names() {
        assert_eq!(indexed("link_url_3"), Some(("link_url", 3)));
        assert_eq!(
            indexed("link_service_other_0"),
            Some(("link_service_other", 0))
        );
        assert_eq!(indexed("id_value_12"), Some(("id_value", 12)));
        assert_eq!(indexed("id_value_0"), Some(("id_value", 0)));
        assert_eq!(indexed("title"), None);
        assert_eq!(indexed("link_url_x"), None);
    }
}
