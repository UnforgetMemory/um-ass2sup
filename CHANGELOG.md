# Changelog

All notable changes to um-ass2sup will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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
