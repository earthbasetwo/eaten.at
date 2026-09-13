//! From a posted form to a [`DocumentDraft`]: every field checked, every
//! problem reported beside its field, and a foreign document's unknown
//! fields carried across untouched (plan §4.2).

use std::collections::BTreeMap;

use eaten_at_atproto::at_uri::AtUri;
use eaten_at_atproto::lexicon::{
    ExternalUrl, LatE6, LonE6, Place, PriceBand, Rating, Visit, VisitDate, PLACE_NSID, VISIT_NSID,
};
use unicode_segmentation::UnicodeSegmentation;

use super::form::{EditorForm, PlaceMode, PUBLICATION_NEW};
use super::{MAX_BODY_BYTES, MAX_LINKS, MAX_TAGS};
use crate::publish::MAX_POST_GRAPHEMES;
use crate::tags;

/// Lexicon limits, in graphemes unless named otherwise.
const MAX_TITLE_GRAPHEMES: usize = 500;
const MAX_DESCRIPTION_GRAPHEMES: usize = 3000;
const MAX_PLACE_NAME_GRAPHEMES: usize = 200;
const MAX_ADDRESS_GRAPHEMES: usize = 300;
const MAX_GERS_ID_BYTES: usize = 128;
const MAX_SERVICE_BYTES: usize = 640;
const MAX_LABEL_GRAPHEMES: usize = 64;
const MAX_TAG_GRAPHEMES: usize = 128;
const MAX_TAG_BYTES: usize = 1280;
const MAX_PUBLICATION_NAME_GRAPHEMES: usize = 500;
const MAX_URL_BYTES: usize = 2048;

/// Problems found in a form, keyed by field name (`title`,
/// `link_url_0`, …), in field order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FieldErrors(BTreeMap<String, String>);

impl FieldErrors {
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn get(&self, field: &str) -> Option<&str> {
        self.0.get(field).map(String::as_str)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.0.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    fn add(&mut self, field: impl Into<String>, message: impl Into<String>) {
        self.0.entry(field.into()).or_insert_with(|| message.into());
    }
}

/// Where the document goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// One of the author's existing publications.
    Existing(AtUri),
    /// A publication to create first.
    New { name: String, url: String },
}

/// A validated write-up, ready to become records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentDraft {
    pub title: String,
    pub markdown: String,
    /// Only what the author typed (D19). `None` when blank.
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// The visit, without its body (the markdown is beside it), with any
    /// unknown fields of the original carried over.
    pub visit: Visit,
    pub target: Target,
    /// The text of a Bluesky post to make on publish, when the author
    /// asked for one (plan §5.7).
    pub crosspost: Option<String>,
}

/// The post text offered by default: the place's name.
pub fn default_post_text(place_name: &str) -> String {
    place_name.trim().to_owned()
}

/// What validation needs to know beyond the form.
#[derive(Debug, Clone, Copy, Default)]
pub struct Context<'a> {
    /// The author's publications, by AT-URI.
    pub publications: &'a [AtUri],
    /// The visit being edited, whose unknown fields are preserved.
    pub original: Option<&'a Visit>,
}

