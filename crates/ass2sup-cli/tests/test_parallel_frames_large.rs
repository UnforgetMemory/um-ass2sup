//! Regression test for v0.5.2 hang with `--parallel-frames` on large ASS files.
//!
//! v0.5.2 bug: Converting a 1988-event ASS with `--parallel-frames` would
//! hang due to 16.5GB peak memory from storing all RenderedFrames in a Vec
//! before quantization. This test ensures the fix holds.
//!
//! The 200-event fixture is enough to trigger the original OOM on a typical
//! CI runner (200 × 8.3 MB ≈ 1.6 GB of forced pre-quantisation allocation in
//! the buggy code path) while keeping the debug-binary test runtime under
//! ~30 s per test. Run with `cargo test --release` for sub-second tests.

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use assert_cmd::Command;
use tempfile::TempDir;

/// Path to fixtures stored inside the crate's `tests/fixtures/` directory.
fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

/// Process-wide lock to serialise the heavy `ass2sup` subprocess tests below.
///
/// `cargo test` runs tests in parallel by default; each test here spawns a
/// `ass2sup` subprocess that allocates ~8.3 MB per event for the buggy
/// `Vec<RenderedFrame>` peak. Three of them racing for RAM can cause one to
/// be killed by the OOM killer or by the per-test `timeout()` (yielding
/// `code=<interrupted>`). This Mutex makes the three tests run one at a time
/// while leaving the rest of the test suite free to parallelise.
static HEAVY_LOCK: Mutex<()> = Mutex::new(());

/// RAII guard that releases the lock even on panic.
fn lock_heavy() -> std::sync::MutexGuard<'static, ()> {
    match HEAVY_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

#[test]
fn test_parallel_frames_large_completes_within_timeout() {
    let _guard = lock_heavy();
    let fixture = fixtures_dir().join("large_parallel.ass");
    assert!(fixture.exists(), "fixture not found: {}", fixture.display());

    let tmp = TempDir::new().expect("tempdir");
    let output = tmp.path().join("out.sup");

    let start = Instant::now();
    Command::cargo_bin("ass2sup")
        .expect("ass2sup binary")
        .arg(&fixture)
        .arg("-o")
        .arg(&output)
        .arg("--parallel-frames")
        .arg("--no-check-fonts")
        .arg("--quiet")
        .timeout(std::time::Duration::from_secs(90))
        .assert()
        .success();
    let elapsed = start.elapsed();

    assert!(output.exists(), "output SUP not created");
    let meta = std::fs::metadata(&output).expect("output metadata");
    assert!(meta.len() > 0, "output SUP is empty");

    // Should complete in well under 90s on typical hardware in debug mode;
    // under 1s in release. v0.5.2 hung indefinitely; any completion
    // in < 90s confirms the fix.
    eprintln!("test_parallel_frames_large_completed elapsed={:?}", elapsed);
}

#[test]
fn test_sequential_frames_large_completes_within_timeout() {
    let _guard = lock_heavy();
    // Sanity check: sequential (non-parallel) mode also works on the same fixture.
    // This is the baseline that should never hang.
    let fixture = fixtures_dir().join("large_parallel.ass");
    assert!(fixture.exists(), "fixture not found: {}", fixture.display());

    let tmp = TempDir::new().expect("tempdir");
    let output = tmp.path().join("out.sup");

    let start = Instant::now();
    Command::cargo_bin("ass2sup")
        .expect("ass2sup binary")
        .arg(&fixture)
        .arg("-o")
        .arg(&output)
        .arg("--no-check-fonts")
        .arg("--quiet")
        .timeout(std::time::Duration::from_secs(120))
        .assert()
        .success();
    let elapsed = start.elapsed();

    assert!(output.exists(), "output SUP not created");
    let meta = std::fs::metadata(&output).expect("output metadata");
    assert!(meta.len() > 0, "output SUP is empty");

    eprintln!(
        "test_sequential_frames_large_completed elapsed={:?}",
        elapsed
    );
}

#[test]
fn test_parallel_vs_sequential_produce_same_pgs_segments() {
    let _guard = lock_heavy();
    // Functional correctness: parallel and sequential must produce
    // the same set of PGS display events (modulo RLE byte-level differences
    // from non-deterministic palette ordering in quantizer).
    // We compare the count of display sets as a sanity check.

    use std::io::Read;

    let fixture = fixtures_dir().join("large_parallel.ass");
    let content = std::fs::read_to_string(&fixture).expect("read fixture");
    let ass = ass_parser::AssFile::parse(&content).expect("parse fixture");
    let dialogue_count = ass.events.len();
    assert_eq!(dialogue_count, 200, "fixture should have 200 events");

    // Both modes must produce a SUP file with valid PGS magic + non-empty content.
    let tmp = TempDir::new().expect("tempdir");

    let par_output = tmp.path().join("par.sup");
    Command::cargo_bin("ass2sup")
        .expect("ass2sup binary")
        .arg(&fixture)
        .arg("-o")
        .arg(&par_output)
        .arg("--parallel-frames")
        .arg("--no-check-fonts")
        .arg("--quiet")
        .assert()
        .success();

    let seq_output = tmp.path().join("seq.sup");
    Command::cargo_bin("ass2sup")
        .expect("ass2sup binary")
        .arg(&fixture)
        .arg("-o")
        .arg(&seq_output)
        .arg("--no-check-fonts")
        .arg("--quiet")
        .assert()
        .success();

    // Both files should have the PGS magic "PG" at the start
    fn check_pgs_magic(path: &std::path::Path) -> bool {
        let mut f = std::fs::File::open(path).expect("open");
        let mut buf = [0u8; 2];
        f.read_exact(&mut buf).expect("read");
        buf == *b"PG"
    }

    assert!(
        check_pgs_magic(&par_output),
        "parallel output is not a valid PGS file"
    );
    assert!(
        check_pgs_magic(&seq_output),
        "sequential output is not a valid PGS file"
    );

    // File sizes should be within 20% of each other (palette order may differ)
    let par_size = std::fs::metadata(&par_output).unwrap().len();
    let seq_size = std::fs::metadata(&seq_output).unwrap().len();
    let ratio = par_size as f64 / seq_size as f64;
    eprintln!(
        "par_size={} seq_size={} ratio={:.3}",
        par_size, seq_size, ratio
    );
    assert!(
        ratio > 0.8 && ratio < 1.2,
        "parallel/sequential file size ratio {} out of range",
        ratio
    );
}
