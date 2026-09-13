//! The posted form, as strings, and the actions a JS-free form needs.

use std::fmt;

use axum::extract::Multipart;
use eaten_at_atproto::lexicon::{Document, ExternalUrl, KnownService, Subject};

use super::MAX_LINKS;
use crate::model::{body_of, Body};

/// Everything the editor form carries, exactly as posted or prefilled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorForm {
    pub title: String,
    pub body: String,
    pub description: String,
    pub subject_title: String,
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
    /// The post's text, as typed. Blank means the default, the
    /// subject's title.
    pub post_text: String,
}

/// One external link row.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LinkField {
    pub url: String,
    pub service: ServiceChoice,
    /// The free-text service when [`ServiceChoice::Other`] is chosen.
    pub service_other: String,
    pub label: String,
}

/// The service select. `Unset` (never chosen) is distinct from `None`
/// (chosen: no service) so a suggestion from the URL's host fills the
/// first but never overrides the second.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ServiceChoice {
    #[default]
    Unset,
    None,
    Known(KnownService),
    Other,
}

/// The select value that stands for "no service".
pub const SERVICE_NONE: &str = "none";
/// The select value that reveals the free-text field.
pub const SERVICE_OTHER: &str = "other";
/// The publication select value that reveals the new-publication fields.
pub const PUBLICATION_NEW: &str = "new";

impl ServiceChoice {
    pub fn from_value(value: &str) -> Self {
        match value.trim() {
            "" => Self::Unset,
            SERVICE_NONE => Self::None,
            SERVICE_OTHER => Self::Other,
            known => KnownService::from_value(known).map_or(Self::Other, Self::Known),
        }
    }

    /// The `<option>` value.
    pub fn value(self) -> &'static str {
        match self {
            Self::Unset => "",
            Self::None => SERVICE_NONE,
            Self::Known(service) => service.as_str(),
            Self::Other => SERVICE_OTHER,
        }
    }
}

impl LinkField {
    /// A row prefilled from a record. A service value outside the known
    /// list shows as "other" with the value in the text field, so it
    /// survives a round trip unchanged.
    pub fn from_external_url(link: &ExternalUrl) -> Self {
        let (service, service_other) = match link.service.as_deref() {
            None => (ServiceChoice::None, String::new()),
            Some(value) => match KnownService::from_value(value) {
                Some(known) => (ServiceChoice::Known(known), String::new()),
                None => (ServiceChoice::Other, value.to_owned()),
            },
        };
        Self {
            url: link.url.clone(),
            service,
            service_other,
            label: link.label.clone().unwrap_or_default(),
        }
    }