/// Check every field. All problems are reported at once.
#[allow(clippy::too_many_lines)]
pub fn validate(form: &EditorForm, ctx: &Context<'_>) -> Result<DocumentDraft, FieldErrors> {
    let mut errors = FieldErrors::default();

    let title = form.title.trim();
    if graphemes(title) > MAX_TITLE_GRAPHEMES {
        errors.add(
            "title",
            format!("Keep the title under {MAX_TITLE_GRAPHEMES} characters."),
        );
    }

    let markdown = form.body.trim_end();
    if markdown.trim().is_empty() {
        errors.add("body", "Write something.");
    } else if markdown.len() > MAX_BODY_BYTES {
        errors.add("body", "That's longer than a write-up can be.");
    }

    let description = form.description.trim();
    if graphemes(description) > MAX_DESCRIPTION_GRAPHEMES {
        errors.add(
            "description",
            format!("Keep the excerpt under {MAX_DESCRIPTION_GRAPHEMES} characters."),
        );
    }

    let place_name = form.place_name.trim();
    if form.place_mode == PlaceMode::Choosing {
        errors.add("place_name", "Choose a place first.");
    } else if place_name.is_empty() {
        errors.add("place_name", "Name the place.");
    } else if graphemes(place_name) > MAX_PLACE_NAME_GRAPHEMES {
        errors.add(
            "place_name",
            format!("Keep the place's name under {MAX_PLACE_NAME_GRAPHEMES} characters."),
        );
    }

    let address = form.place_address.trim();
    if graphemes(address) > MAX_ADDRESS_GRAPHEMES {
        errors.add(
            "place_address",
            format!("Keep the address under {MAX_ADDRESS_GRAPHEMES} characters."),
        );
    }

    let price = match form.place_price.trim() {
        "" => None,
        raw => {
            let band = raw.parse::<u8>().ok().and_then(PriceBand::new);
            if band.is_none() {
                errors.add("place_price", "Choose a price band from the list.");
            }
            band
        }
    };

    let visited_on = match form.visited_on.trim() {
        "" => {
            errors.add("visited_on", "Give the date of the visit.");
            None
        }
        raw => {
            let date = VisitDate::parse(raw).ok();
            if date.is_none() {
                errors.add("visited_on", "Use a date like 2026-09-12.");
            }
            date
        }
    };

    let meal = form.meal.record_value();
    if meal.as_ref().is_some_and(|m| m.len() > MAX_SERVICE_BYTES) {
        errors.add("meal", "That's too long for a meal.");
    }

    let rating = match form.rating.trim() {
        "" => None,
        raw => {
            let rating = raw.parse::<u8>().ok().and_then(Rating::from_value);
            if rating.is_none() {
                errors.add("rating", "Choose a rating from the scale.");
            }
            rating
        }
    };

    let crosspost = form.crosspost.then(|| {
        let typed = form.post_text.trim();
        let text = if typed.is_empty() {
            default_post_text(place_name)
        } else {
            typed.to_owned()
        };
        if graphemes(&text) > MAX_POST_GRAPHEMES {
            errors.add(
                "post_text",
                format!("Keep the post under {MAX_POST_GRAPHEMES} characters."),
            );
        }
        text
    });

    let gers_id = form.gers_id.trim();
    if gers_id.len() > MAX_GERS_ID_BYTES {
        errors.add("gers_id", "That id is too long.");
    } else if gers_id.chars().any(char::is_whitespace) {
        errors.add("gers_id", "An id has no spaces in it.");
    }
    // Each half on its own: the lexicon has two independent fields, and
    // a foreign record with one of them must round-trip.
    let lat_e6 = match form.lat_e6.trim() {
        "" => None,
        raw => {
            let lat = raw.parse::<i32>().ok().and_then(LatE6::new);
            if lat.is_none() {
                errors.add("gers_id", "The place's position could not be read.");
            }
            lat
        }
    };
    let lon_e6 = match form.lon_e6.trim() {
        "" => None,
        raw => {
            let lon = raw.parse::<i32>().ok().and_then(LonE6::new);
            if lon.is_none() {
                errors.add("gers_id", "The place's position could not be read.");
            }
            lon
        }
    };

    let mut urls = Vec::new();
    for (i, link) in form.links.iter().enumerate() {
        let url = link.url.trim();
        if url.is_empty() {
            continue;
        }
        match checked_https_url(url) {
            Ok(url) => {
                let label = link.label.trim();
                if graphemes(label) > MAX_LABEL_GRAPHEMES {
                    errors.add(
                        format!("link_label_{i}"),
                        format!("Keep the label under {MAX_LABEL_GRAPHEMES} characters."),
                    );
                }
                let service = link.service_value();
                if service
                    .as_ref()
                    .is_some_and(|s| s.len() > MAX_SERVICE_BYTES)
                {
                    errors.add(
                        format!("link_service_{i}"),
                        "That's too long for a service.",
                    );
                }
                urls.push(ExternalUrl {
                    url,
                    service,
                    label: (!label.is_empty()).then(|| label.to_owned()),
                    extra: serde_json::Map::new(),
                });
            }
            Err(message) => errors.add(format!("link_url_{i}"), message),
        }
    }
    if urls.len() > MAX_LINKS {
        errors.add("links", format!("At most {MAX_LINKS} links."));
    }

    let tags = match parse_tags(&form.tags) {
        Ok(tags) => tags,
        Err(message) => {
            errors.add("tags", message);
            Vec::new()
        }
    };

    let target = if form.publication.trim() == PUBLICATION_NEW {
        let name = form.new_publication_name.trim();
        if name.is_empty() {
            errors.add("new_publication_name", "Name the publication.");
        } else if graphemes(name) > MAX_PUBLICATION_NAME_GRAPHEMES {
            errors.add(
                "new_publication_name",
                format!("Keep the name under {MAX_PUBLICATION_NAME_GRAPHEMES} characters."),
            );
        }
        let url = match checked_https_url(form.new_publication_url.trim()) {
            Ok(url) => url,
            Err(message) => {
                errors.add("new_publication_url", message);
                String::new()
            }
        };
        Target::New {
            name: name.to_owned(),
            url,
        }
    } else {
        match AtUri::parse(form.publication.trim()) {
            Ok(uri) if ctx.publications.contains(&uri) => Target::Existing(uri),
            _ => {
                errors.add("publication", "Choose a publication.");
                Target::New {
                    name: String::new(),
                    url: String::new(),
                }
            }
        }
    };

    // A missing date is already an error, so this only passes with one.
    let Some(visited_on) = visited_on.filter(|_| errors.is_empty()) else {
        return Err(errors);
    };

    let mut visit = Visit {
        type_: Some(VISIT_NSID.to_owned()),
        place: Place {
            type_: None,
            name: place_name.to_owned(),
            address: (!address.is_empty()).then(|| address.to_owned()),
            price,
            gers_id: (!gers_id.is_empty()).then(|| gers_id.to_owned()),
            lat_e6,
            lon_e6,
            urls,
            extra: serde_json::Map::new(),
        },
        visited_on,
        meal,
        rating,
        body: None,
        photos: Vec::new(),
        extra: serde_json::Map::new(),
    };
    if let Some(original) = ctx.original {
        preserve_unknown_fields(&mut visit, original);
    }

    // Standard requires a title; left blank, the place's name is it.
    let title = if title.is_empty() {
        place_name.to_owned()
    } else {
        title.to_owned()
    };

    Ok(DocumentDraft {
        title,
        markdown: markdown.to_owned(),
        description: (!description.is_empty()).then(|| description.to_owned()),
        tags,
        visit,
        target,
        crosspost,
    })
}

