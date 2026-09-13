//! Typed views of the records this app reads and writes.
//!
//! Every struct keeps unknown fields in an `extra` map so a record authored
//! by another client survives a read-modify-write through our editor
//! unchanged, and so schema additions never break deserialization.

mod common;
pub mod publish;
pub mod resolve;
pub mod schema;
pub mod site_standard;

pub mod at_eaten;

pub use at_eaten::{ExternalUrl, KnownService, Preferences, Subject, SUBJECT_NSID};

pub use common::{lenient_option, BlobRef, Datetime, SelfLabel, SelfLabels, StrongRef};
pub use site_standard::{
    Contributor, Document, Publication, PublicationPreferences, ThemeBasic, DOCUMENT_NSID,
    PUBLICATION_NSID,
};
