//! The editor (plan §7.5): a form that works with JavaScript off and
//! produces a validated [`DocumentDraft`] without touching the network.
//!
//! Three layers, each pure:
//!
//! - [`form`]: what the browser posted, exactly as strings, plus the
//!   structural actions (add or remove a row) that a JS-free form needs.
//! - [`draft`]: validation of a form into a [`DocumentDraft`], with one
//!   error per field, and the merge that keeps a foreign document's
//!   unknown fields intact.
//! - [`view`]: the markup, with errors rendered beside their fields.
//!
//! Writing the draft to the repository is the next chunk's job.

pub mod draft;
pub mod form;
pub mod view;

pub use draft::{default_post_text, validate, Context, DocumentDraft, FieldErrors, Target};
pub use form::{
    Action, Choice, EditorForm, IdField, LinkField, RowKind, ServiceChoice, PUBLICATION_NEW,
};

/// Most external links on one place (lexicon `maxLength`).
pub const MAX_LINKS: usize = 12;
/// Most external ids on one place (lexicon `maxLength`).
pub const MAX_IDS: usize = 8;
/// Most tags on one document. The lexicon sets no cap; this keeps the
/// listing's chip row sane.
pub const MAX_TAGS: usize = 20;
/// Request body cap for the editor routes: the write-up plus every
/// other field, URL-encoded, with room to spare.
pub const MAX_REQUEST_BYTES: usize = 1024 * 1024;
/// Longest write-up, in bytes. Well past anything reasonable, well short
/// of what would trouble a PDS.
pub const MAX_BODY_BYTES: usize = 200 * 1024;