/// Fields our form does not know about live in the `extra` maps. Carry
/// them over from the original for the visit, its place, and every link
/// that is still present, matched by URL. A `$type` the original put on
/// its place stays too, and so do the photos, which have their own page
/// (plan 07).
fn preserve_unknown_fields(visit: &mut Visit, original: &Visit) {
    visit.extra.clone_from(&original.extra);
    visit.photos.clone_from(&original.photos);
    visit.place.extra.clone_from(&original.place.extra);
    visit.place.type_ = original
        .place
        .type_
        .clone()
        .filter(|t| t == PLACE_NSID || !t.is_empty());
    for link in &mut visit.place.urls {
        if let Some(known) = original.place.urls.iter().find(|u| u.url == link.url) {
            link.extra = known.extra.clone();
        }
    }
}

fn graphemes(s: &str) -> usize {
    s.graphemes(true).count()
}

/// An absolute `https` URL, normalised by the URL parser.
fn checked_https_url(raw: &str) -> Result<String, String> {
    if raw.is_empty() {
        return Err("Enter a URL.".to_owned());
    }
    if raw.len() > MAX_URL_BYTES {
        return Err("That URL is too long.".to_owned());
    }
    match url::Url::parse(raw) {
        Ok(url) if url.scheme() == "https" && url.host_str().is_some() => Ok(url.to_string()),
        Ok(url) if url.scheme() == "http" => Err("Links must be https.".to_owned()),
        _ => Err("That doesn't look like a URL.".to_owned()),
    }
}

