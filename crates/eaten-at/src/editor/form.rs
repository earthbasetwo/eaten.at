//! The posted form, as strings, and the actions a JS-free form needs.

use eaten_at_atproto::lexicon::{
    Document, ExternalUrl, KnownService, KnownValue, Meal, Photo, Visit, MAX_PHOTOS,
};

use super::MAX_LINKS;
use crate::model::{body_of, Body};
use crate::places::Hit;

/// Everything the editor form carries, exactly as posted or prefilled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorForm {
    /// Local draft version, echoed only after a successful write.
    pub draft_id: Option<u64>,
    pub title: String,
    pub body: String,
    pub description: String,
    pub place_name: String,
    pub place_address: String,
    /// The price band select: blank, or `1` to `4`.
    pub place_price: String,
    /// Where the place came from, and so which state the editor is in.
    pub place_mode: PlaceMode,
    /// Returning to an existing draft after choosing another restaurant.
    pub changing_place: bool,
    /// The search box on the choosing state.
    pub place_query: String,
    /// The Overture GERS id of the picked place; blank by hand.
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
    /// The photos, in the author's order, as blob references the
    /// photos island uploaded or the record carried. Strings, so the
    /// form can carry them across its own re-renders (D37 amended).
    pub photos: Vec<PhotoField>,
    /// Comma-separated, as typed.
    pub tags: String,
    /// Whether to post to Bluesky on publish (plan §5.7).
    pub crosspost: bool,
    /// The post's text, as typed. Blank means the default, the place's
    /// name.
    pub post_text: String,
}

/// How the place in the form was arrived at: still choosing one, matched
/// to an Overture listing by a search, or entered by hand (plan 06).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PlaceMode {
    #[default]
    Choosing,
    Picked,
    Manual,
}

impl PlaceMode {
    /// The hidden field's value.
    pub fn value(self) -> &'static str {
        match self {
            Self::Choosing => "choosing",
            Self::Picked => "picked",
            Self::Manual => "manual",
        }
    }

    /// The mode a posted value names; anything else means choosing.
    pub fn parse(value: &str) -> Self {
        match value.trim() {
            "picked" => Self::Picked,
            "manual" => Self::Manual,
            _ => Self::Choosing,
        }
    }
}

/// One photo, as the form carries it: the blob's reference and what
/// the author wrote about it. A row without a CID is skipped.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PhotoField {
    pub cid: String,
    pub mime: String,
    /// The blob's size in bytes, as a string.
    pub size: String,
    /// The caption: the photo's alt text.
    pub alt: String,
    /// The encoded dimensions, as strings; blank when unknown.
    pub width: String,
    pub height: String,
}

impl PhotoField {
    /// A row prefilled from a record.
    pub fn from_photo(photo: &Photo) -> Self {
        Self {
            cid: photo.image.cid().to_owned(),
            mime: photo.image.mime_type.clone(),
            size: photo.image.size.to_string(),
            alt: photo.alt.clone().unwrap_or_default(),
            width: photo
                .aspect_ratio
                .map(|r| r.width.to_string())
                .unwrap_or_default(),
            height: photo
                .aspect_ratio
                .map(|r| r.height.to_string())
                .unwrap_or_default(),
        }
    }
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
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Choice<K: KnownValue> {
    #[default]
    None,
    Known(K),
    Foreign(String),
}

/// The link service select, for callers that name the type.
pub type ServiceChoice = Choice<KnownService>;

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
    /// Take the numbered result of the cached suggestion search as the
    /// place: what a suggestion picked in the browser submits.
    Pick(usize),
    /// Take the name and address as typed, with no listing behind them.
    Manual,
    /// Back to choosing a place, keeping everything else.
    ChangePlace,
    /// Re-render the form as it is. Return pressed in a text field
    /// lands here, so nothing typed is ever sent by accident.
    Keep,
    Publish,
}

impl Action {
    /// The button's `value`.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "keep" => Some(Self::Keep),
            "publish" => Some(Self::Publish),
            "manual" => Some(Self::Manual),
            "change_place" => Some(Self::ChangePlace),
            other => {
                if let Some(kind) = other.strip_prefix("add_") {
                    return RowKind::from_name(kind).map(Self::AddRow);
                }
                if let Some(index) = other.strip_prefix("pick:") {
                    return index.parse().ok().map(Self::Pick);
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
            Self::Pick(i) => format!("pick:{i}"),
            Self::Manual => "manual".to_owned(),
            Self::ChangePlace => "change_place".to_owned(),
            Self::Keep => "keep".to_owned(),
            Self::Publish => "publish".to_owned(),
        }
    }
}

