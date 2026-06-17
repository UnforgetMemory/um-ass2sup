/// libass compatibility test suite.
///
/// Parses every .ass fixture under `fixtures/libass/` and captures strict parse
/// success, recovery warnings count, recovery errors count, and event count as
/// insta snapshots.
///
/// Guarantees:
/// - No panics on any libass-format input file.
/// - Output stability — insta detects any regression in event/warning/error count.
use std::path::Path;

use ass_parser::AssFile;

/// Path to fixtures relative to CARGO_MANIFEST_DIR (the ass-parser crate root).
const FIXTURE_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/fixtures/libass");

/// Collect all .ass files in the fixture directory, sorted for deterministic ordering.
fn collect_fixtures() -> Vec<std::path::PathBuf> {
    let dir = std::fs::read_dir(FIXTURE_DIR).expect("fixtures/libass/ directory not found");
    let mut files: Vec<std::path::PathBuf> = dir
        .filter_map(|entry| {
            let e = entry.ok()?;
            let p = e.path();
            if p.extension()?.to_str()? == "ass" {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    files.sort();
    files
}

/// Derive a stable snapshot name from a fixture file path.
fn snapshot_name(path: &Path) -> String {
    let stem = path
        .file_stem()
        .expect("file stem")
        .to_str()
        .expect("valid utf-8");
    format!("libass_compat__{stem}")
}

/// Run both strict and recovery parsing on content, return a structured summary.
fn analyze(content: &str) -> String {
    // Strict parse — may fail entirely
    let strict_ok = AssFile::parse(content).is_ok();

    // Recovery parse — always succeeds; captures warnings + errors count
    let (ass, errors) = AssFile::parse_with_recovery(content);
    let r_warnings = ass.warnings.len();
    let r_errors = errors.len();
    let r_events = ass.events.len();

    format!(
        "strict_ok: {strict_ok}\n\
         recovery_warnings: {r_warnings}\n\
         recovery_errors: {r_errors}\n\
         recovery_events: {r_events}\n"
    )
}

/// Single test that iterates ALL fixture files and snapshots their parse results.
#[test]
fn libass_compat_all() {
    let files = collect_fixtures();

    assert!(
        files.len() >= 100,
        "Expected at least 100 fixtures, found {}",
        files.len()
    );

    for path in &files {
        let content = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("failed to read {:?}: {}", path, e));

        let output = analyze(&content);
        let snap_name = snapshot_name(path);
        insta::assert_snapshot!(snap_name, output);
    }
}
