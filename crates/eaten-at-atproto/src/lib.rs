//! AT Protocol plumbing for eaten.at.
//!
//! Identity resolution, repository I/O, OAuth, and lexicon publishing live
//! here. This crate knows nothing about HTML or routing.

pub mod at_uri;
pub mod http;
pub mod identity;
pub mod lexicon;
pub mod oauth;
pub mod repo;

/// The NSID authority every lexicon this project publishes shares.
///
/// Flat NSIDs (`at.eaten.<name>`) mean one DNS record and one repo
/// carry every schema; see the plan, §4.5.
pub const NSID_AUTHORITY: &str = "at.eaten";

/// Lexicon JSON bundled with the crate, keyed by NSID.
///
/// These are the source of truth for the schemas we publish; the publish
/// tool (Phase 2) writes them into the `eaten.at` repo unchanged.
pub const LEXICONS: &[(&str, &str)] = &[
    (
        "at.eaten.subject",
        include_str!("../../../lexicons/at.eaten.subject.json"),
    ),
    (
        "at.eaten.preferences",
        include_str!("../../../lexicons/at.eaten.preferences.json"),
    ),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_bundled_lexicon_is_under_our_authority() {
        for (nsid, _) in LEXICONS {
            assert!(
                nsid.starts_with(&format!("{NSID_AUTHORITY}.")),
                "{nsid} is not under {NSID_AUTHORITY}"
            );
        }
    }

    #[test]
    fn bundled_lexicon_ids_match_their_keys() {
        for (nsid, json) in LEXICONS {
            let needle = format!("\"id\": \"{nsid}\"");
            assert!(
                json.contains(&needle),
                "{nsid} JSON does not declare its own id"
            );
        }
    }
}
