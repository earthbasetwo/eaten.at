//! Site-route paths (plan §5.6). DIDs are unencoded: `:` is a legal path
//! character. Record keys are validated to a URL-safe alphabet.

use eaten_at_atproto::identity::{Did, Handle};
use eaten_at_web::layout::urlencoding;

pub fn repo(did: &Did) -> String {
    format!("/at/{did}/")
}

pub fn publication(did: &Did, pub_rkey: &str) -> String {
    format!("/at/{did}/{pub_rkey}/")
}

pub fn document(did: &Did, pub_rkey: &str, doc_rkey: &str) -> String {
    format!("/at/{did}/{pub_rkey}/{doc_rkey}")
}

pub fn tagged(did: &Did, pub_rkey: &str, tag: &str) -> String {
    format!("/at/{did}/{pub_rkey}/tagged/{}", urlencoding(tag))
}

pub fn feed(did: &Did, pub_rkey: &str) -> String {
    format!("/at/{did}/{pub_rkey}/feed.xml")
}

pub fn handle_lookup(handle: &Handle) -> String {
    format!("/@{handle}")
}

/// The cover-art proxy for a document, card size.
pub fn cover(did: &Did, doc_rkey: &str) -> String {
    format!("/img/{did}/{doc_rkey}")
}

/// The cover-art proxy for a document, OpenGraph size.
pub fn cover_og(did: &Did, doc_rkey: &str) -> String {
    format!("/img/{did}/{doc_rkey}?size=og")
}

/// The publication-icon proxy.
pub fn icon(did: &Did, pub_rkey: &str) -> String {
    format!("/img/{did}/{pub_rkey}?kind=icon")
}
