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

/// The cover-art proxy for a document, card size, at the revision
/// `version` (the record's CID). The cover can change while the path
/// does not, so the revision is in the URL: a cache holding the old
/// cover holds it under the old URL (see `routes::image::cover`).
pub fn cover(did: &Did, doc_rkey: &str, version: &str) -> String {
    format!("/img/{did}/{doc_rkey}?v={}", urlencoding(version))
}

/// The cover-art proxy for a document, OpenGraph size, at the revision
/// `version`, as [`cover`].
pub fn cover_og(did: &Did, doc_rkey: &str, version: &str) -> String {
    format!("/img/{did}/{doc_rkey}?size=og&v={}", urlencoding(version))
}

/// One of a document's photos through the proxy, at `thumb` or `full`.
pub fn photo(did: &Did, doc_rkey: &str, cid: &str, size: &str) -> String {
    format!("/img/{did}/{doc_rkey}/{cid}?size={size}")
}

/// One of the signed-in author's own blobs, for the editor's tiles
/// before and after the record carries them, at `thumb` or `full`.
pub fn own_photo(cid: &str, size: &str) -> String {
    format!("/write/photo/{cid}?size={size}")
}

/// The upload the editor's photos island posts files to.
pub const UPLOAD: &str = "/write/upload";

/// The publication-icon proxy, at the publication record's `version`
/// (its CID), as [`cover`]: the path stays while the icon changes.
pub fn icon(did: &Did, pub_rkey: &str, version: &str) -> String {
    format!("/img/{did}/{pub_rkey}?kind=icon&v={}", urlencoding(version))
}
