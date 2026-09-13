//! From a posted form to a [`DocumentDraft`]: every field checked, every
//! problem reported beside its field, and a foreign document's unknown
//! fields carried across untouched (plan §4.2).

use std::collections::BTreeMap;
use std::fmt;

use eaten_at_atproto::at_uri::AtUri;
use eaten_at_atproto::lexicon::{ExternalUrl, Subject, SUBJECT_NSID};
use unicode_segmentation::UnicodeSegmentation;

use super::form::{EditorForm, PUBLICATION_NEW};
use super::{MAX_BODY_BYTES, MAX_LINKS, MAX_TAGS, MAX_UPLOAD_IMAGE_BYTES};
use crate::img;
use crate::publish::MAX_POST_GRAPHEMES;
use crate::tags;

/// Lexicon limits, in graphemes.
const MAX_TITLE_GRAPHEMES: usize = 500;
const MAX_DESCRIPTION_GRAPHEMES: usize = 3000;
const MAX_SUBJECT_TITLE_GRAPHEMES: usize = 200;
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

/// A cover image ready to upload.
#[derive(Clone, PartialEq, Eq)]
pub struct Cover {
    pub bytes: Vec<u8>,
    pub mime: &'static str,
}

impl fmt::Debug for Cover {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Cover")
            .field("bytes", &self.bytes.len())
            .field("mime", &self.mime)
            .finish()
    }
}

/// A validated write-up, ready to become records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentDraft {
    pub title: String,
    pub markdown: String,
    /// Only what the author typed (D19). `None` when blank.
    pub description: Option<String>,
    pub tags: Vec<String>,
    /// The subject, with any unknown fields of the original carried over.
    pub subject: Subject,
    /// A new cover. `None` keeps the existing one, if any.
    pub cover: Option<Cover>,
    pub target: Target,
    /// The text of a Bluesky post to make on publish, when the author
    /// asked for one (plan §5.7).
    pub crosspost: Option<String>,
}

/// The post text offered by default: the subject's title.
pub fn default_post_text(subject_title: &str) -> String {
    subject_title.trim().to_owned()
}

/// What validation needs to know beyond the form.
#[derive(Debug, Clone, Copy, Default)]
pub struct Context<'a> {
    /// The author's publications, by AT-URI.
    pub publications: &'a [AtUri],
    /// The subject being edited, whose unknown fields are preserved.
    pub original: Option<&'a Subject>,
}

