//! Tests for CLI error types.
//!
//! Verifies that all [`CliError`] variants produce sensible Display
//! messages with the expected formatting and content.

use ass2sup_cli::error::CliError;

// ---------------------------------------------------------------------------
// Individual variant tests
// ---------------------------------------------------------------------------

#[test]
fn test_cli_error_invalid_resolution() {
    let err = CliError::InvalidResolution {
        input: "abc".into(),
        message: "Expected WIDTHxHEIGHT".into(),
    };
    let msg = err.to_string();
    assert!(msg.contains("abc"));
    assert!(msg.contains("WIDTHxHEIGHT"));
}

#[test]
fn test_cli_error_input_too_large() {
    let err = CliError::InputTooLarge {
        path: "big.mkv".into(),
        size: 200_000_000,
        max: 100_000_000,
    };
    let msg = err.to_string();
    assert!(msg.contains("200000000"));
    assert!(msg.contains("100000000"));
}

#[test]
fn test_cli_error_conversion() {
    let err = CliError::Conversion("render failed".into());
    assert!(err.to_string().contains("render failed"));
}

#[test]
fn test_cli_error_no_input_files() {
    let err = CliError::NoInputFiles;
    assert_eq!(
        err.to_string(),
        "No input files found. Provide positional args or use --glob."
    );
}

#[test]
fn test_cli_error_read_error() {
    let err = CliError::ReadError("missing.srt".into(), "No such file".into());
    let msg = err.to_string();
    assert!(msg.contains("missing.srt"));
    assert!(msg.contains("No such file"));
}

#[test]
fn test_cli_error_parse_error() {
    let err = CliError::ParseError("bad.ass".into(), "line 42".into());
    let msg = err.to_string();
    assert!(msg.contains("bad.ass"));
    assert!(msg.contains("line 42"));
}

#[test]
fn test_cli_error_create_dir_error() {
    let err = CliError::CreateDirError("/out".into(), "permission denied".into());
    let msg = err.to_string();
    assert!(msg.contains("/out"));
    assert!(msg.contains("permission denied"));
}

#[test]
fn test_cli_error_batch_failed() {
    let err = CliError::BatchFailed {
        successes: 5,
        failures: 2,
    };
    let msg = err.to_string();
    assert!(msg.contains("5"));
    assert!(msg.contains("succeeded"));
    assert!(msg.contains("2"));
    assert!(msg.contains("failed"));
}

// ---------------------------------------------------------------------------
// Trait implementations
// ---------------------------------------------------------------------------

#[test]
fn test_cli_error_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<CliError>();
}

#[test]
fn test_cli_error_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<CliError>();
}

#[test]
fn test_cli_error_implements_std_error() {
    fn assert_error<T: std::error::Error>() {}
    assert_error::<CliError>();
}
