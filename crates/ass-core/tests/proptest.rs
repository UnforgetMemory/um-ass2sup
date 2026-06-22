//! Property-based tests for ass-core.
//!
//! These tests verify invariants that must hold for ALL valid inputs:
//! - Determinism: same input always produces same output
//! - No panics: parser never crashes on arbitrary input
//! - Timestamp round-trip: ms → string → ms preserves value

use ass_core::time::Timestamp;
use proptest::prelude::*;

proptest! {
    // ── Timestamp invariants ──

    #[test]
    fn timestamp_from_ms_roundtrip(ms in 0u64..86_400_000u64) {
        // 24-hour range covers all realistic subtitle durations
        let ts = Timestamp::from_ms(ms);
        let ass_str = ts.as_ass_time();
        let parsed = Timestamp::from_ass_time(&ass_str).unwrap();
        // ASS format uses centiseconds (10ms resolution),
        // so we expect at most 10ms deviation
        let diff = (parsed.as_ms() as i64 - ms as i64).abs();
        prop_assert!(diff <= 10, "ms={ms}, parsed={}, diff={diff}", parsed.as_ms());
    }

    #[test]
    fn timestamp_from_srt_roundtrip(ms in 0u64..86_400_000u64) {
        let ts = Timestamp::from_ms(ms);
        let srt_str = ts.as_srt_time();
        let parsed = Timestamp::from_srt_timecode(&srt_str).unwrap();
        prop_assert_eq!(parsed, ts, "SRT roundtrip failed: ms={}", ms);
    }

    #[test]
    fn timestamp_ordering(ms1 in 0u64..86_400_000u64, ms2 in 0u64..86_400_000u64) {
        let t1 = Timestamp::from_ms(ms1);
        let t2 = Timestamp::from_ms(ms2);
        prop_assert_eq!(t1 < t2, ms1 < ms2);
        prop_assert_eq!(t1.cmp(&t2), ms1.cmp(&ms2));
    }

    #[test]
    fn timestamp_duration_saturating(ms1 in 0u64..86_400_000u64, ms2 in 0u64..86_400_000u64) {
        let t1 = Timestamp::from_ms(ms1);
        let t2 = Timestamp::from_ms(ms2);
        let dur = t1.duration_ms(t2);
        if ms2 >= ms1 {
            prop_assert_eq!(dur, ms2 - ms1);
        } else {
            prop_assert_eq!(dur, 0);
        }
    }

    // ── Parse determinism ──

    #[test]
    fn ass_parse_deterministic(content in ".*") {
        let (doc1, _) = ass_core::SubtitleDocument::parse_with_recovery(&content);
        let (doc2, _) = ass_core::SubtitleDocument::parse_with_recovery(&content);
        prop_assert_eq!(doc1, doc2, "parse must be deterministic");
    }
}

/// Quick-check: parser never panics on arbitrary input.
#[test]
fn ass_parse_never_panics() {
    use ass_core::SubtitleDocument;
    use proptest::test_runner::{Config, TestRunner};

    let mut runner = TestRunner::new(Config {
        cases: 500,
        ..Config::default()
    });
    let result = runner.run(
        &".*".prop_filter("skip_empty", |s| !s.is_empty()),
        |content| {
            let _ = SubtitleDocument::parse_with_recovery(&content);
            Ok(())
        },
    );
    assert!(result.is_ok(), "parser panicked on some input");
}

/// Quick-check: lenient parse always returns a valid document.
#[test]
fn ass_parse_lenient_always_returns_doc() {
    use ass_core::SubtitleDocument;
    use proptest::test_runner::{Config, TestRunner};

    let mut runner = TestRunner::new(Config {
        cases: 200,
        ..Config::default()
    });
    let result = runner.run(
        &"(.|\n){0,50}".prop_filter("non_empty", |s| !s.is_empty()),
        |content| {
            let (doc, _) = SubtitleDocument::parse_with_recovery(&content);
            // Document should always have valid format set
            prop_assert!(
                matches!(
                    doc.format,
                    ass_core::SubtitleFormat::Ass | ass_core::SubtitleFormat::Srt
                ),
                "format should always be set"
            );
            Ok(())
        },
    );
    assert!(result.is_ok(), "lenient parse returned invalid doc");
}

/// Serialization: round-trip SRT content through parse → to_srt.
#[test]
fn srt_roundtrip_preserves_content() {
    let input = "1\n00:00:01,000 --> 00:00:05,000\nHello World\n\n2\n00:00:06,000 --> 00:00:10,000\nLine two\n";
    let doc = ass_core::srt::parse_srt(input).unwrap();
    let output = ass_core::srt::to_srt(&doc);
    assert_eq!(input, output, "SRT round-trip failed");
}