/// Check every field. All problems are reported at once.
#[allow(clippy::too_many_lines)]
pub fn validate(form: &EditorForm, ctx: &Context<'_>) -> Result<DocumentDraft, FieldErrors> {
    let mut errors = FieldErrors::default();

    let title = form.title.trim();
    if title.is_empty() {
        errors.add("title", "Give the write-up a title.");
    } else if graphemes(title) > MAX_TITLE_GRAPHEMES {
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

    let subject_title = form.subject_title.trim();
    if subject_title.is_empty() {
        errors.add("subject_title", "Name the subject.");
    } else if graphemes(subject_title) > MAX_SUBJECT_TITLE_GRAPHEMES {
        errors.add(
            "subject_title",
            format!("Keep the subject title under {MAX_SUBJECT_TITLE_GRAPHEMES} characters."),
        );
    }

    let crosspost = form.crosspost.then(|| {
        let typed = form.post_text.trim();
        let text = if typed.is_empty() {
            default_post_text(subject_title)
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

    let mut external_urls = Vec::new();
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
                external_urls.push(ExternalUrl {
                    url,
                    service: link.service_value(),
                    label: (!label.is_empty()).then(|| label.to_owned()),
                    extra: serde_json::Map::new(),
                });
            }
            Err(message) => errors.add(format!("link_url_{i}"), message),
        }
    }
    if external_urls.len() > MAX_LINKS {
        errors.add("links", format!("At most {MAX_LINKS} links."));
    }

    let tags = match parse_tags(&form.tags) {
        Ok(tags) => tags,
        Err(message) => {
            errors.add("tags", message);
            Vec::new()
        }
    };

    let cover = match &form.cover {
        None => None,
        Some(upload) if upload.bytes.len() > MAX_UPLOAD_IMAGE_BYTES => {
            errors.add(
                "cover",
                format!(
                    "Choose an image under {} MB.",
                    MAX_UPLOAD_IMAGE_BYTES / (1024 * 1024)
                ),
            );
            None
        }
        Some(upload) => match img::cover_upload(&upload.bytes) {
            Ok((bytes, mime)) => Some(Cover { bytes, mime }),
            Err(err) => {
                tracing::debug!(%err, "cover upload rejected");
                errors.add(
                    "cover",
                    "That file isn't an image we can use. JPEG, PNG, GIF, or WebP, please.",
                );
                None
            }
        },
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

    if !errors.is_empty() {
        return Err(errors);
    }

    let mut subject = Subject {
        type_: Some(SUBJECT_NSID.to_owned()),
        title: subject_title.to_owned(),
        external_urls,
        extra: serde_json::Map::new(),
    };
    if let Some(original) = ctx.original {
        preserve_unknown_fields(&mut subject, original);
    }

    Ok(DocumentDraft {
        title: title.to_owned(),
        markdown: markdown.to_owned(),
        description: (!description.is_empty()).then(|| description.to_owned()),
        tags,
        subject,
        cover,
        target,
        crosspost,
    })
}

/// Fields our form does not know about live in the `extra` maps. Carry
/// them over from the original for the subject itself and for every
/// link that is still present, matched by URL.
fn preserve_unknown_fields(subject: &mut Subject, original: &Subject) {
    subject.extra.clone_from(&original.extra);
    for link in &mut subject.external_urls {
        if let Some(known) = original.external_urls.iter().find(|u| u.url == link.url) {
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
    use crate::editor::form::{LinkField, ServiceChoice, Upload};
    use eaten_at_atproto::lexicon::KnownService;

    fn publication() -> AtUri {
        AtUri::parse("at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1")
            .unwrap()
    }

    fn good_form() -> EditorForm {
        EditorForm {
            title: "A room with the lights off".into(),
            body: "Forty-six minutes.\n".into(),
            description: String::new(),
            subject_title: "Promises".into(),
            links: vec![
                LinkField {
                    url: "https://example.com/official".into(),
                    service: ServiceChoice::Known(KnownService::OfficialSite),
                    service_other: String::new(),
                    label: String::new(),
                },
                LinkField {
                    url: "https://example.com/buy".into(),
                    service: ServiceChoice::Other,
                    service_other: "shop".into(),
                    label: "Buy the LP".into(),
                },
                LinkField::default(),
            ],
            tags: "#notes, Short, notes, one long sit".into(),
            publication: publication().as_str().to_owned(),
            new_publication_name: String::new(),
            new_publication_url: String::new(),
            cover: None,
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
            "blank text means the subject title"
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
        assert_eq!(draft.subject.type_.as_deref(), Some(SUBJECT_NSID));
        assert_eq!(
            draft.subject.external_urls.len(),
            2,
            "blank rows are skipped"
        );
        assert_eq!(
            draft.subject.external_urls[0].service.as_deref(),
            Some("officialSite")
        );
        assert_eq!(
            draft.subject.external_urls[1].service.as_deref(),
            Some("shop")
        );
        assert_eq!(
            draft.subject.external_urls[1].label.as_deref(),
            Some("Buy the LP")
        );
        assert_eq!(draft.target, Target::Existing(publication()));
        assert!(draft.cover.is_none());
    }

    #[test]
    fn every_problem_is_reported_beside_its_field() {
        let pubs = [publication()];
        let mut form = good_form();
        form.title = "  ".into();
        form.body = String::new();
        form.subject_title = " ".into();
        form.links[0].url = "http://insecure.example/page".into();
        form.links[1].label = "x".repeat(65);
        form.tags = "a, ".to_owned() + &"y".repeat(129);
        form.cover = Some(Upload {
            file_name: "big.png".into(),
            content_type: "image/png".into(),
            bytes: vec![0; MAX_UPLOAD_IMAGE_BYTES + 1],
        });
        form.publication = "at://did:plc:other/site.standard.publication/x".into();
        let errors = validate(&form, &ctx(&pubs)).unwrap_err();
        let expected = [
            ("title", "Give the write-up a title."),
            ("body", "Write something."),
            ("subject_title", "Name the subject."),
            ("link_url_0", "Links must be https."),
            ("link_label_1", "Keep the label under 64 characters."),
            ("cover", "Choose an image under 5 MB."),
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
        let original: Subject = serde_json::from_value(serde_json::json!({
            "$type": "at.eaten.subject",
            "title": "Sample Subject",
            "externalUrls": [
                {"url": "https://example.com/loveless", "service": "bc", "label": "Elsewhere", "rank": 1},
                {"url": "https://example.com/review", "note": "long read"}
            ],
            "edition": "2021 remaster"
        }))
        .unwrap();
        let doc: eaten_at_atproto::lexicon::Document = serde_json::from_value(serde_json::json!({
            "site": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1",
            "title": "Everything at once",
            "publishedAt": "2026-08-30T12:00:00.000Z",
            "tags": ["Longform"],
            "content": {"$type": "at.markpub.markdown", "text": {"$type": "at.markpub.text", "markdown": "Loud."}}
        }))
        .unwrap();
        let form = EditorForm::from_document(&doc, &original);
        assert_eq!(form.links[0].service, ServiceChoice::Other);
        assert_eq!(form.links[0].service_other, "bc");
        assert_eq!(form.links[1].service, ServiceChoice::None);
        let pubs = [publication()];
        let draft = validate(
            &form,
            &Context {
                publications: &pubs,
                original: Some(&original),
            },
        )
        .unwrap();
        assert_eq!(draft.subject, original, "nothing was normalised or lost");
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
