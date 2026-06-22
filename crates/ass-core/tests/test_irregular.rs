//! Tests for irregular/malformed ASS fixture files.
//!
//! Each fixture exercises an edge case that should never cause a panic,
//! even if the content is not fully valid ASS.

use ass_core::SubtitleDocument;

macro_rules! assert_no_panic {
    ($path:expr, $name:expr) => {
        let content = include_str!($path);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            SubtitleDocument::parse_with_recovery(content);
        }));
        assert!(
            result.is_ok(),
            "{}: parse_with_recovery panicked (must not panic)",
            $name
        );
    };
}

// ── 1. Mixed CRLF / LF line endings ─────────────────────────────────────

#[test]
fn mixed_line_endings() {
    assert_no_panic!(
        "fixtures/irregular/mixed_line_endings.ass",
        "mixed_line_endings"
    );
}

// ── 2. UTF-8 BOM prefix ─────────────────────────────────────────────────

#[test]
fn bom_prefix() {
    assert_no_panic!("fixtures/irregular/bom.ass", "bom");
}

// ── 3. Events without [Events] section header ───────────────────────────

#[test]
fn no_section_header() {
    assert_no_panic!(
        "fixtures/irregular/no_section_header.ass",
        "no_section_header"
    );
}

// ── 4. Malformed style lines (missing fields, extra commas) ─────────────

#[test]
fn malformed_styles() {
    assert_no_panic!(
        "fixtures/irregular/malformed_styles.ass",
        "malformed_styles"
    );
}

// ── 5. Font name containing commas ──────────────────────────────────────

#[test]
fn font_name_with_commas() {
    assert_no_panic!(
        "fixtures/irregular/font_name_with_commas.ass",
        "font_name_with_commas"
    );
}

// ── 6. Empty event text / comment events ────────────────────────────────

#[test]
fn empty_events() {
    assert_no_panic!("fixtures/irregular/empty_events.ass", "empty_events");
}

// ── 7. Negative timestamps ──────────────────────────────────────────────

#[test]
fn negative_timestamps() {
    assert_no_panic!(
        "fixtures/irregular/negative_timestamps.ass",
        "negative_timestamps"
    );
}

// ── 8. Zero-duration events (start == end) ──────────────────────────────

#[test]
fn zero_duration() {
    assert_no_panic!("fixtures/irregular/zero_duration.ass", "zero_duration");
}

// ── 9. Huge / overflow-sized values ─────────────────────────────────────

#[test]
fn huge_values() {
    assert_no_panic!("fixtures/irregular/huge_values.ass", "huge_values");
}

// ── 10. Unicode CJK, emoji, special characters ──────────────────────────

#[test]
fn special_chars() {
    assert_no_panic!("fixtures/irregular/special_chars.ass", "special_chars");
}
