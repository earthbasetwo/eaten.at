//! The posted form, as strings, and the actions a JS-free form needs.

use eaten_at_atproto::lexicon::{Document, ExternalUrl, KnownService, KnownValue, Meal, Visit};

use super::MAX_LINKS;
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
    /// The Overture GERS id, as typed or as picked (plan 06).
    pub gers_id: String,
    /// The place's coordinates in microdegrees, hidden fields filled by a
    /// pick (plan 06) or carried from the record; blank when unknown.
    pub lat_e6: String,
    pub lon_e6: String,
    /// The visit date, `YYYY-MM-DD`, as the date input posts it.
    pub visited_on: String,
    pub meal: Choice<Meal>,
    /// The rating radio: blank for unrated, or `1` to `4`.
    pub rating: String,
    pub links: Vec<LinkField>,
    /// Comma-separated, as typed.
    pub tags: String,
    /// An AT-URI of one of the author's publications, or `new`.
    pub publication: String,
    pub new_publication_name: String,
    pub new_publication_url: String,
    /// Whether to post to Bluesky on publish (plan §5.7).
    pub crosspost: bool,
    /// The post's text, as typed. Blank means the default, the place's
    /// name.
    pub post_text: String,
}

/// One external link row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkField {
    pub url: String,
    pub service: Choice<KnownService>,
    pub label: String,
}

/// A select over a lexicon's `knownValues`. `None` is the blank option
/// (no value written). A value outside the list is another client's
/// vocabulary: the editor never offers one, but one already on a record
/// is preserved as [`Choice::Foreign`] so it survives an edit (D31).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice<K: KnownValue> {
    None,
    Known(K),
    Foreign(String),
}

impl<K: KnownValue> Default for Choice<K> {
    fn default() -> Self {
        Self::None
    }
}

/// The link service select, for callers that name the type.
pub type ServiceChoice = Choice<KnownService>;

/// The publication select value that reveals the new-publication fields.
pub const PUBLICATION_NEW: &str = "new";

impl<K: KnownValue> Choice<K> {
    /// The choice a posted `<select>` value stands for.
    pub fn from_value(value: &str) -> Self {
        match value.trim() {
            "" => Self::None,
            value => {
                K::from_value(value).map_or_else(|| Self::Foreign(value.to_owned()), Self::Known)
            }
        }
    }

    /// The choice for a record's optional value.
    pub fn from_record(value: Option<&str>) -> Self {
        match value {
            None => Self::None,
            Some(value) => Self::from_value(value),
        }
    }

    /// The `<option>` value.
    pub fn value(&self) -> &str {
        match self {
            Self::None => "",
            Self::Known(known) => known.as_str(),
            Self::Foreign(value) => value,
        }
    }

    /// The record value this choice writes.
    pub fn record_value(&self) -> Option<String> {
        match self {
            Self::None => None,
            Self::Known(known) => Some(known.as_str().to_owned()),
            Self::Foreign(value) => Some(value.clone()),
        }
    }

    /// Whether this is a value the editor did not offer.
    pub fn is_foreign(&self) -> bool {
        matches!(self, Self::Foreign(_))
    }
}

impl LinkField {
    /// A row prefilled from a record.
    pub fn from_external_url(link: &ExternalUrl) -> Self {
        Self {
            url: link.url.clone(),
            service: Choice::from_record(link.service.as_deref()),
            label: link.label.clone().unwrap_or_default(),
        }
    }

    /// The `service` value this row would write.
    pub fn service_value(&self) -> Option<String> {
        self.service.record_value()
    }
}

/// The repeated rows the form has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowKind {
    Link,
}

