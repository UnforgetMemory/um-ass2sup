use assert_cmd::Command;
use std::path::PathBuf;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn simple_fixture() -> PathBuf {
    fixtures_dir().join("simple.ass")
}

#[test]
fn snapshot_help() {
    let output = Command::cargo_bin("ass2sup")
        .unwrap()
        .arg("--help")
        .output()
        .expect("run --help");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    insta::assert_snapshot!("help", stdout);
}

#[test]
fn snapshot_short_help() {
    let output = Command::cargo_bin("ass2sup")
        .unwrap()
        .arg("-h")
        .output()
        .expect("run -h");
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).expect("utf-8 stdout");
    insta::assert_snapshot!("short_help", stdout);
}

#[test]
fn snapshot_missing_file_error() {
    let output = Command::cargo_bin("ass2sup")
        .unwrap()
        .arg("/nonexistent/path/to/file.ass")
        .arg("-o")
        .arg("/dev/null")
        .output()
        .expect("run with missing file");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    insta::assert_snapshot!("missing_file_error", stderr.into_owned());
}

#[test]
fn snapshot_validate_clean() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output_path = tmp.path().join("out.sup");
    let output = Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(simple_fixture())
        .arg("--validate")
        .arg("-o")
        .arg(&output_path)
        .arg("--quiet")
        .output()
        .expect("run --validate");
    let combined = format!(
        "exit={}\nstdout=\n{}\nstderr=\n{}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    insta::assert_snapshot!("validate_clean", combined);
}

#[test]
fn snapshot_dry_run_no_output() {
    let tmp = tempfile::TempDir::new().unwrap();
    let output_path = tmp.path().join("out.sup");
    let output = Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(simple_fixture())
        .arg("--dry-run")
        .arg("-o")
        .arg(&output_path)
        .arg("--quiet")
        .output()
        .expect("run --dry-run");
    assert!(!output_path.exists(), "dry-run must not create output file");
    let combined = format!(
        "exit={}\nstdout=\n{}\nstderr=\n{}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    insta::assert_snapshot!("dry_run_no_output", combined);
}