/// Comma-separated tags: trimmed, a leading `#` dropped, duplicates
/// folded case-insensitively keeping the first spelling (D18).
pub fn parse_tags(raw: &str) -> Result<Vec<String>, String> {
    let cleaned: Vec<String> = raw
        .split(',')
        .map(|t| t.trim().trim_start_matches('#').trim().to_owned())
        .filter(|t| !t.is_empty())
        .collect();
    let tags = tags::distinct(cleaned.iter().map(String::as_str));
    if tags.len() > MAX_TAGS {
        return Err(format!("At most {MAX_TAGS} tags."));
    }
    if let Some(long) = tags
        .iter()
        .find(|t| graphemes(t) > MAX_TAG_GRAPHEMES || t.len() > MAX_TAG_BYTES)
    {
        return Err(format!(
            "“{}…” is too long for a tag.",
            long.graphemes(true).take(20).collect::<String>()
        ));
    }
    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor::form::{Choice, LinkField};
    use eaten_at_atproto::lexicon::{KnownService, Meal};

    fn publication() -> AtUri {
        AtUri::parse("at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1")
            .unwrap()
    }

    fn good_form() -> EditorForm {
        EditorForm {
            title: "A room with the lights off".into(),
            body: "Forty-six minutes.\n".into(),
            description: String::new(),
            place_name: "Promises".into(),
            place_address: " 1 Example St ".into(),
            place_price: "2".into(),
            place_mode: PlaceMode::Picked,
            place_query: String::new(),
            near_lat: String::new(),
            near_lon: String::new(),
            gers_id: " 08f2a5b6c7d8e9f0a1b2c3d4e5f60718 ".into(),
            lat_e6: "40688838".into(),
            lon_e6: "-73979914".into(),
            visited_on: "2026-09-08".into(),
            meal: Choice::Known(Meal::Dinner),
            rating: "3".into(),
            links: vec![
                LinkField {
                    url: "https://example.com/official".into(),
                    service: Choice::Known(KnownService::OfficialSite),
                    label: String::new(),
                },
                LinkField {
                    url: "https://example.com/buy".into(),
                    service: Choice::Foreign("shop".into()),
                    label: "Buy the LP".into(),
                },
                LinkField::default(),
            ],
            tags: "#notes, Short, notes, one long sit".into(),
            publication: publication().as_str().to_owned(),
            new_publication_name: String::new(),
            new_publication_url: String::new(),
            crosspost: false,
            post_text: String::new(),
        }
    }

    fn ctx(pubs: &[AtUri]) -> Context<'_> {
        Context {
            publications: pubs,
            original: None,
        }
    }

    #[test]
    fn crosspost_text_defaults_and_is_bounded() {
        let pubs = [publication()];
        let mut form = good_form();
        assert_eq!(validate(&form, &ctx(&pubs)).unwrap().crosspost, None);
        form.crosspost = true;
        let draft = validate(&form, &ctx(&pubs)).unwrap();
        assert_eq!(
            draft.crosspost.as_deref(),
            Some("Promises"),
            "blank text means the place's name"
        );
        form.post_text = "  New one from me  ".into();
        assert_eq!(
            validate(&form, &ctx(&pubs)).unwrap().crosspost.as_deref(),
            Some("New one from me")
        );
        form.post_text = "x".repeat(301);
        let errors = validate(&form, &ctx(&pubs)).unwrap_err();
        assert!(errors.get("post_text").unwrap().contains("300"));
        assert_eq!(default_post_text("  Promises "), "Promises");
    }

    #[test]
    fn a_complete_form_becomes_a_draft() {
        let pubs = [publication()];
        let draft = validate(&good_form(), &ctx(&pubs)).unwrap();
        assert_eq!(draft.title, "A room with the lights off");
        assert_eq!(draft.markdown, "Forty-six minutes.");
        assert_eq!(draft.description, None);
        assert_eq!(draft.tags, ["notes", "Short", "one long sit"]);
        let visit = &draft.visit;
        assert_eq!(visit.type_.as_deref(), Some(VISIT_NSID));
        assert_eq!(visit.place.name, "Promises");
        assert_eq!(visit.place.address.as_deref(), Some("1 Example St"));
        assert_eq!(visit.place.price, PriceBand::new(2));
        assert_eq!(visit.visited_on.as_string(), "2026-09-08");
        assert_eq!(visit.meal.as_deref(), Some("dinner"));
        assert_eq!(visit.rating, Some(Rating::StronglyRecommended));
        assert_eq!(visit.body, None);
        assert_eq!(
            visit.place.gers_id.as_deref(),
            Some("08f2a5b6c7d8e9f0a1b2c3d4e5f60718")
        );
        assert_eq!(visit.place.coordinates(), Some((40.688_838, -73.979_914)));
        assert_eq!(visit.place.urls.len(), 2, "blank rows are skipped");
        assert_eq!(visit.place.urls[0].service.as_deref(), Some("officialSite"));
        assert_eq!(visit.place.urls[1].service.as_deref(), Some("shop"));
        assert_eq!(visit.place.urls[1].label.as_deref(), Some("Buy the LP"));
        assert_eq!(draft.target, Target::Existing(publication()));
    }

    #[test]
    fn every_problem_is_reported_beside_its_field() {
        let pubs = [publication()];
        let mut form = good_form();
        form.title = "  ".into();
        form.body = String::new();
        form.place_name = " ".into();
        form.place_price = "9".into();
        form.visited_on = "12/09/2026".into();
        form.rating = "0".into();
        form.gers_id = "two words".into();
        form.links[0].url = "http://insecure.example/page".into();
        form.links[1].label = "x".repeat(65);
        form.tags = "a, ".to_owned() + &"y".repeat(129);
        form.publication = "at://did:plc:other/site.standard.publication/x".into();
        let errors = validate(&form, &ctx(&pubs)).unwrap_err();
        let expected = [
            ("body", "Write something."),
            ("place_name", "Name the place."),
            ("place_price", "Choose a price band from the list."),
            ("visited_on", "Use a date like 2026-09-12."),
            ("rating", "Choose a rating from the scale."),
            ("gers_id", "An id has no spaces in it."),
            ("link_url_0", "Links must be https."),
            ("link_label_1", "Keep the label under 64 characters."),
            ("publication", "Choose a publication."),
        ];
        for (field, message) in expected {
            assert_eq!(errors.get(field), Some(message), "{field}: {errors:?}");
        }
        assert!(
            errors.get("tags").unwrap().contains("too long"),
            "{errors:?}"
        );
        assert_eq!(errors.len(), expected.len() + 1, "{errors:?}");

        let mut form = good_form();
        form.visited_on = String::new();
        let errors = validate(&form, &ctx(&pubs)).unwrap_err();
        assert_eq!(
            errors.get("visited_on"),
            Some("Give the date of the visit.")
        );
    }

    #[test]
    fn a_form_still_choosing_a_place_cannot_be_published() {
        let pubs = [publication()];
        let mut form = good_form();
        form.place_mode = PlaceMode::Choosing;
        let errors = validate(&form, &ctx(&pubs)).unwrap_err();
        assert_eq!(errors.get("place_name"), Some("Choose a place first."));
        form.place_mode = PlaceMode::Manual;
        form.gers_id = String::new();
        let draft = validate(&form, &ctx(&pubs)).unwrap();
        assert_eq!(draft.visit.place.gers_id, None, "by hand means no id");
    }

    #[test]
    fn coordinates_are_checked_one_half_at_a_time() {
        let pubs = [publication()];
        let mut form = good_form();
        form.gers_id = String::new();
        form.lat_e6 = String::new();
        form.lon_e6 = String::new();
        let draft = validate(&form, &ctx(&pubs)).unwrap();
        assert_eq!(draft.visit.place.gers_id, None);
        assert_eq!(draft.visit.place.coordinates(), None);
        // One half alone is kept (a foreign record may have it) but is
        // not a position.
        form.lat_e6 = "40688838".into();
        let draft = validate(&form, &ctx(&pubs)).unwrap();
        assert_eq!(draft.visit.place.lat_e6.map(LatE6::value), Some(40_688_838));
        assert_eq!(draft.visit.place.coordinates(), None);
        form.lon_e6 = "180000001".into();
        let errors = validate(&form, &ctx(&pubs)).unwrap_err();
        assert_eq!(
            errors.get("gers_id"),
            Some("The place's position could not be read.")
        );
        form.lon_e6 = "x".into();
        assert!(validate(&form, &ctx(&pubs)).is_err());
    }

    #[test]
    fn a_blank_title_is_the_place_name() {
        let pubs = [publication()];
        let mut form = good_form();
        form.title = "  ".into();
        let draft = validate(&form, &ctx(&pubs)).unwrap();
        assert_eq!(draft.title, "Promises");
        form.title = "x".repeat(501);
        let errors = validate(&form, &ctx(&pubs)).unwrap_err();
        assert!(errors.get("title").unwrap().contains("500"), "{errors:?}");
        // A blank title and a blank place name are one problem, not two.
        form.title = String::new();
        form.place_name = String::new();
        let errors = validate(&form, &ctx(&pubs)).unwrap_err();
        assert_eq!(errors.get("title"), None);
        assert_eq!(errors.get("place_name"), Some("Name the place."));
    }

    #[test]
    fn a_title_that_is_the_place_name_prefills_blank() {
        let make = |title: &str| -> eaten_at_atproto::lexicon::Document {
            serde_json::from_value(serde_json::json!({
                "site": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1",
                "title": title,
                "publishedAt": "2026-08-30T12:00:00.000Z",
                "content": {"$type": "at.eaten.visit", "place": {"name": "Promises"}, "visitedOn": "2026-09-01"}
            }))
            .unwrap()
        };
        let visit: Visit = serde_json::from_value(make("x").content.clone().unwrap()).unwrap();
        assert_eq!(
            EditorForm::from_document(&make("Promises "), &visit).title,
            ""
        );
        assert_eq!(
            EditorForm::from_document(&make("Late at Promises"), &visit).title,
            "Late at Promises"
        );
    }

    #[test]
    fn new_publication_rules() {
        let pubs = [publication()];
        let mut form = good_form();
        form.publication = "new".into();
        form.new_publication_url = "ftp://x".into();
        let errors = validate(&form, &ctx(&pubs)).unwrap_err();
        assert_eq!(
            errors.get("new_publication_name"),
            Some("Name the publication.")
        );
        assert_eq!(
            errors.get("new_publication_url"),
            Some("That doesn't look like a URL.")
        );

        let mut form = good_form();
        form.publication = "new".into();
        form.new_publication_name = "Liner Notes".into();
        form.new_publication_url = "https://notes.alice.test".into();
        let draft = validate(&form, &ctx(&[])).unwrap();
        assert_eq!(
            draft.target,
            Target::New {
                name: "Liner Notes".into(),
                url: "https://notes.alice.test/".into()
            }
        );
    }

    #[test]
    fn a_foreign_document_round_trips_unchanged() {
        let original: Visit = serde_json::from_value(serde_json::json!({
            "$type": "at.eaten.visit",
            "place": {
                "$type": "at.eaten.place",
                "name": "Sample Place",
                "ids": [{"service": "yelp", "id": "sample", "verified": true}],
                "gersId": "sample-gers",
                "latE6": 1,
                "urls": [
                    {"url": "https://example.com/loveless", "service": "bc", "label": "Elsewhere", "rank": 1},
                    {"url": "https://example.com/review", "note": "long read"}
                ],
                "neighbourhood": "Old Town"
            },
            "visitedOn": "2026-09-01",
            "meal": "tea",
            "body": {"$type": "at.markpub.markdown", "text": {"$type": "at.markpub.text", "markdown": "Loud."}},
            "photos": [{"image": {"$type": "blob", "ref": {"$link": "bafyp"}, "mimeType": "image/jpeg", "size": 3}, "alt": "Loud room"}],
            "edition": "2021 remaster"
        }))
        .unwrap();
        let doc: eaten_at_atproto::lexicon::Document = serde_json::from_value(serde_json::json!({
            "site": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1",
            "title": "Everything at once",
            "publishedAt": "2026-08-30T12:00:00.000Z",
            "tags": ["Longform"],
            "content": serde_json::to_value(&original).unwrap()
        }))
        .unwrap();
        let form = EditorForm::from_document(&doc, &original);
        assert_eq!(form.place_mode, PlaceMode::Picked, "it has a gersId");
        assert_eq!(form.meal, Choice::Foreign("tea".into()));
        assert_eq!(form.gers_id, "sample-gers");
        assert_eq!(form.lat_e6, "1");
        assert_eq!(form.lon_e6, "", "half a position is carried as typed");
        assert_eq!(form.links[0].service, Choice::Foreign("bc".into()));
        assert_eq!(form.links[1].service, Choice::None);
        assert_eq!(form.rating, "");
        assert_eq!(form.place_price, "");
        let pubs = [publication()];
        let draft = validate(
            &form,
            &Context {
                publications: &pubs,
                original: Some(&original),
            },
        )
        .unwrap();
        let mut expected = original.clone();
        expected.body = None;
        assert_eq!(draft.visit, expected, "nothing was normalised or lost");
        assert_eq!(draft.title, "Everything at once");
        assert_eq!(draft.markdown, "Loud.");
        assert_eq!(draft.tags, ["Longform"]);
    }

    #[test]
    fn tag_parsing() {
        assert_eq!(
            parse_tags(" #Longform , longform, ##loud, , field  notes").unwrap(),
            ["Longform", "loud", "field  notes"]
        );
        assert_eq!(parse_tags("").unwrap(), Vec::<String>::new());
        assert!(parse_tags(
            &(0..21)
                .map(|i| format!("t{i}"))
                .collect::<Vec<_>>()
                .join(",")
        )
        .is_err());
    }

    #[test]
    fn urls_are_checked() {
        assert_eq!(
            checked_https_url("https://x.test/a?b=c").unwrap(),
            "https://x.test/a?b=c"
        );
        assert_eq!(
            checked_https_url("https://X.test").unwrap(),
            "https://x.test/"
        );
        assert!(checked_https_url("http://x.test")
            .unwrap_err()
            .contains("https"));
        assert!(checked_https_url("x.test").is_err());
        assert!(checked_https_url("").is_err());
    }
}
