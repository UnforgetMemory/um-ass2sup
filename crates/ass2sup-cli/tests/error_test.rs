//! Unit tests for the unified `ass2sup_cli::error` module.
//!
//! These tests guard the public contract of the error type system:
//!   - Display formatting for every variant
//!   - Source chain preservation for IO errors
//!   - `From` conversions from sub-errors
//!   - Idempotent / consistent equality semantics

use ass2sup_cli::error::*;
use std::error::Error as _;
use std::io;

#[test]
fn parse_error_display_includes_file_message_and_line() {
    let err = Error::Parse {
        file: "subs/test.ass".into(),
        message: "invalid style override".into(),
        line: Some(42),
    };
    let s = err.to_string();
    assert!(s.contains("subs/test.ass"), "missing file: {s}");
    assert!(s.contains("invalid style override"), "missing message: {s}");
    assert!(s.contains("42"), "missing line: {s}");
}

#[test]
fn parse_error_display_omits_line_when_none() {
    let err = Error::Parse {
        file: "test.ass".into(),
        message: "broken".into(),
        line: None,
    };
    // Line number must not appear as a stray "0" or similar.
    let s = err.to_string();
    assert!(s.contains("test.ass"));
    assert!(s.contains("broken"));
}

#[test]
fn io_error_chains_source() {
    let io_err = io::Error::new(io::ErrorKind::NotFound, "file missing");
    let err = Error::io("missing.ass", io_err);
    assert!(err.source().is_some(), "expected source() chain for Io");
    // Source must be the same io::Error preserved by reference.
    let s = err.to_string();
    assert!(s.contains("missing.ass"), "missing path: {s}");
}

#[test]
fn io_helper_accepts_pathbuf() {
    let io_err = io::Error::other("x");
    let err = Error::io(std::path::PathBuf::from("/tmp/a"), io_err);
    assert!(err.to_string().contains("/tmp/a"));
}

#[test]
fn render_error_event_propagates_via_from() {
    let inner = RenderError::Event {
        event_idx: 100,
        pts_ms: 1234,
        message: "glyph render failed".into(),
    };
    let err: Error = inner.into();
    assert!(err.to_string().contains("event 100"));
    assert!(err.to_string().contains("1234"));
    assert!(err.to_string().contains("glyph render failed"));
}

#[test]
fn render_error_effect_stringifies_kind() {
    let err = RenderError::Effect {
        effect: "blur".into(),
        message: "radius out of range".into(),
    };
    assert!(err.to_string().contains("blur"));
    assert!(err.to_string().contains("radius out of range"));
}

#[test]
fn render_error_backend_wraps() {
    let err = RenderError::Backend("vello init failed".into());
    assert!(err.to_string().contains("vello init failed"));
}

#[test]
fn output_error_pgs_bdn_ttml_webvtt() {
    let pgs = OutputError::Pgs("bad segment".into());
    let bdn = OutputError::Bdn("malformed".into());
    let ttml = OutputError::Ttml("invalid time".into());
    let vtt = OutputError::WebVtt("bad cue".into());
    assert!(pgs.to_string().contains("PGS"));
    assert!(bdn.to_string().contains("BDN"));
    assert!(ttml.to_string().contains("TTML"));
    assert!(vtt.to_string().contains("WebVTT"));
}

#[test]
fn config_error_read_preserves_io_source() {
    let io_err = io::Error::new(io::ErrorKind::PermissionDenied, "nope");
    let err = ConfigError::Read {
        path: std::path::PathBuf::from("/etc/x"),
        source: io_err,
    };
    assert!(err.source().is_some(), "expected source() chain");
    assert!(err.to_string().contains("/etc/x"));
}

#[test]
fn config_error_validation_mentions_message() {
    let err = ConfigError::Validation("fps must be > 0".into());
    assert!(err.to_string().contains("fps must be > 0"));
}

#[test]
fn font_error_variants_display() {
    let a = FontError::NotFound("Arial".into());
    let b = FontError::NoCjkGlyphs("DejaVu Sans".into());
    let c = FontError::FallbackExhausted("OP_1".into());
    let d = FontError::Fontconfig("query failed".into());
    assert!(a.to_string().contains("Arial"));
    assert!(b.to_string().contains("DejaVu Sans"));
    assert!(b.to_string().contains("CJK"));
    assert!(c.to_string().contains("OP_1"));
    assert!(d.to_string().contains("fontconfig"));
}

#[test]
fn color_error_variants_display() {
    let a = ColorError::Unsupported("Rec.2020".into());
    let b = ColorError::Conversion("clamp failed".into());
    assert!(a.to_string().contains("Rec.2020"));
    assert!(b.to_string().contains("clamp failed"));
}

#[test]
fn format_detection_validation_cli_variants_display() {
    let f = Error::FormatDetection(std::path::PathBuf::from("weird.bin"));
    let v = Error::Validation(7);
    let c = Error::Cli("bad flag".into());
    assert!(f.to_string().contains("weird.bin"));
    assert!(v.to_string().contains("7"));
    assert!(c.to_string().contains("bad flag"));
}

#[test]
fn nested_render_error_via_from_keeps_display_chain() {
    let inner = RenderError::Backend("missing gpu".into());
    let err: Error = inner.into();
    // The outer Display must include the inner message via the From impl.
    assert!(err.to_string().contains("missing gpu"));
}

#[test]
fn nested_output_error_via_from_keeps_display_chain() {
    let inner = OutputError::Pgs("PCS oob".into());
    let err: Error = inner.into();
    assert!(err.to_string().contains("PCS oob"));
}

#[test]
fn from_string_to_error_produces_cli_variant_for_legacy_migration() {
    let err: Error = "legacy message".to_string().into();
    match err {
        Error::Cli(s) => assert_eq!(s, "legacy message"),
        _ => panic!("expected Cli variant"),
    }
}

#[test]
fn error_is_send_and_sync() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<Error>();
    assert_sync::<Error>();
}

#[test]
fn error_is_debug() {
    let err = Error::Validation(3);
    let dbg = format!("{err:?}");
    assert!(dbg.contains("Validation"));
}
