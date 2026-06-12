# Changelog

All notable changes to **um-ass2sup** (the ASS/SSA → Blu-ray SUP subtitle converter) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.5.0] - 2026-06-12

### Highlights
- **CJK OCR verification**: 9 end-to-end OCR test fixtures (ASCII, French, Chinese Simplified/Traditional, Japanese, Korean, Mixed CN/EN, Effects, Chinese Styled) — all pass with PaddleOCR.
- **Security hardening**: Integer overflow protection for SUP dimensions, shell injection fix in OCR command execution, temp file leak fix in Python harness.
- **CJK rendering fixes**: Correct glyph Y-axis conversion (font→screen coordinate transform), top-aligned text baseline shift, CJK font fallback priority.

### Fixed
- **Shell injection in `ocr.rs`**: Replaced `sh -c` command interpolation with direct `Command::new(program).arg(path)` — unescaped file paths no longer pass through a shell.
- **Integer overflow in SUP dimension calculations**: Added `checked_mul` in `decode_to_image.rs` and `rle.rs` — malicious SUP files with `width × height` exceeding `usize::MAX` now return `InvalidDimensions` error instead of silently wrapping.
- **Spurious alpha-channel swap in `decode_to_image.rs`**: Removed dead code that read `pixel[3]` (alpha channel) as a palette index and applied `swap(idx, transparent_index)`, corrupting transparency for all frames with `transparent_index != 0`.
- **Single-byte opaque RLE format ambiguity**: Encoder now emits `[color, 0x40]` (2-byte) instead of `[color]` (1-byte) for 1-pixel opaque runs, preventing misinterpretation by the decoder's transparent long-run parser.
- **RLE collision-range palette index remap**: `remap_collision_range()` in `encoder.rs` remaps indices `0x40–0x7F` to unused slots in `0x80–0xBF` before encoding, eliminating ambiguity with transparent long-run format bytes.
- **CJK font fallback priority**: Font resolution now checks CJK-capable fonts (Noto Sans CJK SC/TC/JP/KR, Microsoft YaHei) before Latin-only fonts (DejaVu Sans).
- **Glyph Y-axis conversion**: `OutlineAdapter` in `rasterizer.rs` correctly negates Y coordinates (`offset_y - y * scale`), and `compute_tight_bbox` in `compositing.rs` uses proper `gy - bbox.y_max` / `gy - bbox.y_min` ordering.
- **Top-aligned text clipping**: For ASS alignment 7/8/9 (top), baseline Y is shifted down by `font_size` to prevent glyphs from rendering above the frame boundary.
- **Temp file leak in `ocr_harness.py`**: Split into `main()` + `_run_ocr()` with explicit `os.unlink()` cleanup after OCR completes.

### Added
- **9 OCR E2E test fixtures**: `simple.ass` (English), `ocr_fr.ass` (French), `ocr_zhcn.ass` (Chinese Simplified), `ocr_zhtw.ass` (Chinese Traditional), `ocr_ja.ass` (Japanese), `ocr_ko.ass` (Korean), `ocr_mixed_cn_en.ass`, `ocr_effects.ass`, `island_disappeared.ass`.
- **`InvalidDimensions` variant** on `DecodeImageError` for overflow-safe dimension validation.
- Japanese (`Noto Sans CJK JP`) and Korean (`Noto Sans CJK KR`) fonts installed for test environment.

### Changed
- **`decode_to_image.rs`**: Removed unused `swap` import (was leftover from the alpha-channel swap bug).
- **`test_ocr_e2e.rs`**: OCR comparison now uses only the rendered event's text (was comparing against all events concatenated, producing artificially low similarity).

### Security
- H1: Integer overflow in `width × height × 4` buffer allocations → `checked_mul` with error propagation.
- H2: Shell injection via `sh -c` with unescaped path → direct `Command::new().arg()` invocation.
- M1: Temp file leak in `ocr_harness.py` → explicit cleanup with `os.unlink()`.

---

## [0.4.1] - 2026-06-11

### Fixed
- **PGS RLE encoding collision range**: Colors in `0x40..0x7F` no longer produce ambiguous byte patterns that collide with transparent long-run markers. The encoder now forces these colors into 3-byte long-run format to avoid decoder misinterpretation.
- **PGS RLE decoding ambiguity**: The decoder now tries transparent interpretation first for ambiguous `[0x40|len_hi, len_lo]` byte sequences, with bounds checking to prevent false positives. Falls back to opaque interpretation when transparent fails.
- **PCS header byte-layout mismatch**: The decoder now correctly reads `palette_update` (1 bit) + `palette_id` (7 bits) packed in a single byte, matching both the PGS spec and encoder serialization. `PCS_HEADER_SIZE` reduced from 11 to 10 bytes.
- **ODS payload length field**: Removed erroneous `+4` from RLE data length calculation, fixing format compliance.
- **RLE decode output bounds**: Added bounds checking for short transparent runs and row separators to prevent output from exceeding `total_pixels`.
- **Palette double-swap in multi-window compositing**: Moved palette reconstruction for `transparent_index != 0` before the object loop to prevent double-swap with multiple display objects.

