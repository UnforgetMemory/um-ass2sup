//! Tests for malformed ASS files: syntax errors, unclosed brackets,
//! out-of-order events, bad timecodes, and other real-world edge cases.
//!
//! These tests verify that the parser handles garbage input gracefully.
//! No panic is acceptable; best-effort recovery is expected.

use ass_core::SubtitleDocument;

fn parse_fixture(name: &str) -> (SubtitleDocument, Vec<ass_core::ParseError>) {
    let path = format!("tests/fixtures/malformed/{name}.ass");
    let content = std::fs::read_to_string(&path).expect("fixture file not found");
    SubtitleDocument::parse_with_recovery(&content)
}

#[test]
fn unclosed_brackets_no_panic() {
    let (doc, errors) = parse_fixture("unclosed_brackets");
    // Must parse without panic; should recover all 4 events despite syntax errors
    assert_eq!(doc.events.len(), 4, "should recover all 4 events");
    // At least some events should have their text preserved
    assert!(doc.events[0].text_raw.contains("Unclosed bold"));
    assert!(doc.events[1].text_raw.contains("Unclosed pos"));
    // Extra closing brace should not break parsing
    assert!(doc.events[3].text_raw.contains("Extra closing brace"));
    eprintln!(
        "unclosed_brackets: {} errors, {} warnings",
        errors.len(),
        doc.warnings.len()
    );
}

#[test]
fn unclosed_transform_no_panic() {
    let (doc, errors) = parse_fixture("unclosed_transform");
    // Should handle missing commas, unclosed parens, empty t(), bare \t
    assert_eq!(doc.events.len(), 4, "should recover all 4 events");
    // Empty \t() should not crash
    assert!(doc.events[2].text_raw.contains("Empty t"));
    // Bare \t with no args should not crash
    assert!(doc.events[3].text_raw.contains("T with no args"));
    eprintln!("unclosed_transform: {} errors", errors.len());
}

#[test]
fn syntax_errors_no_panic() {
    let (doc, errors) = parse_fixture("syntax_errors");
    assert_eq!(doc.events.len(), 4, "should recover all 4 events");
    // Doubled backslash should still work or degrade gracefully
    // Mixed case tags should not crash
    assert!(doc.events[1].text_raw.contains("Mixed case"));
    // Nested override blocks should not crash
    assert!(doc.events[2].text_raw.contains("Nested override"));
    // Unknown tags should be silently skipped
    eprintln!("syntax_errors: {} errors", errors.len());
}

#[test]
fn out_of_order_no_panic() {
    let (doc, errors) = parse_fixture("out_of_order");
    // All 4 events should be parsed regardless of ordering
    assert_eq!(
        doc.events.len(),
        4,
        "should parse all 4 out-of-order events"
    );
    // The events should preserve their declaration order (not sorted)
    assert!(doc.events[0].text_raw.contains("Late start first"));
    assert!(doc.events[1].text_raw.contains("Early start second"));
    assert!(doc.events[2].text_raw.contains("Middle event"));
    assert!(doc.events[3].text_raw.contains("Earliest start last"));
    // Timestamps should be correct regardless of order
    assert_eq!(doc.events[0].start_ms, 10000, "first event start=10s");
    assert_eq!(doc.events[1].start_ms, 1000, "second event start=1s");
    eprintln!(
        "out_of_order: {} errors, {} warnings",
        errors.len(),
        doc.warnings.len()
    );
}

#[test]
fn bad_timecodes_no_panic() {
    let (doc, errors) = parse_fixture("bad_timecodes");
    // Some events have bad timecodes and should be skipped
    // Normal event, bad start, bad end, end<start, impossible time
    assert!(!doc.events.is_empty(), "at least normal event parsed");
    // The normal event should have correct start/end
    if let Some(event) = doc.events.first() {
        assert_eq!(event.start_ms, 1000);
    }
    // Events with bad timestamps may be in errors list
    eprintln!(
        "bad_timecodes: {} events, {} errors, {} warnings",
        doc.events.len(),
        errors.len(),
        doc.warnings.len()
    );
}

#[test]
fn missing_style_no_panic() {
    let (doc, errors) = parse_fixture("missing_style");
    assert_eq!(doc.events.len(), 3, "all 3 events should parse");
    assert_eq!(doc.events[0].style.as_str(), "NonExistentStyle");
    assert_eq!(doc.events[1].style.as_str(), "Default");
    assert_eq!(doc.events[2].style.as_str(), "AnotherMissing");
    eprintln!(
        "missing_style: {} errors, {} warnings",
        errors.len(),
        doc.warnings.len()
    );
}

#[test]
fn missing_playres_defaults() {
    let (doc, errors) = parse_fixture("missing_playres");
    assert_eq!(doc.events.len(), 1);
    assert_eq!(doc.metadata.play_res_x, 1920, "default PlayResX=1920");
    assert_eq!(doc.metadata.play_res_y, 1080, "default PlayResY=1080");
    eprintln!(
        "missing_playres: {} errors, PlayRes={}x{}",
        errors.len(),
        doc.metadata.play_res_x,
        doc.metadata.play_res_y
    );
}

#[test]
fn malformed_v4plus_style_fields() {
    let (doc, errors) = parse_fixture("malformed_v4plus");
    assert_eq!(doc.events.len(), 1, "event should still parse");
    eprintln!(
        "malformed_v4plus: {} styles, {} events, {} errors",
        doc.styles.len(),
        doc.events.len(),
        errors.len()
    );
}

#[test]
fn no_format_line_no_panic() {
    let (doc, errors) = parse_fixture("no_format_line");
    assert_eq!(doc.events.len(), 1, "event should parse");
    assert!(doc.events[0].text_raw.contains("No format line"));
    eprintln!(
        "no_format_line: {} styles, {} errors",
        doc.styles.len(),
        errors.len()
    );
}

#[test]
fn empty_script_info_no_panic() {
    let (doc, errors) = parse_fixture("empty_script_info");
    assert_eq!(doc.metadata.play_res_x, 1920, "default PlayResX");
    assert_eq!(doc.metadata.play_res_y, 1080, "default PlayResY");
    assert_eq!(doc.events.len(), 1, "event should parse");
    eprintln!("empty_script_info: {} errors", errors.len());
}