impl RowKind {
    fn name(self) -> &'static str {
        match self {
            Self::Link => "link",
        }
    }

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "link" => Some(Self::Link),
            _ => None,
        }
    }

    /// The most rows of this kind, from the lexicon's `maxLength`.
    pub fn cap(self) -> usize {
        match self {
            Self::Link => MAX_LINKS,
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

impl EditorForm {
    /// An empty form for a new write-up dated today, with one link row.
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
            gers_id: visit.place.gers_id.clone().unwrap_or_default(),
            lat_e6: visit
                .place
                .lat_e6
                .map(|v| v.value().to_string())
                .unwrap_or_default(),
            lon_e6: visit
                .place
                .lon_e6
                .map(|v| v.value().to_string())
                .unwrap_or_default(),
            visited_on: visit.visited_on.as_string(),
            meal: Choice::from_record(visit.meal.as_deref()),
            rating: visit
                .rating
                .map(|r| r.value().to_string())
                .unwrap_or_default(),
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
            crosspost: false,
            post_text: String::new(),
        };
        form.ensure_rows();
        form
    }

    /// Read the posted fields. Repeated rows are addressed by index in
    /// the field name (`link_url_2`, `link_label_0`), so order on the wire
    /// does not matter. Returns the form and the action, if a button
    /// named one.
    pub fn from_pairs<I>(pairs: I) -> (Self, Option<Action>)
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let mut form = Self::default();
        let mut action = None;
        for (name, value) in pairs {
            match name.as_str() {
                "title" => form.title = value,
                "body" => form.body = value,
                "description" => form.description = value,
                "place_name" => form.place_name = value,
                "place_address" => form.place_address = value,
                "place_price" => form.place_price = value,
                "gers_id" => form.gers_id = value,
                "lat_e6" => form.lat_e6 = value,
                "lon_e6" => form.lon_e6 = value,
                "visited_on" => form.visited_on = value,
                "meal" => form.meal = Choice::from_value(&value),
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
                            "link_label" => row(&mut form.links, index, MAX_LINKS).label = value,
                            _ => {}
                        }
                    }
                }
            }
        }
        form.ensure_rows();
        (form, action)
    }

    /// Add or remove a row. Removing never empties a list: the last row
    /// stays, cleared.
    pub fn apply(&mut self, action: &Action) {
        match action {
            Action::AddRow(kind) => match kind {
                RowKind::Link => push_within(&mut self.links, kind.cap()),
            },
            Action::RemoveRow(kind, i) => match kind {
                RowKind::Link => remove_if_present(&mut self.links, *i),
            },
            Action::Preview | Action::Publish => {}
        }
        self.ensure_rows();
    }

    fn ensure_rows(&mut self) {
        if self.links.is_empty() {
            self.links.push(LinkField::default());
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
        form.apply(&Action::RemoveRow(RowKind::Link, 0));
        assert_eq!(form.links.len(), 1, "the last row stays");
        for _ in 0..40 {
            form.apply(&Action::AddRow(RowKind::Link));
        }
        assert_eq!(form.links.len(), MAX_LINKS);
        form.apply(&Action::RemoveRow(RowKind::Link, 11));
        assert_eq!(form.links.len(), 11);
        form.apply(&Action::RemoveRow(RowKind::Link, 99));
        assert_eq!(form.links.len(), 11);
    }

    #[test]
    fn choice_values() {
        assert_eq!(ServiceChoice::from_value(""), Choice::None);
        assert_eq!(ServiceChoice::from_value("  "), Choice::None);
        assert_eq!(
            ServiceChoice::from_value("officialSite"),
            Choice::Known(KnownService::OfficialSite)
        );
        // Case matters: a near miss is another client's word, kept as is.
        assert_eq!(
            ServiceChoice::from_value("OfficialSite"),
            Choice::Foreign("OfficialSite".into())
        );
        assert_eq!(
            Choice::<Meal>::from_value("lateNight"),
            Choice::Known(Meal::LateNight)
        );
        assert_eq!(Choice::<Meal>::from_record(None), Choice::None);
        assert_eq!(
            Choice::<Meal>::from_record(Some("tea")),
            Choice::Foreign("tea".into())
        );
        let foreign = LinkField::from_external_url(&ExternalUrl {
            url: "https://x.test".into(),
            service: Some("bc".into()),
            label: None,
            extra: serde_json::Map::new(),
        });
        assert_eq!(foreign.service, Choice::Foreign("bc".into()));
        assert!(foreign.service.is_foreign());
        assert_eq!(foreign.service.value(), "bc");
        assert_eq!(foreign.service_value().as_deref(), Some("bc"));
        assert_eq!(LinkField::default().service_value(), None);
    }

    #[test]
    fn indexed_field_names() {
        assert_eq!(indexed("link_url_3"), Some(("link_url", 3)));
        assert_eq!(indexed("link_service_0"), Some(("link_service", 0)));
        assert_eq!(indexed("link_label_12"), Some(("link_label", 12)));
        assert_eq!(indexed("link_label_0"), Some(("link_label", 0)));
        assert_eq!(indexed("title"), None);
        assert_eq!(indexed("link_url_x"), None);
    }
}
