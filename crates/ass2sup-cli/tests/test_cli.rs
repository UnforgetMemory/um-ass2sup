use assert_cmd::Command;
use predicates::prelude::*;
use std::path::PathBuf;
use tempfile::TempDir;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn simple_fixture() -> PathBuf {
    fixtures_dir().join("simple.ass")
}

fn batch_fixture() -> PathBuf {
    fixtures_dir().join("batch_mode.ass")
}

// ──────────────────────────────────────────────
// 1. Simple conversion: valid fixture → exit 0
// ──────────────────────────────────────────────
#[test]
fn test_cli_simple_conversion() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("out.sup");

    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(simple_fixture())
        .arg("-o")
        .arg(&output)
        .arg("--quiet")
        .assert()
        .success();

    assert!(output.exists(), "output file should have been created");
}

// ──────────────────────────────────────────────
// 2. Missing file: nonexistent path → exit 1 + error
// ──────────────────────────────────────────────
#[test]
fn test_cli_missing_file() {
    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg("/nonexistent/path/file.ass")
        .arg("-o")
        .arg("/dev/null")
        .assert()
        .failure()
        .stdout(predicate::str::contains("Failed to read"));
}

// ──────────────────────────────────────────────
// 3. Bad resolution: -r "abc" → exit 1
// ──────────────────────────────────────────────
#[test]
fn test_cli_bad_resolution() {
    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(simple_fixture())
        .arg("-r")
        .arg("abc")
        .arg("-o")
        .arg("/dev/null")
        .assert()
        .failure();
}

// ──────────────────────────────────────────────
// 4. Validate flag: --validate → exit 0
// ──────────────────────────────────────────────
#[test]
fn test_cli_validate_flag() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("out.sup");

    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(simple_fixture())
        .arg("--validate")
        .arg("-o")
        .arg(&output)
        .arg("--quiet")
        .assert()
        .success();
}

// ──────────────────────────────────────────────
// 5. Dry run: --dry-run → exit 0, no output file
// ──────────────────────────────────────────────
#[test]
fn test_cli_dry_run() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("out.sup");

    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(simple_fixture())
        .arg("--dry-run")
        .arg("-o")
        .arg(&output)
        .arg("--quiet")
        .assert()
        .success();

    assert!(!output.exists(), "dry-run should NOT create output file");
}

// ──────────────────────────────────────────────
// 6. Force flag: --validate --force → proceeds
// ──────────────────────────────────────────────
#[test]
fn test_cli_force_flag() {
    let tmp = TempDir::new().unwrap();
    let output = tmp.path().join("out.sup");

    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(simple_fixture())
        .arg("--validate")
        .arg("--force")
        .arg("-o")
        .arg(&output)
        .arg("--quiet")
        .assert()
        .success();
}

// ──────────────────────────────────────────────
// 7. Batch mode: 2 inputs → 2 outputs
// ──────────────────────────────────────────────
#[test]
fn test_cli_batch_mode() {
    let tmp = TempDir::new().unwrap();
    let out_dir = tmp.path().join("out");
    std::fs::create_dir(&out_dir).unwrap();

    let input1 = simple_fixture();
    let input2 = batch_fixture();

    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(&input1)
        .arg(&input2)
        .arg("-d")
        .arg(&out_dir)
        .arg("--quiet")
        .assert()
        .success();

    let sup1 = out_dir.join("simple.sup");
    let sup2 = out_dir.join("batch_mode.sup");
    assert!(sup1.exists(), "first output should exist");
    assert!(sup2.exists(), "second output should exist");
}

// ──────────────────────────────────────────────
// 8. Parallel flag: -p → 2 outputs
// ──────────────────────────────────────────────
#[test]
fn test_cli_parallel_flag() {
    let tmp = TempDir::new().unwrap();
    let out_dir = tmp.path().join("out");
    std::fs::create_dir(&out_dir).unwrap();

    let input1 = simple_fixture();
    let input2 = batch_fixture();

    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(&input1)
        .arg(&input2)
        .arg("-p")
        .arg("-d")
        .arg(&out_dir)
        .arg("--quiet")
        .assert()
        .success();

    let sup1 = out_dir.join("simple.sup");
    let sup2 = out_dir.join("batch_mode.sup");
    assert!(sup1.exists(), "first output should exist");
    assert!(sup2.exists(), "second output should exist");
}

// ──────────────────────────────────────────────
// 9. SRT→SRT self-check: --to-srt roundtrips losslessly on valid SRT
// ──────────────────────────────────────────────
#[test]
fn test_cli_srt_to_srt_self_check() {
    let tmp = TempDir::new().unwrap();
    let input = fixtures_dir().join("basic.srt");
    let output = tmp.path().join("out.srt");

    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(&input)
        .arg("--to-srt")
        .arg("-o")
        .arg(&output)
        .arg("--quiet")
        .assert()
        .success();

    assert!(output.exists(), "output should have been created");
    assert!(
        output.metadata().unwrap().len() > 0,
        "output should not be empty"
    );

    // SRT parser/serializer must roundtrip losslessly
    let in_text = std::fs::read_to_string(&input).unwrap();
    let out_text = std::fs::read_to_string(&output).unwrap();
    assert_eq!(in_text, out_text, "SRT→SRT roundtrip must be lossless");
}

// ──────────────────────────────────────────────
// 10. --check on SRT input: now actually validates SRT (was silent buggy pass)
// ──────────────────────────────────────────────
#[test]
fn test_cli_check_on_srt() {
    let input = fixtures_dir().join("chinese.srt");
    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg("--check")
        .arg(&input)
        .arg("--quiet")
        .assert()
        .success();
}
