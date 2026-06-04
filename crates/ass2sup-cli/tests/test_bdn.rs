use assert_cmd::Command;
use std::path::PathBuf;
use tempfile::TempDir;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

fn simple_fixture() -> PathBuf {
    fixtures_dir().join("simple.ass")
}

/// The BDN output is written to `<out_dir>/<stem>/`, where stem is the
/// input filename without extension.  This helper returns that subdirectory.
fn bdn_output_dir(out_dir: &std::path::Path, input: &std::path::Path) -> PathBuf {
    let stem = input
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("subtitle");
    out_dir.join(stem)
}

// ──────────────────────────────────────────────
// 1. BDN conversion: valid fixture → exit 0 + BDN.xml + PNGs
// ──────────────────────────────────────────────
#[test]
fn test_bdn_conversion() {
    let tmp = TempDir::new().unwrap();
    let out_dir = tmp.path().join("bdn_out");

    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(simple_fixture())
        .arg("--to-bdn")
        .arg("-d")
        .arg(&out_dir)
        .arg("--quiet")
        .assert()
        .success();

    let bdn_dir = bdn_output_dir(&out_dir, &simple_fixture());

    // Check BDN.xml exists
    let xml_path = bdn_dir.join("BDN.xml");
    assert!(xml_path.exists(), "BDN.xml should exist at {:?}", xml_path);

    // Check at least one PNG exists
    let png_path = bdn_dir.join("0001.png");
    assert!(png_path.exists(), "0001.png should exist at {:?}", png_path);

    // BDN.xml should start with <?xml
    let xml_content = std::fs::read_to_string(&xml_path).expect("Failed to read BDN.xml");
    assert!(
        xml_content.starts_with("<?xml"),
        "BDN.xml should start with <?xml"
    );
    assert!(
        xml_content.contains("<BDN"),
        "BDN.xml should contain <BDN element"
    );
}

// ──────────────────────────────────────────────
// 2. BDN with missing file → exit 1
// ──────────────────────────────────────────────
#[test]
fn test_bdn_missing_file() {
    let tmp = TempDir::new().unwrap();

    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg("/nonexistent/path/file.ass")
        .arg("--to-bdn")
        .arg("-d")
        .arg(tmp.path())
        .assert()
        .failure();
}

// ──────────────────────────────────────────────
// 3. BDN with custom resolution
// ──────────────────────────────────────────────
#[test]
fn test_bdn_custom_resolution() {
    let tmp = TempDir::new().unwrap();
    let out_dir = tmp.path().join("bdn_720p");

    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(simple_fixture())
        .arg("--to-bdn")
        .arg("-r")
        .arg("1280x720")
        .arg("-d")
        .arg(&out_dir)
        .arg("--quiet")
        .assert()
        .success();

    let bdn_dir = bdn_output_dir(&out_dir, &simple_fixture());
    let xml_path = bdn_dir.join("BDN.xml");
    assert!(
        xml_path.exists(),
        "BDN.xml should exist for 720p at {:?}",
        xml_path
    );
}

// ──────────────────────────────────────────────
// 4. BDN with --to-srt conflict → exit 1
// ──────────────────────────────────────────────
#[test]
fn test_bdn_conflicts_with_to_srt() {
    let tmp = TempDir::new().unwrap();

    Command::cargo_bin("ass2sup")
        .unwrap()
        .arg(simple_fixture())
        .arg("--to-bdn")
        .arg("--to-srt")
        .arg("-d")
        .arg(tmp.path())
        .assert()
        .failure();
}