### Changed
- **`decode_to_image.rs`**: Removed `rle_data[8..]` offset skip (was compensating for the ODS length bug). Uses `rle_data` directly.
- **`decode_to_image.rs`**: `composite_objects` now takes `&mut RenderContext` for palette reconstruction.
- **`rle.rs`**: Replaced `repeat().take()` with `repeat_n()` throughout (clippy compliance).
- **`color.rs`**: Fixed `clone_on_copy` warning on `PaletteEntry`.

### Tests
- Added `test_decode_roundtrip` integration tests (sparse glyph, bottom row, second-to-last row).
- Removed duplicate `test_roundtrip2` test (was redundant with `test_decode_roundtrip`).
- Updated benchmark `encoder.rs` to pass 4th argument to `rle_encode`.

### Security
- Added bounds checking in `rle_decode` for short transparent runs to prevent output exceeding `total_pixels`.

---

## [0.4.0] - 2026-06-09

### Highlights
- **OCR verification pipeline**: Full SUP→PNG decode + PaddleOCR text comparison for end-to-end subtitle quality verification.
- **CJK font support**: Extended font fallback chain with `Noto Sans CJK SC/TC`, `WenQuanYi Micro Hei`, `Source Han Sans CN`, `IPAGothic`, `NanumGothic`.
- **Per-style font fallback**: New `--font-map` CLI flag for per-ASS-style font fallback chains.

### Added
- **`crates/pgs-encoder/src/color.rs`**: `ycbcr_to_rgba()` (BT.601 inverse) and `palette_to_rgba()` — symmetric inverse of existing `rgba_to_ycbcr()`.
- **`crates/pgs-encoder/src/decode_to_image.rs`**: `decode_frame_to_rgba()` and `frame_to_png()` — SUP→RGBA→PNG decode pipeline supporting multi-window/epoch-split compositing.
- **`crates/ass2sup-cli/src/ocr.rs`**: OCR toolkit — `run_ocr()`, `strip_ass_tags()`, `normalized_similarity()`, `parse_ocr_json()`, `is_match()`.
- **`scripts/ocr_harness.py`**: PaddleOCR CLI harness for `scripts/ocr-requirements.txt`.
- E2E OCR roundtrip test (`crates/ass2sup-cli/tests/test_ocr_e2e.rs`) with 6 fixtures — runs automatically in `cargo test` and skips gracefully when PaddlePaddle is unavailable.
- **`docs/sup-ocr-validation.md`**: Documents the full OCR validation pipeline architecture.
- **`docs/FONT_REQUIREMENTS.md`**: Documents font fallback chain, CJK font installation, and CLI `--font` / `--font-map` usage.
- **`check_ass_fonts()`**: Pre-render font availability check that enumerates all missing fonts before failing.
- **`--font-map`**: New repeatable CLI flag (`StyleName:fallback1,fallback2`) for per-style font fallback configuration.
- **`--no-check-fonts`**: Existing flag now checks all ASS style fonts, not just the global `--font` argument.

### Changed
- **Font fallback chain** (`subtitle-renderer/src/font.rs`): Extended from 5 Latin-only fonts to 11 including CJK-capable families; `fontconfig SansSerif` query added as step 3.
- **`RenderConfig.default_font`** is now properly consulted when a style's `font_name` is empty (was previously dead code).
- E2E OCR test now runs as part of the normal test suite — no `#[ignore]` removal needed.

---

## [0.3.2] - 2026-06-07

### Fixed
- **Generated `.sup` was unusable: PGS PCS `palette_update_flag` was inverted.** The encoder wrote `palette_update = !palette_changed && frame_count > 0`, which on every frame after the first signaled "no new palette" while the PDS carried a duplicate — but with a different `version` byte (u8 cast from `frame_count`), causing players to reject the PDS and lose all subsequent subtitles. Spec-correct behavior is `palette_update = palette_changed` (`1` ⇒ new palette in PDS, `0` ⇒ reuse previous).
- **`prev_palette_hash` / `prev_object_rle_hash` were never written** in `PgsEncoder` — the fields were only read (for `palette_changed` / `object_changed` computation) but never updated after each `build_display_set` call, so the change detector was always `None != Some(hash) == true`. `build_display_set` now takes `&mut self` and stores both hashes at the end of each call.

### Tests
- 2 new TDD tests in `crates/pgs-encoder/tests/test_edge_cases.rs`:
  - `test_pcs_palette_update_spec_compliance` — encodes 3 frames (new/unchanged/changed) and asserts PCS `palette_update = [true, false, true]`.
  - `test_pcs_palette_update_roundtrips_through_sup_bytes` — encodes 2 identical red frames, decodes via `decode_sup`, asserts `[true, false]`.

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
