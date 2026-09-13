//! Full-document rendering pinned as snapshots.

use eaten_at_web::markdown::{excerpt, render, to_plaintext, EXCERPT_TARGET};

const WRITE_UP: &str = include_str!("fixtures/write-up.md");

#[test]
fn write_up_html() {
    insta::assert_snapshot!(render(WRITE_UP));
}

#[test]
fn write_up_plaintext() {
    insta::assert_snapshot!(to_plaintext(WRITE_UP));
}

#[test]
fn write_up_excerpt() {
    insta::assert_snapshot!(excerpt(WRITE_UP, EXCERPT_TARGET));
}