impl EditorForm {
    /// An empty form for a new write-up dated today, with one link row.
    pub fn blank() -> Self {
        let mut form = Self {
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
            place_mode: if visit.place.gers_id.is_some() {
                PlaceMode::Picked
            } else {
                PlaceMode::Manual
            },
            draft_id: None,
            changing_place: false,
            place_query: String::new(),
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
            photos: visit.photos.iter().map(PhotoField::from_photo).collect(),
            tags: doc.tags.join(", "),
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
                "place_mode" => form.place_mode = PlaceMode::parse(&value),
                "draft_id" => form.draft_id = value.parse().ok(),
                "changing_place" => form.changing_place = value == "1",
                "place_query" => form.place_query = value,
                "gers_id" => form.gers_id = value,
                "lat_e6" => form.lat_e6 = value,
                "lon_e6" => form.lon_e6 = value,
                "visited_on" => form.visited_on = value,
                "meal" => form.meal = Choice::from_value(&value),
                "rating" => form.rating = value,
                "tags" => form.tags = value,
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
                            "photo_cid" => row(&mut form.photos, index, MAX_PHOTOS).cid = value,
                            "photo_mime" => row(&mut form.photos, index, MAX_PHOTOS).mime = value,
                            "photo_size" => row(&mut form.photos, index, MAX_PHOTOS).size = value,
                            "photo_alt" => row(&mut form.photos, index, MAX_PHOTOS).alt = value,
                            "photo_width" => row(&mut form.photos, index, MAX_PHOTOS).width = value,
                            "photo_height" => {
                                row(&mut form.photos, index, MAX_PHOTOS).height = value;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        form.ensure_rows();
        (form, action)
    }

    /// Apply a structural action: add or remove a row (removing never
    /// empties a list: the last row stays, cleared), go back to
    /// choosing, or take the place by hand. Picking needs the search
    /// cache and is the route's business; keeping changes nothing.
    pub fn apply(&mut self, action: &Action) {
        match action {
            Action::AddRow(kind) => match kind {
                RowKind::Link => push_within(&mut self.links, kind.cap()),
            },
            Action::RemoveRow(kind, i) => match kind {
                RowKind::Link => remove_if_present(&mut self.links, *i),
            },
            Action::ChangePlace => self.change_place(),
            Action::Manual => self.manual(),
            Action::Pick(_) | Action::Keep | Action::Publish => {}
        }
        self.ensure_rows();
    }

    /// Back to choosing: the id and position go, the name becomes the
    /// search, and everything else stays.
    pub fn change_place(&mut self) {
        self.changing_place = true;
        self.place_mode = PlaceMode::Choosing;
        self.place_query = self.place_name.trim().to_owned();
        self.forget_listing();
    }

    /// Enter the place by hand: no listing, no id, no position.
    pub fn manual(&mut self) {
        self.place_mode = PlaceMode::Manual;
        self.forget_listing();
    }

    /// Take a search hit as the place. The name and address are the
    /// listing's (and stay editable); the website becomes the first link
    /// when that row is free and the URL is `https`, the only kind a link
    /// may be.
    pub fn pick(&mut self, hit: &Hit) {
        self.place_mode = PlaceMode::Picked;
        self.gers_id.clone_from(&hit.gers_id);
        self.place_name.clone_from(&hit.name);
        self.place_address = hit.address.clone().unwrap_or_default();
        self.lat_e6 = hit.lat_e6.to_string();
        self.lon_e6 = hit.lon_e6.to_string();
        if let Some(website) = hit.website.as_deref().filter(|w| w.starts_with("https://")) {
            let free = self.links.first().is_some_and(|l| l.url.trim().is_empty());
            let already = self.links.iter().any(|l| l.url.trim() == website);
            if free && !already {
                self.links[0] = LinkField {
                    url: website.to_owned(),
                    service: Choice::Known(KnownService::OfficialSite),
                    label: String::new(),
                };
            }
        }
    }

    fn forget_listing(&mut self) {
        self.gers_id.clear();
        self.lat_e6.clear();
        self.lon_e6.clear();
    }

    fn ensure_rows(&mut self) {
        if self.links.is_empty() {
            self.links.push(LinkField::default());
        }
        // A photo row without a blob is nothing; the island keeps the
        // indexes dense, and a gap left by hand is closed here.
        self.photos.retain(|p| !p.cid.trim().is_empty());
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
            Action::Pick(3),
            Action::Manual,
            Action::ChangePlace,
            Action::Keep,
        ] {
            assert_eq!(Action::parse(&action.value()), Some(action));
        }
        assert_eq!(Action::parse("pick:x"), None);
        assert_eq!(Action::parse("remove_link:x"), None);
        assert_eq!(Action::parse("add_thing"), None);
        assert_eq!(Action::parse("remove_thing:1"), None);
        assert_eq!(Action::parse("publish"), Some(Action::Publish));
        assert_eq!(Action::parse("search"), None, "the plain search is gone");
        assert_eq!(Action::parse("preview"), None);

        let mut form = EditorForm::blank();
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
    fn picking_changing_and_manual_entry_move_between_modes() {
        let hit = Hit {
            gers_id: "76f1250d".into(),
            name: "Devocion".into(),
            address: Some("105 York St, Brooklyn, NY 11201".into()),
            lat_e6: 40_701_607,
            lon_e6: -73_986_565,
            distance_mi: 0.9,
            category: Some("coffee shop".into()),
            website: Some("https://www.devocion.com/".into()),
        };
        let mut form = EditorForm::blank();
        assert_eq!(form.place_mode, PlaceMode::Choosing);
        form.body = "Good.".into();
        form.pick(&hit);
        assert_eq!(form.place_mode, PlaceMode::Picked);
        assert_eq!(form.gers_id, "76f1250d");
        assert_eq!(form.place_name, "Devocion");
        assert_eq!(form.place_address, "105 York St, Brooklyn, NY 11201");
        assert_eq!(
            (form.lat_e6.as_str(), form.lon_e6.as_str()),
            ("40701607", "-73986565")
        );
        assert_eq!(form.links[0].url, "https://www.devocion.com/");
        assert_eq!(
            form.links[0].service,
            Choice::Known(KnownService::OfficialSite)
        );
        assert_eq!(form.body, "Good.", "the prose is untouched");

        form.apply(&Action::ChangePlace);
        assert_eq!(form.place_mode, PlaceMode::Choosing);
        assert_eq!(form.place_query, "Devocion", "the name seeds the search");
        assert_eq!(form.gers_id, "");
        assert_eq!(form.lat_e6, "");
        assert_eq!(form.place_name, "Devocion", "kept until a pick replaces it");
        assert_eq!(form.links[0].url, "https://www.devocion.com/", "links stay");

        // A plain-http website is not offered as a link, and a taken
        // first row is left alone.
        let mut form = EditorForm::blank();
        form.pick(&Hit {
            website: Some("http://katzsdelicatessen.com/".into()),
            ..hit.clone()
        });
        assert_eq!(form.links[0].url, "");
        form.links[0].url = "https://mine.example/".into();
        form.pick(&hit);
        assert_eq!(form.links[0].url, "https://mine.example/");

        form.apply(&Action::Manual);
        assert_eq!(form.place_mode, PlaceMode::Manual);
        assert_eq!(form.gers_id, "");
        assert_eq!(PlaceMode::parse("nonsense"), PlaceMode::Choosing);
        assert_eq!(PlaceMode::parse(" picked "), PlaceMode::Picked);
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
    fn photo_rows_are_read_by_index_and_blank_ones_dropped() {
        let (form, _) = EditorForm::from_pairs([
            ("photo_cid_1".to_owned(), "bafyb".to_owned()),
            ("photo_alt_1".to_owned(), "The room".to_owned()),
            ("photo_cid_0".to_owned(), "bafya".to_owned()),
            ("photo_mime_0".to_owned(), "image/jpeg".to_owned()),
            ("photo_size_0".to_owned(), "1234".to_owned()),
            ("photo_width_0".to_owned(), "4".to_owned()),
            ("photo_height_0".to_owned(), "6".to_owned()),
            ("photo_alt_3".to_owned(), "no blob".to_owned()),
        ]);
        assert_eq!(form.photos.len(), 2, "{:?}", form.photos);
        assert_eq!(form.photos[0].cid, "bafya");
        assert_eq!(form.photos[0].size, "1234");
        assert_eq!(
            (
                form.photos[0].width.as_str(),
                form.photos[0].height.as_str()
            ),
            ("4", "6")
        );
        assert_eq!(form.photos[1].cid, "bafyb");
        assert_eq!(form.photos[1].alt, "The room");
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
