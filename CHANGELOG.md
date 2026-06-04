# Changelog

All notable changes to um-ass2sup will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.1] - 2026-06-04

### Added
- Chinese (primary) + English bilingual READMEs (`README.md` + `README.en.md`)
- Professional doc structure: architecture, workspace table, CLI reference, library usage, benchmarks

### Changed
- MSRV bumped: 1.75 → 1.85 (ecosystem moved to edition2024; clap, rayon, proptest all require 1.80+)
- CI workflow: `fonts-dejavu-core` installed for consistent test rasterization
- All deps updated to latest stable (`cargo update`)
- `release.yml` toolchain: 1.75 → 1.85 (3 jobs)
- `.gitignore`: inline comments removed (were silently breaking patterns), emacs patterns escaped

### Fixed
- `test_clip_rect_pixels` flaky on CI rustc 1.96: exact pixel assertion replaced with region-based (190-290, 180-280) — guards font rasterization drift
- `cargo audit --deny warnings` ignored RUSTSEC-2025-0119 (`number_prefix` unmaintained, transitive via `indicatif`)
- Clippy 1.96 new lint `manual_repeat_n` in `pgs-encoder/src/decoder.rs:596` — `repeat(0u8).take(48)` → `repeat_n(0u8, 48)`

### Removed
- 11 non-source artifacts (phase plans, HTML specs, test output, coverage artifact)
- 2 redundant `crates/*/fuzz/.gitignore` (consolidated to root pattern)

## [0.3.0] - 2026-06-04

### Added
- Phase 25: `Renderer::try_new()` with `RendererError::NoFonts` error path (replaces panic)
- Phase 25: `deny.toml` for cargo-deny (advisories/bans/sources/licenses)
- Phase 25: `.github/workflows/audit.yml` (weekly Monday 06:00 + on push/PR)
- Phase 25: `SECURITY.md` (vuln reporting policy, supported versions)
- Phase 25: 13 external deps centralized in `[workspace.dependencies]`
- Phase 25: `crates/bdn-xml/Cargo.toml` inherits workspace `license = "MIT OR Apache-2.0"`
- Phase 24: 10 doc-tests converted from `#[ignore]` to `no_run` or runnable
- Phase 24: CI step `cargo test --workspace --doc` in `.github/workflows/ci.yml`
- Phase 24: Two new cargo-fuzz targets (`decode_pgs`, `quantize_rgba`)
- Phase 24: Property test for test_stats_accuracy assertion
- Phase 24: `BENCHMARKS.md` Phase-24 update (2.57x k-d tree speedup)
- Phase 25: `.github/workflows/release.yml` triggered on `v*` tag push
- Phase 25: `CHANGELOG.md` 0.3.0 entry
- Phase 25: `README.md` CI/Audit/License/Rust/Version badges
- Phase 26: ass-parser proptest +8 tests (ASS determinism, ASS lenient recovery, 5 SRT roundtrips)
- Phase 26: insta 1.47.2 CLI snapshot tests (5 cases for `--check`, `--to-srt`, etc.)
- Phase 26: `cargo bench --workspace --no-run` step in CI test job
- Phase 26: `COVERAGE.md` with 88.13% baseline (tarpaulin xml, lower bound)
- Phase 27: `--parallel-frames` CLI flag (rayon-parallel quantize, opt-in, default off)
- Phase 28: `#![warn(missing_docs)]` enforced in 3 crates (ass2sup-cli, color-quantizer, subtitle-validator)
- Phase 28: 14 pub items documented across 4 files
- Phase 28: `cast_lossless` clippy lint enforced workspace-wide (49+ fixes via `u32::from`/`u64::from`/etc.)
- Phase 29: 3 runnable examples (`parse_ass`, `quantize_image`, `encode_sup`)
- Phase 29: 2 ADRs in `docs/adr/` (kd-tree quantizer, parallel quantize)
- Phase 29: 100 MiB `MAX_INPUT_SIZE_BYTES` input guard with `CliError::InputTooLarge`
- Phase 29: `main.rs` now prints `CliError` to stderr before exit (was silent failure)