    /// The `service` value this row would write.
    pub fn service_value(&self) -> Option<String> {
        match self.service {
            ServiceChoice::Unset | ServiceChoice::None => None,
            ServiceChoice::Known(service) => Some(service.as_str().to_owned()),
            ServiceChoice::Other => {
                let other = self.service_other.trim();
                (!other.is_empty()).then(|| other.to_owned())
            }
        }
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

/// What the submit button asked for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    AddLink,
    RemoveLink(usize),
    Preview,
    Publish,
}

impl Action {
    /// The button's `value`.
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "add_link" => Some(Self::AddLink),
            "preview" => Some(Self::Preview),
            "publish" => Some(Self::Publish),
            other => other
                .strip_prefix("remove_link:")
                .and_then(|i| i.parse().ok())
                .map(Self::RemoveLink),
        }
    }

    pub fn value(&self) -> String {
        match self {
            Self::AddLink => "add_link".to_owned(),
            Self::RemoveLink(i) => format!("remove_link:{i}"),
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
    /// An empty form for a new write-up, with one link row.
    pub fn blank(publication: &str) -> Self {
        let mut form = Self {
            publication: publication.to_owned(),
            ..Self::default()
        };
        form.ensure_rows();
        form
    }

    /// The form prefilled from an existing subject document.
    pub fn from_document(doc: &Document, subject: &Subject) -> Self {
        let body = match body_of(doc) {
            Body::Markdown(text) | Body::Plain(text) => text.to_owned(),
            Body::Empty => String::new(),
        };
        let mut form = Self {
            title: doc.title.clone(),
            body,
            description: doc.description.clone().unwrap_or_default(),
            subject_title: subject.title.clone(),
            links: subject
                .external_urls
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
    /// field name (`link_url_2`), so order on the wire
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
                "subject_title" => form.subject_title = value,
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
                            "link_url" => row(&mut form.links, index).url = value,
                            "link_service" => {
                                row(&mut form.links, index).service =
                                    ServiceChoice::from_value(&value);
                            }
                            "link_service_other" => {
                                row(&mut form.links, index).service_other = value;
                            }
                            "link_label" => row(&mut form.links, index).label = value,
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
            Action::AddLink => {
                if self.links.len() < MAX_LINKS {
                    self.links.push(LinkField::default());
                }
            }
            Action::RemoveLink(i) => {
                if *i < self.links.len() {
                    self.links.remove(*i);
                }
            }
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

/// `prefix_N` → `(prefix, N)`.
fn indexed(name: &str) -> Option<(&str, usize)> {
    let (prefix, index) = name.rsplit_once('_')?;
    index.parse().ok().map(|i| (prefix, i))
}

/// The row at `index`, growing the list (within its cap) if needed.
fn row<T: Default>(rows: &mut Vec<T>, index: usize) -> &mut T {
    let index = index.min(MAX_LINKS);
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
        for action in [Action::AddLink, Action::RemoveLink(0), Action::Preview] {
            assert_eq!(Action::parse(&action.value()), Some(action));
        }
        assert_eq!(Action::parse("remove_link:x"), None);
        assert_eq!(Action::parse("publish"), Some(Action::Publish));

        let mut form = EditorForm::blank("new");
        assert_eq!(form.links.len(), 1);
        form.apply(&Action::RemoveLink(0));
        assert_eq!(form.links.len(), 1, "the last row stays");
        for _ in 0..20 {
            form.apply(&Action::AddLink);
        }
        assert_eq!(form.links.len(), MAX_LINKS);
        form.apply(&Action::RemoveLink(11));
        assert_eq!(form.links.len(), 11);
        form.apply(&Action::RemoveLink(99));
        assert_eq!(form.links.len(), 11);
    }

    #[test]
    fn service_choice_values() {
        assert_eq!(ServiceChoice::from_value(""), ServiceChoice::Unset);
        assert_eq!(ServiceChoice::from_value("none"), ServiceChoice::None);
        assert_eq!(ServiceChoice::from_value("other"), ServiceChoice::Other);
        assert_eq!(
            ServiceChoice::from_value("officialSite"),
            ServiceChoice::Known(KnownService::OfficialSite)
        );
        assert_eq!(
            ServiceChoice::from_value("OfficialSite"),
            ServiceChoice::Other
        );
        let foreign = LinkField::from_external_url(&ExternalUrl {
            url: "https://x.test".into(),
            service: Some("bc".into()),
            label: None,
            extra: serde_json::Map::new(),
        });
        assert_eq!(foreign.service, ServiceChoice::Other);
        assert_eq!(foreign.service_other, "bc");
        assert_eq!(foreign.service_value().as_deref(), Some("bc"));
        let blank_other = LinkField {
            service: ServiceChoice::Other,
            ..LinkField::default()
        };
        assert_eq!(blank_other.service_value(), None);
    }

    #[test]
    fn indexed_field_names() {
        assert_eq!(indexed("link_url_3"), Some(("link_url", 3)));
        assert_eq!(
            indexed("link_service_other_0"),
            Some(("link_service_other", 0))
        );
        assert_eq!(indexed("title"), None);
        assert_eq!(indexed("link_url_x"), None);
    }
}
