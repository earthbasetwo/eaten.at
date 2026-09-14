//! Finding one of an author's own write-ups (plan 11): a substring test
//! over the place's name, the title, and the address, case-folded and
//! whitespace-normalized like tags. There is no index (D9); the caller
//! scans the publication's documents the way the tag page does.

use crate::model::VisitDocument;

/// Shortest query that matches anything, in characters. One character
/// would match nearly everything and is more likely a slip.
pub const MIN_QUERY_CHARS: usize = 2;

/// The query as it is compared: trimmed, lowercased, runs of whitespace
/// collapsed. Empty when too short to search for.
pub fn normalize(query: &str) -> String {
    let folded = crate::tags::normalize(query);
    if folded.chars().count() < MIN_QUERY_CHARS {
        String::new()
    } else {
        folded
    }
}

/// Whether a write-up matches a query already passed through
/// [`normalize`]. An empty query matches nothing.
pub fn matches(visit_doc: &VisitDocument, normalized: &str) -> bool {
    if normalized.is_empty() {
        return false;
    }
    let place = &visit_doc.visit.place;
    [
        Some(place.name.as_str()),
        Some(visit_doc.document().title.as_str()),
        place.address.as_deref(),
    ]
    .into_iter()
    .flatten()
    .any(|field| crate::tags::normalize(field).contains(normalized))
}

#[cfg(test)]
mod tests {
    use super::*;
    use eaten_at_atproto::lexicon::Document;
    use eaten_at_atproto::repo::Record;

    fn doc(title: &str, place: &str, address: Option<&str>) -> VisitDocument {
        let record: Record<Document> = serde_json::from_value(serde_json::json!({
            "uri": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.document/d1",
            "cid": "bafy",
            "value": {
                "site": "at://did:plc:re3ebnp5v7ffagz6rb6xfei4/site.standard.publication/pub1",
                "title": title,
                "publishedAt": "2026-09-07T12:00:00.000Z",
                "content": {
                    "$type": "at.eaten.visit",
                    "place": {"name": place, "address": address},
                    "visitedOn": "2026-09-06"
                }
            }
        }))
        .unwrap();
        VisitDocument::from_record(record).unwrap()
    }

    #[test]
    fn queries_fold_case_and_whitespace_and_need_two_characters() {
        assert_eq!(normalize("  Katz's   DELI "), "katz's deli");
        assert_eq!(normalize("k"), "");
        assert_eq!(normalize("   "), "");
        assert_eq!(normalize("東京"), "東京");
    }

    #[test]
    fn matches_place_title_or_address_and_nothing_on_an_empty_query() {
        let d = doc(
            "A long lunch",
            "Katz's Delicatessen",
            Some("205 E Houston St, New York"),
        );
        assert!(matches(&d, &normalize("katz")));
        assert!(matches(&d, &normalize("LONG  LUNCH")));
        assert!(matches(&d, &normalize("houston st")));
        assert!(!matches(&d, &normalize("noodle")));
        assert!(!matches(&d, &normalize("k")));
        assert!(!matches(&d, ""));
        let bare = doc("Untitled", "Cart", None);
        assert!(matches(&bare, &normalize("cart")));
        assert!(!matches(&bare, &normalize("street")));
    }
}
