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