### Changed
- Phase 24: `Renderer::new()` now delegates to `try_new()` (panics retained for compat)
- Phase 24: `Mutex` swapped for `parking_lot::Mutex` in renderer
- Phase 24: Dead `charset` field removed from renderer config
- Phase 24: Small-palette dedup uses `HashSet<u32>` (O(n²) → O(n))
- Phase 24: `find_nearest_index` uses in-tree k-d tree (1080p 908ms → 353ms, 2.57x)
- Phase 24: `subtitle-validator` test_stats_accuracy now asserts `karaoke_events == 1`
- Phase 25: Workspace version bumped 0.2.0 → 0.3.0
- Phase 25: All 6 source crates inherit `license.workspace = true`
- Phase 27: Quantize step parallelized via rayon (sequential quantize 0.366s → parallel 0.270s, 1.36x on 30-event stress test, output byte-identical)

### Fixed
- Phase 24: CLI no longer panics on malformed glob pattern (returns error)
- Phase 24: SRT input now correctly dispatched (was always falling through to ASS parser)
- Phase 24: 10 broken `#[ignore]` doc-tests now compile/run
- Phase 24: `crates/bdn-xml/proptest-regressions/` artifacts gitignored
- Phase 26: pgs-encoder `parse_ods_payload` OOB: data length check was `< 4`, should be `< 8` (width/height at bytes 4-7)
- Phase 26: pgs-encoder `parse_wds_payload` off-by-one: window stride was 8, should be 9 (9 bytes/window: 1 flag + 8 coords)
- Phase 28: `--to-srt` on SRT input was silent 0-byte output (now lossless roundtrip via `parse_file` swap)
- Phase 29: `main.rs` swallowed all errors (now prints `CliError` to stderr, fixes `test_cli_missing_file`)

## [Unreleased] - Phase 23

### Added
- LICENSE-MIT and LICENSE-APACHE dual-license files
- GitHub Actions CI workflow
- Crate-level documentation for bdn-xml
- Inline unit tests in subtitle-validator
- `--to-bdn` CLI flag for BDN XML output mode

### Changed
- Aligned bdn-xml, subtitle-renderer, color-quantizer versions to workspace 0.2.0
- All crates now inherit `rust-version.workspace = true`

### Fixed
- SRT parser graceful handling of malformed timestamps (regression test added)
- ASS lenient parser hardening against malformed [Fonts] sections
- Override tag parser hardening against malformed binary input
- subtitle-validator V014 karaoke consistency check

## [0.2.0] - 2026-06-02

### Added
- Phase 22: 35 baseline benchmarks (24 renderer + 6 encoder + 5 quantizer)
- Phase 22: `--to-srt` CLI flag for ASS\&gt;SRT conversion
- Phase 22: `BENCHMARKS.md` documenting baseline performance
- Phase 22: Proptest property-based testing (23 tests across 3 crates)
- Phase 22: Batch mode flags (`--glob`, `--recursive`, `--max-files`)
- Phase 22: ASS\&gt;SRT serializer with 9 unit tests
- Phase 22: SSA v4 edge cases (7 new tests)
- Phase 22: Module-level rustdoc and fixed 28 broken intra-doc links
- Phase 22: Extracted `ass2sup-cli` lib.rs (Args/CliError/run)

## [0.1.0] - 2026-05-XX

### Added
- Phase 21: CLI polish, fuzz testing infrastructure, expanded test coverage
- Phase 20: Missing ASS effects (vertical text, p4 clip, animation skip)
- Phase 19: `Arc<RenderedFrame>` for memory efficiency
- Phase 18: Various renderer optimizations
- Phase 17: 3D perspective (`\frx`/`\fry`) + embedded font loading
- Phase 16: Clip, drawing, wrap, combined-tag tests
- Phase 15: Shadow, karaoke, transform, fade, move, border test coverage
- Phase 14: Banner/Scroll rendering, karaoke segments, cache fix, `\t(\pos)`, vector clips
- Phase 13: Anisotropic borders via morphological dilation
- Phase 12: Additional ASS tag support, font Mutex for Sync, SIMD bilinear, frame caching
- Phase 11: Vector clip, SIMD pixel ops, font/glyph caching, 165 new tests
- Phase 10: ASS tag unification + glyph outlines + performance optimizations
- v0.1.0 initial release: palette reuse, CLI UX, golden tests, multi-format support
