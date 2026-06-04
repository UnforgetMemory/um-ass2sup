# Changelog

All notable changes to **um-ass2sup** (the ASS/SSA → Blu-ray SUP subtitle converter) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.3.1] - 2026-06-04

### Highlights
- First release published as a **GitHub Release** with prebuilt binaries.
- Library API is now fully `rustdoc` documented (zero missing-doc warnings).
- `ass2sup` is **~1.4× faster** on large multi-event scripts via opt-in parallel quantize.

### Added
- `--check` flag: parse + serialize roundtrip self-check, exit non-zero on divergence.
- `--to-srt` flag: convert ASS/SSA/SRT to SRT (lossless on SRT input).
- `--to-bdn` flag: emit BDN XML format (Blu-ray authoring).
- `--parallel-frames` flag: opt-in rayon-parallel quantization for large scripts.
- 3 runnable example programs (`parse_ass`, `quantize_image`, `encode_sup`) under `crates/*/examples/`.
- 2 Architecture Decision Records under `docs/adr/` (k-d tree quantizer, parallel quantize).
- 100 MiB input size guard (`MAX_INPUT_SIZE_BYTES`) prevents accidental multi-gigabyte reads.
- `deny.toml` for `cargo-deny` (advisories / bans / sources / licenses).
- Weekly `cargo audit` workflow (Mondays 06:00 UTC + on push/PR).
- `SECURITY.md` with vulnerability reporting policy and supported-versions table.

### Changed
- **MSRV bumped from 1.75 to 1.85** (ecosystem has moved to edition2024: `clap` 4.6, `rayon` 1.12, `proptest` 1.11, `getrandom` 0.4 all require Rust ≥ 1.80).
- Workspace dependencies centralized in `[workspace.dependencies]` (13 external crates).
- 6 source crates inherit `license.workspace = true` (MIT OR Apache-2.0).
- Quantize step parallelized via rayon — **30-event stress test: 0.366s → 0.270s (1.36×), output is byte-identical** to the sequential path.
- `find_nearest_index` uses an in-tree k-d tree — **1080p single frame: 908ms → 353ms (2.57×)**.
- `Renderer::new()` now delegates to `try_new()`; `RendererError::NoFonts` replaces the old panic.
- `Mutex` swapped for `parking_lot::Mutex` in renderer for less syscall overhead.
- Small-palette dedup uses `HashSet<u32>` (O(n²) → O(n)).
- Dead `charset` field removed from renderer config.
- CLI errors are now printed to stderr before exit (was silently swallowed).
- `release.yml` toolchain pinned to 1.85 in all 3 jobs.

### Fixed
- **Fuzz-found OOB in `pgs-encoder::parse_ods_payload`**: length check was `< 4` (only checked segment header), should be `< 8` (must also cover width/height at bytes 4–7). Without this fix, malformed ODS segments could trigger an out-of-bounds slice.
- **Fuzz-found off-by-one in `pgs-encoder::parse_wds_payload`**: window stride was 8 bytes, should be 9 (1 flag byte + 8 coord bytes per window).
- `--to-srt` on SRT input produced a silent 0-byte file (parser was bypassing `SubtitleFormat::detect`); now performs a lossless SRT roundtrip via `AssFile::parse_file`.
- `test_clip_rect_pixels` was flaky on CI (font rasterization drift) — region-based assertion replaced exact pixel check.
- Clippy 1.96 `manual_repeat_n` lint: `repeat(0u8).take(48)` → `repeat_n(0u8, 48)`.
- `cargo audit --deny warnings` ignores `RUSTSEC-2025-0119` (`number_prefix` unmaintained, transitive via `indicatif 0.17.11`).
- 10 broken `#[ignore]` doc-tests now compile and run.
- `crates/bdn-xml/proptest-regressions/` artifacts now properly gitignored (previous inline-commented gitignore pattern was broken).

### Removed
- 11 non-source artifacts (phase plans, HTML specs, test output, coverage xml).
- 2 redundant `crates/*/fuzz/.gitignore` files (consolidated to root pattern).
- Internal "Phase 25/26/27/..." jargon from changelog entries — replaced with user-facing descriptions.

---

## [0.3.0] - 2026-06-04

> **Note:** 0.3.0 was tagged locally but never published as a GitHub Release (the `release.yml` was set up in this same window). 0.3.1 is the first public release and contains all 0.3.0 changes plus CI/release hardening.

### Added
- Renderer no longer panics on missing fonts; returns `RendererError::NoFonts` via `try_new()`.
- `deny.toml`, `.github/workflows/audit.yml`, `SECURITY.md`.
- 13 external dependencies centralized in `[workspace.dependencies]`.
- 6 source crates inherit workspace `license = "MIT OR Apache-2.0"`.
- `ass-parser` proptest: 8 new property tests (ASS determinism, ASS lenient recovery, 5 SRT roundtrips).
- Insta 1.47.2 CLI snapshot tests (5 cases for `--check`, `--to-srt`, etc.).
- `cargo bench --workspace --no-run` step in CI test job.
- `COVERAGE.md` with 88.13% line coverage baseline (tarpaulin xml, lower bound).
- `--parallel-frames` CLI flag (rayon-parallel quantize, opt-in, default off).
- `#![warn(missing_docs)]` enforced in `ass2sup-cli`, `color-quantizer`, `subtitle-validator`.
- `cast_lossless` clippy lint enforced workspace-wide (49+ fixes via `u32::from` / `u64::from` etc.).

### Fixed
- `ass-parser` SRT input was being dispatched to the ASS parser, causing 0-event output.
- CLI panic on malformed glob pattern now returns an error.
- `Renderer::new()` no longer silently no-ops without a usable font.

---

## [0.2.0] - 2026-06-02

### Added
- 35 baseline benchmarks (24 renderer + 6 encoder + 5 quantizer).
- `--to-srt` CLI flag for ASS → SRT conversion.
- `BENCHMARKS.md` documenting baseline performance.
- Proptest property-based testing (23 tests across 3 crates).
- Batch mode flags (`--glob`, `--recursive`, `--max-files`).
- ASS → SRT serializer with 9 unit tests.
- SSA v4 edge cases (7 new tests).
- Module-level rustdoc and fixed 28 broken intra-doc links.
- Extracted `ass2sup-cli` lib.rs (Args / CliError / run).

---

## [0.1.0] - 2026-05-31

### Added
- Initial public release.
- ASS / SSA → PGS / SUP conversion with palette reuse.
- SRT round-trip support (parse + serialize).
- BDN XML output format.
- CLI: glob / recursive batch, output directory, palette reuse, dither selection.
- Golden snapshot tests for renderer output.
- 100+ unit and integration tests.
- GitHub Actions CI (linux, stable Rust).
