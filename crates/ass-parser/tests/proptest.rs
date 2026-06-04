use proptest::prelude::*;

// ============================================================
// Property: Timestamp roundtrip
// ============================================================
proptest! {
    #[test]
    fn timestamp_roundtrip(ms: u64) {
        let ts = ass_parser::Timestamp::from_ms(ms);
        assert_eq!(ts.as_ms(), ms);
    }
}

proptest! {
    #[test]
    fn timestamp_large_values(ms: u64) {
        let ts = ass_parser::Timestamp::from_ms(ms);
        assert_eq!(ts.as_ms(), ms);
        assert_eq!(ass_parser::Timestamp::ZERO.duration_ms(ts), ms);
    }
}

// ============================================================
// Property: ASS parse does not panic on arbitrary input
// ============================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn ass_parse_never_panics(input in "\\PC*") {
        let _result = ass_parser::AssFile::parse(&input);
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn ass_parse_lenient_never_panics(input in "\\PC*") {
        let (_ass_file, errors) = ass_parser::AssFile::parse_lenient(&input);
        let _ = errors;
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn ass_parse_lenient_returns_valid_errors(input in "\\PC{0,200}") {
        let (_ass_file, errors) = ass_parser::AssFile::parse_lenient(&input);
        assert!(errors.iter().all(|e| !std::ptr::eq(e, e)));
    }
}

// ============================================================
// Property: Timestamp comparison is consistent
// ============================================================
proptest! {
    #[test]
    fn timestamp_ordering(a: u64, b: u64) {
        let ta = ass_parser::Timestamp::from_ms(a);
        let tb = ass_parser::Timestamp::from_ms(b);
        assert_eq!(a < b, ta < tb);
        assert_eq!(a == b, ta == tb);
        assert_eq!(a > b, ta > tb);
    }
}

proptest! {
    #[test]
    fn timestamp_duration_ms_saturating(start: u64, end: u64) {
        let ts_start = ass_parser::Timestamp::from_ms(start);
        let ts_end = ass_parser::Timestamp::from_ms(end);
        let dur = ts_start.duration_ms(ts_end);
        if end >= start {
            assert_eq!(dur, end - start);
        } else {
            assert_eq!(dur, 0);
        }
    }
}

// ============================================================
// Standalone tests for zero-parameter invariants
// ============================================================
#[test]
fn timestamp_zero_is_zero() {
    let ts = ass_parser::Timestamp::from_ms(0);
    assert_eq!(ts.as_ms(), 0);
    assert_eq!(ts, ass_parser::Timestamp::ZERO);
}

#[test]
fn timestamp_from_ass_time_roundtrip() {
    let ts = ass_parser::Timestamp::from_ass_time("0:00:01.00").unwrap();
    assert_eq!(ts.as_ms(), 1000);
}

#[test]
fn timestamp_from_ass_time_invalid() {
    assert!(ass_parser::Timestamp::from_ass_time("invalid").is_err());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn ass_parse_deterministic(input in "\\PC{0,300}") {
        if let Ok(a) = ass_parser::AssFile::parse(&input) {
            let b = ass_parser::AssFile::parse(&input).expect("first parse ok, second must be too");
            assert_eq!(a, b, "AssFile derived PartialEq — same input must yield same AST");
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn ass_parse_lenient_always_returns_file(input in "\\PC{0,200}") {
        let (file, errors) = ass_parser::AssFile::parse_lenient(&input);
        let _ = file.resolution();
        let _ = file.dialogue_events().count();
        for e in &errors {
            let _ = format!("{:?}", e);
        }
    }
}

#[test]
fn srt_simple_idempotent() {
    let srt = "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n\
               2\n00:00:03,500 --> 00:00:04,500\nWorld\n";
    let a = ass_parser::AssFile::parse(srt).expect("valid SRT should parse");
    let b = ass_parser::AssFile::parse(srt).expect("re-parse same SRT");
    assert_eq!(a, b, "SRT parse must be deterministic");
}

#[test]
fn srt_empty_body() {
    let srt = "1\n00:00:01,000 --> 00:00:02,000\n\n";
    let a = ass_parser::AssFile::parse(srt).expect("SRT with empty body should parse");
    let b = ass_parser::AssFile::parse(srt).expect("re-parse");
    assert_eq!(a, b);
}

#[test]
fn srt_single_event_unicode() {
    let srt = "1\n00:00:00,000 --> 00:00:01,500\n你好世界\n";
    let a = ass_parser::AssFile::parse(srt).expect("unicode SRT");
    let b = ass_parser::AssFile::parse(srt).expect("re-parse unicode SRT");
    assert_eq!(a, b);
}

#[test]
fn srt_many_events_idempotent() {
    let mut srt = String::new();
    for i in 1..=20 {
        srt.push_str(&format!(
            "{i}\n00:00:{sec:02},000 --> 00:00:{end:02},500\nLine {i}\n\n",
            i = i,
            sec = i,
            end = i
        ));
    }
    let a = ass_parser::AssFile::parse(&srt).expect("many-event SRT");
    let b = ass_parser::AssFile::parse(&srt).expect("re-parse");
    assert_eq!(a, b);
}

#[test]
fn srt_resolution_default_when_missing() {
    let srt = "1\n00:00:00,000 --> 00:00:01,000\nHi\n";
    let file = ass_parser::AssFile::parse(srt).expect("SRT");
    let (w, h) = file.resolution();
    assert!(w > 0 && h > 0, "SRT should default to non-zero resolution, got {}x{}", w, h);
}
