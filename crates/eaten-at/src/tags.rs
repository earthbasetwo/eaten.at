//! Tag matching. Tags are free text stored as the author typed them
//! (plan D18); matching for the tag route is case-insensitive and
//! whitespace-normalized, and the display form comes from the document.

/// Canonical form for comparison: trimmed, lowercased, internal runs of
/// whitespace collapsed to one space.
pub fn normalize(tag: &str) -> String {
    tag.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Whether two tags are the same tag.
pub fn matches(a: &str, b: &str) -> bool {
    let (a, b) = (normalize(a), normalize(b));
    !a.is_empty() && a == b
}

/// Distinct tags from a set of documents, in first-seen order, keeping
/// the first spelling seen for each.
pub fn distinct<'a>(tags: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for tag in tags {
        let key = normalize(tag);
        if key.is_empty() || !seen.insert(key) {
            continue;
        }
        out.push(tag.trim().to_owned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_case_and_whitespace() {
        assert_eq!(normalize("  Field  Notes \n"), "field notes");
        assert_eq!(normalize("Longform"), "longform");
        assert_eq!(normalize(""), "");
    }

    #[test]
    fn matching_table() {
        assert!(matches("Longform", "longform"));
        assert!(matches("field notes", "field  notes"));
        assert!(matches("Played This All Summer", "played this all summer"));
        assert!(!matches("#longform", "longform"));
        assert!(!matches("", ""));
        assert!(!matches("longform", "long form"));
    }

    #[test]
    fn distinct_keeps_first_spelling() {
        let tags = [
            "Longform",
            "longform",
            " Field Notes",
            "field  notes",
            "",
            "long read",
        ];
        assert_eq!(
            distinct(tags.into_iter()),
            vec!["Longform", "Field Notes", "long read"]
        );
    }
}
