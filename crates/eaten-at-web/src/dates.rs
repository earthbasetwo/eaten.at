//! Readable dates from the ISO forms records carry.
//!
//! Records hold `publishedAt` as an RFC 3339 timestamp; partial dates
//! as `YYYY`, `YYYY-MM`, or `YYYY-MM-DD` are read too. Readers see "July
//! 28, 2026", "July 2026", or "2026"; anything else is shown as written.

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];

/// Format an ISO date or timestamp for display. Unparseable input is
/// returned trimmed, never dropped.
pub fn human_date(value: &str) -> String {
    let value = value.trim();
    let date = value.split('T').next().unwrap_or(value);
    let mut parts = date.splitn(3, '-');
    let year = parts.next().filter(|y| y.len() == 4 && all_digits(y));
    let month = parts
        .next()
        .filter(|m| m.len() == 2 && all_digits(m))
        .and_then(|m| m.parse::<usize>().ok())
        .filter(|m| (1..=12).contains(m));
    let day = parts
        .next()
        .filter(|d| d.len() == 2 && all_digits(d))
        .and_then(|d| d.parse::<u8>().ok())
        .filter(|d| (1..=31).contains(d));
    let segments = date.matches('-').count();
    match (year, month, day, segments) {
        (Some(year), None, None, 0) => year.to_owned(),
        (Some(year), Some(month), None, 1) => format!("{} {year}", MONTHS[month - 1]),
        (Some(year), Some(month), Some(day), 2) => {
            format!("{} {day}, {year}", MONTHS[month - 1])
        }
        _ => value.to_owned(),
    }
}

fn all_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::human_date;

    #[test]
    fn full_partial_and_timestamp_forms() {
        assert_eq!(human_date("2026-07-28"), "July 28, 2026");
        assert_eq!(human_date("2026-07-08"), "July 8, 2026");
        assert_eq!(human_date("2026-07"), "July 2026");
        assert_eq!(human_date("2026"), "2026");
        assert_eq!(human_date("2026-09-07T21:00:00.000Z"), "September 7, 2026");
        assert_eq!(human_date(" 1997-05-21 "), "May 21, 1997");
    }

    #[test]
    fn malformed_input_is_shown_as_written() {
        for raw in [
            "",
            "late 90s",
            "2026-13",
            "2026-00-10",
            "2026-7-4",
            "2026-07-32",
            "20261",
            "2026-07-28-1",
        ] {
            assert_eq!(human_date(raw), raw.trim(), "{raw:?}");
        }
    }
}
