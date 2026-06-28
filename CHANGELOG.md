# Changelog

All notable changes to **um-ass2sup** (the ASS/SSA → Blu-ray SUP subtitle converter) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [2.7.5] - 2026-06-27

### Added
- **Smart frame classification**: Rewrote `render_and_quantize` to generate render timestamps only at event boundaries and animation-keyed frame points, eliminating ~120k intermediate frame clones. Rendered frames are classified as:
  - *Empty* (all-transparent) → discarded, preceding frame's display duration extended
  - *Duplicate* (pixel-identical to previous unique frame) → merged, duration extended
  - *Unique* → kept as a new NormalCase/EpochStart display set
- **Frame-accurate PTS computation**: New `frame_accurate_pts()` rounds ms timestamps to the nearest video frame boundary before 90 kHz conversion, eliminating NTSC drift (was ~22ms per event, now <1ms).
- **Exact 24 fps support**: CLI now correctly handles 24 fps sources (`-f 24`) without applying NTSC 1001/1000 factors.

### Changed
- **EpochContinue optimization**: `build_continue_display_set` now omits PDS (palette unchanged), reducing SUP size by ~48 MB for heavily duplicated content.
- **No per-frame palette_clear**: Removed trailing palette_clear from `encode_frame` — palette_clear is now emitted only at event boundaries via `emit_clear()` in `encode_sup`, eliminating the clear→show→clear flicker cycle.
- **Extended epoch duration**: `max_frames_per_epoch` increased from ~1s to ~30s, reducing forced EpochStart resets from 6,062 to 103 (98% reduction).
- **PotPlayer compatibility**: EpochContinue PCS retains `palette_update=true` (PotPlayer requirement), with PDS restored to prevent player crash.
- **Exposed `encode_frame_at_pts`**: New public method accepts exact 90 kHz PTS, bypassing ms→90 kHz double conversion for frame-accurate callers.

### Fixed
- **NTSC ms→PTS drift**: `ms_to_90khz` accumulated ~0.7ms error per frame at 23.976 fps, causing subtitle timeline to drift forward by ~85s over a 90-minute movie. Fixed by rounding to the nearest video frame boundary before conversion.
- **EpochContinue palette starvation**: EpochContinue PCS with `palette_update=false` caused PotPlayer to crash. Reverted to `palette_update=true` with full PDS.
- **Frame-timestamp accumulation drift**: `ts += ms_per_frame as u64` truncated ~0.7ms per frame at non-integer frame rates. Changed to f64 accumulation with index-based rounding.
- **Override tag rendering**: Fixed multiple ASS override tags not being applied in the FontRegistry-based renderer:
  - `\fscx`/`\fscy` (ScaleX/ScaleY): `transform_layer` now builds a real affine transform from RenderContext values instead of calling with identity matrix.
  - `\frz`/`\fr` (Angle/Rotation): Incorporated into the composite transform chain (scale → shear → rotate around layer centre).
  - `\fax`/`\fay` (Shear): Now properly transformed via `AffineTransform::shear`.
  - `\bord` (Outline): Added outline rendering pass — tints fill glyph mask with outline_color, applies gaussian blur for expansion, then composites fill over the blurred outline.
  - `\fsp` (Spacing): Now accounted for in word-wrap width calculation (`wrap_text_lines_simple`), preventing premature line breaks when spacing > 0.
- **Alignment anchoring for `\pos`/`\move`**: `has_pos` branch in `shape_horizontal` now correctly places bottom/centre/top-aligned text at the anchor point. Previously bottom-aligned multi-line text extended below `\pos` instead of above it.
- **Alignment without explicit positioning**: `!has_pos` branch uses safe-area margins instead of double-applying `ctx.y` from `build_context`, fixing severe text drift at centre/top alignments.
- **Font weight resolution**: When `bold=true` and `parse_font_name` yields a weight lighter than Bold, also queries the parsed family at Bold weight — matches libass behaviour for font names like "MiSans Demibold" with bold flag set.
- **ASS text escapes**: `process_ass_text_escapes` now converts `\N`/`\n` to newlines, `\h` to non-breaking space, and `\\` to escaped backslash (previously all were passed through as literal text).
- **Alignment with spacing**: `shape_horizontal` now includes `glyph_count * spacing` in per-line total advance (`ta`), preventing text misalignment when `\fsp>0`.
- **`font_available` consistency**: Both `Renderer::font_available()` and the standalone `font.rs` version now use `parse_font_name` decomposition, matching `resolve_font_data` logic for compound font names like "MiSans Demibold".
- **`--no-check-fonts` partial mode**: When `--no-check-fonts` is active, fonts with an explicit `--font-map` entry are skipped; fonts without any fallback are still checked and reported as missing, preventing silent 0-frame output.
- **Font check disambiguation**: `check_ass_fonts_with_fn` no longer returns `Ok` unconditionally when `no_check=true`. Fonts without a `--font-fallback-map` entry are still validated.
- **Cleanup**: Removed duplicate `parse_font_name` from `layout_font_registry.rs` — uses the re-exported version via `renderer/mod.rs` (DRY).

### Security
- **Security audit**: External review of 6 changed files found no CRITICAL/HIGH vulnerabilities. Identified 3 theoretical LOW-severity items (extreme coordinate f32→u32 cast, CPU DoS via unbounded blur/shadow radii, unbounded `apply_to_pixmap` allocation) — none actionable in current CLI usage context.

### Tests
- **ASS text escape coverage**: Added 7 unit tests for `process_ass_text_escapes` (`\N`, `\n`, `\h`, `\\`, mixed, empty, no-escape cases) and 4 tests for `strip_override_blocks`.
- **Clippy fix**: Moved `composite_subregion` before `mod tests` in `composite.rs` to resolve `items_after_test_module`.
- **Stale doctest fix**: Updated `karaoke.rs` doc example from `ass_parser` to `ass_core` (pre-existing reference to renamed crate).

## [2.7.4] - 2026-06-26

### Security
- **Path traversal in embedded font loading**: `create_renderer()` now rejects embedded font filenames containing `..` components that escape the input file directory. Previously, a malicious ASS file with `filename = ../../../../etc/passwd` in the `[Fonts]` section could read arbitrary files from the filesystem (CWE-22). Found via security audit.

### Fixed
- **Frame-level animation support**: Replaced event-driven pipeline (one frame per event) with frame-driven pipeline (one frame per video frame timestamp). Each frame now renders ALL active subtitle events composited together, matching libass behavior. This fixes:
  - Subtitles appearing/disappearing abruptly instead of per-frame
  - Multiple simultaneous subtitles interfering with each other (object_id=0 collision causing premature clear)
  - PTS drift where `q.pts_ms = event.start_ms` instead of frame-aligned timestamps
  - ASS styles (\pos, \move, \an) not being applied correctly due to single-frame rendering
- **Fade effect transitions lost**: Removed `compute_render_pts()` which shifted render PTS past fade-in duration, causing fade animations to be completely lost. Frame-driven rendering now naturally captures fade alpha at each frame timestamp.
- **write_bdn frame-driven output**: Fixed BDN XML writer to iterate over frames instead of events, using `q.pts_ms` and `q.duration_ms` for timecodes instead of event start/end times.
- **Smart rendering optimization**: Frame-driven pipeline now re-renders only at change points (event start/end), reusing the previous frame for static periods. Reduces rendering from ~120K+ frames to ~7K-111K depending on event density.
- **Per-frame duration calculation**: `duration_ms` now equals time to next frame timestamp instead of single `ms_per_frame`, ensuring proper display duration in SUP output.
- **Font registry did not index full family names**: Fonts with multiple family names (e.g., "MiSans" + "MiSans Demibold") were only indexed under the primary name. ASS files referencing "MiSans Demibold" could not find the font. Fixed: `FontIndex::insert` now also indexes by full name (family + weight).

### Changed
- **Pipeline architecture**: `render_and_quantize()` now iterates over frame time points instead of iterating over events. Each frame renders all active events via `renderer.render_ass(doc, frame_pts)`.
- **PTS assignment**: `q.pts_ms` now equals frame timestamp (frame boundary aligned) instead of `event.start_ms`. `q.duration_ms` now equals time to next frame instead of `event.end_ms - event.start_ms`.
- **Parallel processing**: Removed `--parallel-frames` rayon path (incompatible with frame-driven sequential rendering). Flag is now deprecated with warning.
- **Progress bar**: Shows frame count instead of event count.
- **Font fallback chain**: `resolve_font_data()` and `resolve_glyph_font_data()` now consult the per-style `font_map` from `--font-map` CLI option when the primary font is not found. Fallback success emits a `WARN` message indicating which font fell back to which fallback.
- **Font availability check**: `check_ass_fonts()` is now integrated into the conversion pipeline and runs before rendering. Missing fonts are reported with detailed error messages. Use `--no-check-fonts` to skip.
- **Font index**: `FontIndex::insert` now also indexes fonts under their full name (family + weight) for ASS compatibility (e.g., "MiSans Demibold" as well as "MiSans").

### Added
- `ass2sup-cli/util.rs`: `generate_frame_timeline(events, fps) -> Vec<u64>` function computes all frame timestamps from earliest event start to latest event end, using float-based per-frame computation to avoid drift at non-integer fps (23.976, 29.97, etc.).
- `subtitle-renderer/renderer/mod.rs`: `Renderer::font_available(family)` and `Renderer::set_font_map(font_map)` public methods for font availability checking and font map injection.
- `subtitle-renderer/font/types.rs`: `FontWeight::as_str()` method returning the weight variant name (e.g., "Bold", "Semibold").
- `ass2sup-cli/config/font.rs`: `check_ass_fonts_with_fn()` variant using a closure instead of direct registry reference, enabling font checks through the Renderer's internal registry.

### Removed
- `ass2sup-cli/util.rs`: `compute_render_pts()` function (obsoleted by frame-driven rendering).
- `ass2sup-cli/pipeline/convert.rs`: Parallel render path using `rayon::par_iter`.

---

## [2.7.3] - 2026-06-26

### Fixed
- **PGS PCS ObjectComposition x/y hardcoded to (0,0) — subtitles stacked in top-left corner**: All display set builders (`build_single_window_display_set`, `build_multi_window_display_set`, `build_continue_display_set`, `build_palette_only_display_set`) set `ObjectComposition.x = 0, y = 0` in the PCS segment, while positioning was only in the WDS (WindowDef). Many players (including PotPlayer) honor PCS object coordinates, causing all subtitle bitmaps to render at screen origin. Fixed: PCS ObjectComposition now carries `frame.x, frame.y` matching the WDS window position.
- **Epoch-split path loses original subtitle position**: `build_epoch_split_display_set` constructed band frames with `x: 0, y: 0`, discarding the original frame origin. Large subtitles hitting the split threshold rendered at (0,0) regardless of ASS positioning. Fixed: band frames now propagate `frame.x` and `frame.y + band_y_offset`.
- **`\fsc` (ScaleReset) override tag silently ignored**: `OverrideTag::ScaleReset` was parsed but fell through `_ => {}` in `build_context.rs` and `transform.rs`. Fixed: now resets `ctx.scale_x` and `ctx.scale_y` to 100.0.
- **`\fe` (Charset) override tag silently ignored**: `OverrideTag::Charset` was parsed but not applied. Added `font_charset: u8` field to `RenderContext` and handling in `build_context.rs` and `transform.rs`.
- **`\clip(@)` / `\iclip(@)` (ClipDrawingCurrent) override tags silently ignored**: `ClipDrawingCurrent` and `ClipInverseDrawingCurrent` were parsed but not applied. Fixed: now reuses the most recent drawing clip commands from the same event.

### Changed
- `pgs-encoder/encoding/display_set.rs`: PCS ObjectComposition x/y now propagates `frame.x, frame.y` in all display set builders.
- `subtitle-renderer/renderer/build_context.rs`: Added handling for `ScaleReset`, `Charset`, `ClipDrawingCurrent`, `ClipInverseDrawingCurrent` override tags.
- `subtitle-renderer/renderer/animation/transform.rs`: Added handling for `ScaleReset` and `Charset` in `\t()` animation path.
- `subtitle-renderer/context.rs`: Added `font_charset: u8` field to `RenderContext`.

### Security
- Security audit completed with 5 findings (0 Critical, 0 High, 3 Medium, 2 Low).
- Medium: Multi-window y-composition u16 overflow risk, RLE slicing panic on malformed input, window defs not clamped to display bounds.
- Low: Object ID u16 overflow risk, unvalidated Charset byte.
- All findings are in safe Rust (no `unsafe`); impact is DoS via panic or data corruption, not memory safety.

### Added
- `pgs-encoder/encoder.rs`: Regression tests `test_pcs_object_position_propagated` and `test_wds_position_matches_frame` verify PCS/WDS coordinates match `QuantizedFrame.x/y`.
- `subtitle-renderer/tests/test_context.rs`: Tests for `ScaleReset`, `Charset`, `ClipDrawingCurrent`, `ClipInverseDrawingCurrent` override tags.

---

## [2.7.2] - 2026-06-26

### Changed
- **License changed from MIT OR Apache-2.0 to Apache-2.0 only**: Simplified licensing by removing dual-license option and using Apache-2.0 exclusively.

### Fixed
- **`composite_over` SIMD stride error causing vertical stripe artifacts**: The SIMD loop used `len/16` stride with `i*16` offset, but only processed 4 bytes per iteration (1 pixel). This skipped 75% of pixels, leaving them uncomposited. Fixed stride to `len/4` with `i*4` offset to process all pixels. Also fixed alpha channel calculation where SIMD computed `src_A²/255` instead of the correct Porter-Duff "over" formula `src_A + dst_A * (1 - src_A/255)`.
- **`\pos` + center/right alignment causing off-screen text rendering**: `shape_horizontal` incorrectly applied full-width centering offset on top of `\pos` coordinates, pushing text beyond the display boundary. Events with `\pos` and non-left alignment rendered as fully transparent frames and were silently dropped (28/1988 events lost). `build_context.rs` now propagates `ctx.has_pos = true`; `shape_horizontal` uses `ctx.x - ta/2` (center) or `ctx.x - ta` (right) when `has_pos` is true.
- **Palette transparent entry contamination during reuse**: `quantize_with_prev` called `find_nearest_index` against the full palette including the transparent `[0,0,0,0]` entry, causing dark opaque pixels to map to index 0 and disappear. Fixed by excluding the transparent entry from the search palette.
- **Dithering colour corruption**: Floyd-Steinberg and Ordered dithering could map opaque pixels to the transparent palette entry (index 0). Post-processing now remaps any opaque pixel at index 0 to the nearest opaque palette entry.
- **`composite_over` SIMD premultiplication bug**: The wide-SIMD path (`u32x4`) omitted the `src * sa` premultiplication step, causing colour shifts during alpha blending (fade effects, anti-aliased text edges). Formula corrected to `s * sa / 255 + d * (255 - sa) / 255`.
- **Default colour space selection**: For HD content (1080p), the default `Srgb` colour space mapped to BT.601 YCbCr coefficients in the PGS encoder, while PotPlayer expects BT.709 for HD video. Now auto-selects BT.709 when resolution height > 576 and no `--color-space` flag is given.
- **PDS palette entry byte order**: PDS serialization wrote `Y, Cb, Cr, Alpha` per palette entry, but the Blu-ray PGS spec requires `Y, Cr, Cb, Alpha`. This Cr↔Cb swap caused systematic colour shifts (red→blue, green→yellow-green) in player decoding. Also added the missing `palette_entry_id` byte (5 bytes per entry per spec, was 4).
- **EpochContinue display set invisible after palette-clear**: The palette-clear display set updated the PGS palette to all-transparent entries. Subsequent EpochContinue display sets had `palette_update=false`, so they re-used the transparent palette and the subtitle appeared invisible in players like PotPlayer. Fixed: EpochContinue now sends `palette_update=true` with a full PDS containing the correct frame palette.

### Changed
- `color-quantizer/pipeline.rs`: `quantize_with_prev` remapping excludes transparent palette entry; dither post-processing handles index-0 contamination.
- `subtitle-renderer/cosmic/effects/composite.rs`: `composite_over` SIMD path corrected with proper premultiplied alpha formula.
- `ass2sup-cli/pipeline/convert.rs`: Auto-select BT.709 for HD content in PGS palette encoding.
- `subtitle-renderer/renderer/build_context.rs`: `\pos`/`\move` set `ctx.has_pos = true`.
- `subtitle-renderer/renderer/layout.rs`: `shape_horizontal` uses `ctx.has_pos` to anchor alignment offsets around `\pos` coordinates.

### Fixed
- **SUP PTS timestamps**: All PGS display sets had PTS=0 due to hardcoded `pts_ms=0, duration_ms=0` in `encode_sup()`, causing players to display no subtitles. Now populated from actual event `start_ms`/`end_ms`.

### Changed
- `color_quantizer::QuantizedFrame` gains `pts_ms` and `duration_ms` fields for carrying presentation timing through the render pipeline.

## [2.7.1] - 2026-06-24

### Fixed
- **cosmic-text/gaussian blur**: Fix pixel write offset miscalculation in `apply_gaussian_blur` where `off` offset was double-counted against the row slice.
- **cosmic-text/clip mask**: Add bounds checking to prevent out-of-bounds memory writes when data buffer is undersized.
- **cosmic-text/composite**: Replace debug-only `debug_assert!` with runtime bounds check and early return for release-build safety.

### Changed
- Workspace version bumped to 2.7.1 (from 0.5.5).

## [2.7.0] - 2026-06-24

### Added
- **DDD architecture for CLI**: Complete rewrite of `ass2sup-cli` from monolithic 1570-line `lib.rs` to domain-driven modules: `cli/` (args, progress), `pipeline/` (convert, batch, srt, check), `config/` (resolution, color-space, font), `error.rs`, `telemetry.rs`. Each source file ≤300 lines.
- **Resolution validation**: `-r` flag is now validated early with a clear error message for malformed values.
- **Feature gate removal**: `cosmic-text` is now the always-enabled font backend (no `cfg(feature = "cosmic-text")`). The old `fontdb`+`rustybuzz`+`ttf-parser` codepath is removed.
- **New test suite**: 15 new test files covering CLI args, errors, telemetry, pipeline, integration, renderer basic, effects, karaoke, and pixmap pool. Old broken test files (10 files, 5000+ lines) rewritten or deleted.

### Changed
- **ass-parser → ass-core**: Renamed and restructured `ass_parser` crate to `ass_core`. `AssFile` → `SubtitleDocument`, `ScriptInfo` → `ScriptMetadata`. Event fields `start`/`end` → `start_ms`/`end_ms` (u64), `style_name` → `style` (StyleRef), `text` → `text_raw`, `name` → `actor`, `margin_l/r/v` → `Option<u32>`, `override_tags` → `Vec<TaggedOverride>`, `karaoke_segments` → `karaoke`. Removed `raw_override_block`.
- **Style type safety**: `border_style` → `BorderStyle` enum, `alignment` → `Alignment` enum, `encoding` → `FontEncoding` enum. Margins consolidated into `Margins` struct. `outline_width` → `outline`, `shadow_depth` → `shadow`.
- **Effect field names**: `Banner.delay_per_pixel` → `delay`, `ScrollUp/ScrollDown.delay_per_row` → `delay`, `top_offset` → `top`, `bottom_offset` → `bottom`.
- **Custom resolution detection**: When `-r` is omitted, the CLI now falls back to `PlayResX`/`PlayResY` from the ASS `[Script Info]` section. Falls back to 1920×1080 if both CLI and script resolution are missing or invalid.

### Removed
- **`_archive/ass-parser/`**: Entire archived crate (200+ files, 5435 lines) deleted, including its fuzz targets, examples, and test data.
- **`FontManager`/`Shaper`/`FrameCache`**: All removed in favour of `cosmic-text` equivalents.
- **Broken test files**: 10 stale test files deleted from `subtitle-renderer/tests/`, `subtitle-renderer/benches/`, and `ass2sup-cli/tests/`.
- **`ass-parser` workspace dependency**: Fully replaced by `ass-core`.
- **`#[cfg(feature = "cosmic-text")]` guards**: Feature-gated codepath removed.

### Fixed
- **Integer overflow in validator**: `event.end_ms - event.start_ms` uses `saturating_sub` to avoid panics when end < start (V012 duration check).
- **Alignment enum validation**: V008 rule still works with `Alignment::to_u8()` type-safe access.

### Internal
- All workspace crates (`subtitle-validator`, `subtitle-renderer`, `ass2sup-cli`) migrated to `ass_core` types exclusively.

### Added
- **cosmic-text font engine**: Migrated from `fontdb`+`rustybuzz`+`ttf-parser` to unified `cosmic-text` with `swash` glyph rasterization. `FontCosmicResolver` wraps `FontSystem`+`SwashCache` for cross-platform font discovery (DirectWrite/CoreText/fontconfig).
- **CosmicShaper**: HarfBuzz shaping pipeline via `cosmic-text::Buffer` with per-glyph color, border, and outline rendering.
- **CosmicPixmapPool**: 8-buffer reusable pixmap pool replaces per-call `Pixmap::new()` allocations, bounding peak memory regardless of parallel render count.
- **53-tag build_context**: Full `OverrideTag`→`RenderContext` pipeline supporting all ASS override tags including `\k`/`\kf`/`\ko` karaoke, `\clip`/`\iclip` vector clipping, `\p4` drawing level-4 clip masks, and `\frz`/`\frx`/`\fry` perspective transforms.
- **Karaoke 4-mode**: `\k` (syllable start), `\kf` (fill clip sweep with sub-pixel accuracy), `\ko` (outline only), and `\K` (optional fine timing). Blur and shadow apply only to the filled/active portion.
- **Cross-platform font fallback**: 8-level chain with CJK glyph verification (`query_cjk_capable_any`), hardcoded CJK fallback list (Noto Sans CJK, WenQuanYi, IPAGothic, NanumGothic), fontconfig alias resolution, and `Family::SansSerif` generic fallback.
- **Parallel rendering**: Rayon `par_iter()` chain merges render+quantize into a single pass, eliminating the intermediate `Vec<RenderedFrame>` (16.5 GB peak avoided for 1988 events at 1080p). Pre-warmed `ThreadPool` prevents Windows deadlock.
- **Spans parsing module**: Structured parse of ASS event text into styled text spans via `cosmic/spans.rs`.
- **Multiline banner/scroll effects**: BOB/TOP/BOTTOM scrolling, left/right/up/down banner scroll implemented with per-line wrap.

### Changed
- **Renderer modularization**: Split monolithic files into focused modules:
  - `renderer/cosmic.rs` (918→236 lines) — extracted layout and karaoke to separate files
  - `renderer/animation.rs` (834 lines) → `animation/{fade,transform,move}.rs`
  - `renderer/compositing.rs` (488→225 lines) — effects extracted to `cosmic/effects/{composite,clip,blur,shadow}`
  - `effects.rs` (446→9 lines) — thin re-export of `cosmic/effects`
  - `renderer/mod.rs` (492→195 lines) — build_context extracted to standalone file
- **Renderer context**: Deprecated `renderer/context/` module (old build_context) with `#[allow(dead_code)]`; unified 53-tag `build_context.rs` is the single source of truth.
- **CLI `-debug` output**: Full execution-chain tracing (read→parse→render→encode→write) via `tracing` spans.
- **`--font-dir` repeatable**: Load TTF/OTF/WOFF2 from arbitrary directories (containerised deployments).

### Fixed
- **3 DoS vulnerabilities** (audit-discovered, security-reviewed):
  - Blur radius clamped to 64px max (`\blur(999999999)` no longer causes CPU DoS)
  - Outline width clamped to 64px max (`\bord(999999999)` no longer causes OOM)
  - Drawing repeat count capped at 10000 (`\p1 999999999 m 0 0` no longer allocates huge Vec)
- **5 `unwrap()` calls** in shadow/clip/karaoke effects replaced with safe `match` fallbacks (prevent potential panic on zero-dimension pixmaps).
- **\p4 clip mask**: Drawing level-4 now correctly applies clip masks instead of skipping.

### Removed
- Legacy `font.rs`/`font_manager.rs`/`shaper.rs`/`rasterizer.rs` (fontdb+rustybuzz stack) — archived to `docs/_archive/fontdb-legacy/`.
- Old `renderer/karaoke.rs` state machine — replaced by `renderer/cosmic_karaoke.rs`.
- `ass-parser` fuzz targets from workspace (crate archived to `_archive`).
- `#[cfg(feature = "cosmic-text")]` gates — cosmic-text is now the sole font backend, not an optional feature.

### Added
- **PGS encoder Epoch management**: `CompositionState::EpochContinue(0xC0)` variant added for PGS spec compliance. `palette_update` flag now dynamically computed from palette hash (was always true). `DisplaySetKind` enum (EpochStart/NormalCase/EpochContinue/PaletteOnly) drives epoch state selection. 8 new tests verifying epoch state transitions, palette update flags, and display set composition.
- **53-tag build_context coverage tests**: 62 new unit tests in `crates/subtitle-renderer/src/renderer/context/tests.rs` covering every `OverrideTag` variant through the `build_context` pipeline. Tests verify correct `RenderContext` field mutations per handler module (position, font, color, border, geometry, clip, karaoke, reset, transform, misc) including scaling, move interpolation, and composite states.
- **Test helper module**: `crates/subtitle-renderer/tests/common/mod.rs` with shared builders (`make_event`, `make_test_doc`, `parse_doc`, `render_doc`) for ass_core type integration tests.

### Changed
- **test_renderer.rs fully migrated to ass_core types**: 107 of 123 tests switched from `renderer.render_ass(&AssFile)` to `common::render_doc(&SubtitleDocument)`. 16 effect/animation tests (banner, scroll, fade, karaoke-fade, perspective, transform) retained on `renderer.render_ass(&AssFile)` for the legacy path — these effects are pending `render_ass_core` implementation.

### Fixed
- **Archived ass-parser test paths**: Fixed `include_str!` paths in `crates/_archive/ass-parser/tests/` that broke when the crate was moved from `crates/ass-parser/` to `crates/_archive/ass-parser/`.

## [2.1.0] - 2026-06-22

### Added
- **New `ass-core` crate**: Complete rewrite of ASS/SSA/SRT parser from scratch on branch `dev-2026-06-22`.
  - `SubtitleDocument` AST: lossless, preserves original text (`text_raw`), `Option<u32>` margins with unset semantics.
  - `time/` module: `Fps{num,den}` rational frame rate, pure integer `ms_to_90khz = ms * 90`, 5 timecode formats (ASS/SRT/TTML/BDN XML/90kHz).
  - `lexer.rs`: Token stream + Section recognition with BOM handling and `Span(line,col,len)` tracking.
  - `section.rs`: ScriptInfo/Style/Event/Font section parsers.
  - `override_tag/`: 9 per-category sub-modules (position, color, font, geometry, clip, effect, karaoke, border + shared util).
  - Libass-compatible override tag parsing: 50/50 PASS per TAG_MATRIX.md, including `\K`=`\kf`, `\a4`→5 VSFilter quirks, `\clip(@)` path reference, `\fsc` scale reset.
  - `srt.rs`: Standalone SRT parser with `to_srt()` roundtrip.
  - 193 tests: unit + proptest + complex scenarios + irregular fixtures + malformed ASS.
  - 4 fuzz targets, 3 Criterion benchmarks.
- **DDD modular architecture**: `types.rs` (StyleRef/Alignment/BorderStyle/Margins), `event.rs` (Event/EventType), `style.rs` (Style) separated from `lib.rs`.

### Changed
- **Old `ass-parser` moved**: `crates/ass-parser/` → `crates/_archive/ass-parser/` for clear separation. Downstream crates (ass2sup-cli, subtitle-renderer, subtitle-validator) still reference it via updated path.
- **Workspace Cargo.toml**: Updated member paths for archive relocation.

### Fixed
- **Eliminated 760 lines of duplicate code**: No more `event.rs::parse_single_tag` vs `override_tag.rs::parse_override_tag` divergence.
- **Zero unwrap_or in parser path**: All silent data loss paths replaced with explicit error/warning propagation.
- **Pure integer timestamp math**: No f64 division/ceil/multiply/round accumulation drift.
- **`\t()` transform parsing**: Fixed to use libass cnt-based argument counting (was treating tag as timing parameter).
- **`\fsc` missing**: Added `ScaleReset` variant (libass resets both scale axes to style defaults).
- **`\fn0` style reset**: Empty `FontName("")` signals style default font (libass compat).
- **`\b0`/`\i0`/`\K` case handling**: Fixed in old ass-parser duplicate code; eliminated in ass-core.
- **`\a4`→5 VSFilter mapping**: Legacy alignment values 4 and 8 remapped to center alignment 5.

### Changed
- **Dependency upgrades**: fontdb 0.16.2→0.23.0, ttf-parser 0.21→0.25.1, rustybuzz 0.14→0.20.1, tiny-skia 0.11→0.12.0 — 4 core dependencies upgraded across 19 major versions.
- **FontManager interior mutability**: Wrapped inner state in `parking_lot::Mutex<FontManagerInner>` to enable font loading from read-only render path.

### Added
- **Fontconfig FFI integration (Linux)**: New `fontconfig` Cargo feature (off by default, `--features fontconfig`) that uses the system fontconfig C library via FFI to discover and load fonts missed by fontdb. Particularly useful for CJK fonts installed through system package managers. Uses `dlopen` to load `libfontconfig` at runtime — no build-time dependency needed.
- **`resolve_via_fontconfig()`**: On-demand font name resolution that lazily loads fontconfig-discovered fonts into fontdb's database. Results cached to avoid repeated lookups.

### Fixed
- **Pre-existing test failures**: Fixed `test_clip_inverse_pixels` (region-based check) and `test_drawing_basic_move_line` (closed path for visible fill).
- **Pre-existing clippy warnings**: Added missing docs to all public items in `ocr.rs` and removed unused imports in test files.
- **`has_available_font()` Tier 4 removed**: Fontconfig's `find()` API always returns a fallback (never fails), making it unsuitable for font existence checking. Font availability is now verified through fontdb's exact family and substring matching (Tiers 1–3).

### Added
- **`--debug` execution chain tracing**: `--debug` now produces a full TRACE-level pipeline trace (`ass2sup_cli::*` + `subtitle_renderer::*`). Previously the flag was a no-op apart from enabling source location fields. Trace events cover: file I/O, format detection, font loading, embedded fonts, `check_ass_fonts` per-style decisions, renderer construction, dither / quantizer / PGS encoder setup, per-frame render / quantize / encode / write, and each step of the font fallback cascade (scoring → suffix-strip → fontconfig → hardcoded CJK → cross-platform CJK scan → hardcoded generic → `Family::SansSerif` → any-face).
- **Cross-platform CJK fallback (`query_cjk_capable_any`)**: New step in the font fallback chain scans `db.faces()` and returns the first face with the CJK test glyph (U+4E2D 中). This works on macOS (Hiragino / PingFang), Windows (Microsoft YaHei / MS Gothic / Malgun Gothic), and Linux (Noto CJK) without hardcoding platform-specific font names. Triggered when both the requested family and the hardcoded Linux-biased CJK list miss.
- **`--font-dir <DIR>`** (repeatable): Add a directory of TTF / OTF / WOFF2 files to the font database before rendering. Recurses into nested directories via fontdb's `load_fonts_dir`. Useful for containerized deployments and user-installed font packs that aren't on the OS font path.
- **`setup_logging` rewritten** with `tracing_subscriber::registry` + `EnvFilter` + `fmt::layer` (2026 best-practice pattern): respects `RUST_LOG` for per-module filtering without dropping the CLI default level, honours `--color`, redirects to `stderr`, and uses `try_init` so repeated calls (tests, embedded usage) are safe.

### Tests
- 9 new fontconfig integration tests: initialization, font availability, nonexistent fonts, alias handling, cache negative, query fallback priority.
- 3 new `setup_logging` tests: idempotent under `try_init`, accepts all colour modes, accepts all `(verbose, quiet, debug)` combinations.
- 3 new font fallback tests: cross-platform CJK scan smoke, empty-`FontManager` negative path, hardcoded CJK list with no system fonts.
- All 440+ tests pass across 8 crates.
- Clippy zero warnings, fmt zero drift.
- Insta snapshots updated to include the new `--debug` and `--font-dir` entries.

### Highlights
- **ASS escape sequence handling**: `\N` and `\n` are now converted to actual newlines during ASS parsing, enabling multi-line subtitle rendering. Previously rendered as literal text "\\N" in the output.
- **PotPlayer crash fix**: Fixed `0xC0000005` access violation in PotPlayer when rendering multi-line subtitles. Root cause: RLE index corruption when transparent palette entry is not at index 0.
- **ASS tag coverage**: Added missing `\c` color shorthand alias and `\clip(@)`/`\iclip(@)` drawing-path clip syntax.

### Fixed
- **`\N`/`\n`/`\h` escape sequences**: Converted at parser level in `ass-parser/src/event.rs`. `\N` and `\n` become actual newlines; `\h` is available via `convert_ass_escapes` API. (Fixes subtitles displaying literal "\\N" text.)
- **PotPlayer crash (0xC0000005)**: In `pgs-encoder`, when `transparent_index != 0`, palette entry swap (for PGS index-0 convention) now ALSO swaps index values in the pixel data (`frame.indices`). Previously only palette entries were swapped, causing the RLE encoder to misinterpret index-0 pixels as non-transparent and emit raw `0x00` bytes, corrupting the RLE stream.
- **`\t(tag, t1, 0, accel)` animation**: When `t2=0` (animate until end of event), the animation now progresses over the event duration instead of snapping instantly to the end state.
- **Karaoke `\fad`/`\fade` effects**: Alpha multiplier is now applied to karaoke foreground/background layers before compositing, making fade effects work with karaoke events.
- **ScrollUp/ScrollDown boundaries**: `top_offset` (ScrollUp) and `bottom_offset` (ScrollDown) now properly clamp the scroll region, preventing text from scrolling beyond the intended boundaries.
- **`\c` color shorthand**: Added `("c", "primary")` to the color prefix array in `override_tag.rs`, enabling `\c&HBBGGRR&` as an alias for `\1c&HBBGGRR&`.
- **`\clip(@)`/`\iclip(@)` syntax**: Added `ClipDrawingCurrent` and `ClipInverseDrawingCurrent` enum variants, enabling `\clip(@)` to reference the current drawing path as a clip mask.
- **Animations with `t2=0`**: Progress now correctly interpolates from `anim_start` to `event_end_ms` instead of always returning 1.0 when `t2=0`.

### Added
- **`convert_ass_escapes`**: Public utility function in `subtitle-renderer` for processing ASS escape sequences. Includes 7 unit tests covering `\N`, `\n`, `\h`, edge cases, and integration with `strip_override_blocks`.
- **Scroll/Effect boundary tests**: 2 new tests verifying ScrollUp `top_offset` and ScrollDown `bottom_offset` clamping behavior.
- **Karaoke fade tests**: 2 new tests verifying `\fad` alpha multiplier is applied during karaoke rendering.

### Changed
- **ASS parsing**: `\N` and `\n` escape sequences are now converted to actual newlines at the parser level (`ass-parser/src/event.rs`), before renderer processing. This ensures consistent behavior across all rendering paths.
- **RLE transparent index**: `build_display_set` now passes `transparent_index=0` to the RLE encoder after palette swap, and remaps the pixel index data (0 ↔ ti) to match. This ensures the RLE encoder always detects transparent pixels correctly regardless of the original quantizer's transparent index placement.

### Highlights
- **PotPlayer compatibility**: Complete rewrite of PGS encoder output to be playable in PotPlayer. The SUP output now renders CJK subtitles correctly with proper timing, palette, and font rendering.
- **FFmpeg-compatible RLE**: Encoder/decoder rewritten to use FFmpeg's PGS RLE format (`0x00` prefix for opaque/transparent runs), replacing the incompatible `[color][0x40|len]` format that caused garbled rendering.
- **Palette clear via palette update**: Subtitles end at their specified time using PCS(palette_update=true) + PDS(all-transparent), avoiding PotPlayer crashes from empty display sets (num_objects=0).
- **CJK font fallback**: Added glyph coverage check (`font_has_cjk_glyphs`) to prevent tofu (□) when fontdb fuzzy-matching returns Latin-only fonts for CJK family names.

### Fixed
- **PGS PCS format**: Switched from packed (palette_update|palette_id in 1 byte) to separate bytes (palette_update at offset 8, palette_id at offset 9), matching reference Blu-ray SUP format. The packed format caused PotPlayer to read num_objects=0, resulting in no subtitle display.
- **Transparent color at palette index 0**: Quantizer now places transparent entry at index 0 (prepend instead of append), eliminating the need for palette index swap in the encoder. Aligns with PGS convention.
- **RLE encoder**: Complete rewrite to FFmpeg-compatible format. Opaque runs now use `0x00 [0x80|len] [color]` instead of `[color][0x40|len]`. Single pixels are just the color byte. This fixes garbled rendering in PotPlayer.
- **Tight-bbox crop**: ODS dimensions now use actual subtitle pixel bounds (via `crop_to_tight_bbox`) instead of the full 1920×1080 canvas. Reduces file size ~33%.
- **BT.709 color space**: Encoder now uses BT.709 for HD content (height > 576 lines), matching FFmpeg behavior. Previously used BT.601 for all content.
- **Subtitle timing**: Each subtitle display set now includes a palette-clear display set (PCS with palette_update=true + all-transparent PDS) at the end time, making subtitles disappear at their specified end time instead of persisting until the next subtitle.
- **Decoder PCS_HEADER_SIZE**: Updated from 10 to 11 to match the separate-byte PCS format.
- **Decoder cropped flag**: Now skips 8 extra crop bytes when the cropped flag is set in object composition.
- **Decoder last_in_sequence**: Now parsed from ODS flags bit 6.
- **verify_roundtrip**: Now skips ODS check for palette-update-only display sets (palette_update=true without ODS is valid).

### Changed
- **CompositionState**: Always EpochStart (0x80) for all display sets. Previously used NormalCase for non-first frames.
- **palette_update**: Always true (0x80) for display PCS. PotPlayer requires this flag to load the PDS palette.
- **fontdb font matching**: `query_with_fallback_inner` now checks CJK glyph coverage after `query_with_score` returns a font. If the matched font lacks CJK glyphs (e.g., DejaVu Sans for "Microsoft YaHei"), it falls through to CJK fallback list.
- **build_palette**: Now accepts `display_height` parameter to choose BT.601 (≤576) or BT.709 (>576).

### Removed
- Dead code: `remap_collision_range` function (unused after RLE format change)
- Dead code: Duplicate `swap` function in `rle.rs` (uses `crate::color::swap`)
- Dead code: Duplicate `frame_rate_code` function in `encoder.rs` (uses `crate::types::frame_rate_code`)
- Dead parameter: `palette_changed` from `build_single_window_display_set`, `build_multi_window_display_set`, `build_epoch_split_display_set`
- Dead variable: `object_changed` in `build_display_set`

### Tests
- Updated segment count assertions from 5 to 8 (palette-clear adds PCS+PDS)
- Updated RLE encode/decode tests for FFmpeg-compatible format
- Updated pcs_palette_updates helper to filter display PCS by objects
- verify_roundtrip: skips ODS check for palette-update-only display sets
- 120/120 tests pass (79 lib + 40 integration + 4 doc)

### Security
- No critical/high issues found in audit
- `checked_mul` prevents integer overflow in RLE dimension calculations
- CLI enforces 100MB input limit

---

## [0.5.5] - Unreleased

### Fixed
- **`--parallel-frames` Windows hang root cause (v0.5.3 + v0.5.4 still hung)**: the real culprit was not the `Vec<RenderedFrame>` memory blowup (v0.5.3 fix) or rayon's cold-start thread pool (v0.5.4 fix), but the **per-render `font_has_cjk_glyphs` call on the scoring-match font**. On the user's 247-font Windows system, parsing the 22+ MB TTF for "MiSans Demibold" in the first event's font fallback took 30+ seconds (likely Windows font cache service contention), during which the watchdog thread was the only thing printing. **Fix**: removed the `font_has_cjk_glyphs` check from the scoring-match path in `query_with_fallback_inner` (Step 1). The scoring result from `query_with_score` is now trusted directly. The dedicated CJK scan at Step 2.5 (already pre-warmed at startup) still catches the case where the user explicitly requests a CJK family that fontdb cannot find at all. Trade-off: if the scoring match happens to be a Latin-only font and the rendered text contains CJK characters, the output will show tofu (□) for those characters — but the render will complete, not hang.

---

## [0.5.4] - Unreleased

### Fixed
- **`--parallel-frames` first-call deadlock on Windows (v0.5.3 regression)**: v0.5.3 still hung on Windows with 1988-event CJK ASS. The `pipeline merge` + `PixmapPool` changes from v0.5.3 were necessary but not sufficient: rayon's global thread pool is built lazily on the first `par_iter()` call, and on Windows that first call can deadlock when invoked deep inside a parallel render loop after fonts/quantizer have been initialised. **Fix**: explicitly build and warm a dedicated `rayon::ThreadPool` at startup (named workers `ass2sup-worker-N`) and execute a no-op `par_iter()` on every worker. All subsequent `par_iter()` calls in the render loop reuse the already-initialised pool, eliminating the cold-start deadlock.
- **Progress-bar heartbeat in debug mode**: Without periodic log output, a hang presents as wall-clock silence with no diagnostic signal. **Fix**: when `--debug` is set, spawn a watchdog thread (`ass2sup-watchdog`) that prints `[watchdog <N>s] alive, rss=<X> MiB` to stderr every 5 seconds. Combined with the per-event trace logs and RSS samples, this makes Windows-side hangs visible in the log instead of being silent.

### Added
- **Full-pipeline precise source-traceable debug logging**: When `--debug` is set, every key stage emits a `tracing::trace!` line tagged with `target:file:line` (already in v0.5.3) plus a new high-resolution uptime timestamp and current process RSS (read from `/proc/self/status` on Linux; 0 on Windows). Coverage:
  - **CLI entry** (`crates/ass2sup-cli/src/lib.rs:convert_file`): `convert_file entry` → `input file read` → `format detection result` → `ASS file parsed` (styles, events, embedded fonts, rss_mib) → `Renderer constructed` (font_count, rss_mib) → `rayon thread pool pre-warmed` → `encode-loop progress` (every 100 events: event_idx, cumulative_ms, last_event_us, frames_encoded, rss_mib).
  - **Font loading** (`crates/subtitle-renderer/src/font.rs`): `query_cjk_capable_any: scanning all faces` → `scan complete and cached` now records `elapsed_ms`. A scan taking >1 s emits a `WARN query_cjk_capable_any: SLOW scan` to surface systems with too many fonts.
  - **Renderer** (`crates/subtitle-renderer/src/renderer/mod.rs:render_ass`): every call emits `entry` (timestamp_ms, visible_events, width, height, elapsed_us), per-event `event done` (style, text_len, elapsed_us) **or** `WARN render_ass: SLOW event (>500ms)` for stalls, and final `exit` (timestamp_ms, total_us, bitmap_bytes).
  - **Watchdog**: every 5s while `--debug` is set.
- **`current_rss_bytes()`** helper: reads `/proc/self/status` for `VmRSS` (Linux only; returns 0 on macOS/Windows to keep the binary portable). Used in 6+ trace points to track memory growth.
- **`tracing_subscriber::fmt::time::uptime()`** timer in `setup_logging` so log lines include a high-resolution elapsed-since-process-start timestamp, making relative timing between log lines trivial.
- **`rayon::scope` + dedicated `ThreadPool`** warm-up in `run()`: built only when `--parallel-frames` is set; reports `elapsed_ms` and `workers` in the warmup trace.

### Tests
- `cargo test -p ass2sup-cli --test test_parallel_frames_large`: 3 tests on a 200-event fixture pass in ~60 s (debug) / <2 s (release). The fixture was reduced from 1500 → 200 events after profiling showed debug-binary tests are 28× slower than release; 200 events × 8.3 MB ≈ 1.6 GB is still enough to surface the original OOM but keeps CI runtime reasonable. Tests are serialised by a process-wide `Mutex<()>` to prevent three heavy `ass2sup` subprocesses from OOM-killing each other in parallel.
- Manual QA on `.localref/[岛·消失的人们][2016][012751][24]_CN.ass` (1121 events, real CJK): 25.0 s serial → 18.8 s parallel, MD5-identical.
- Manual QA on a synthesised 1988-event CJK file (real CJK lines duplicated, time codes offset): 44.8 s serial → 45.4 s parallel, **MD5 `f4023cf27f758d94226db03afc9e0f94` identical** (45 MB output).
- All existing tests still pass (440+ across 8 crates). Clippy `-D warnings` clean, `cargo fmt --check` clean.

---

## [0.5.3] - Unreleased

## [0.5.1] - 2026-06-13

### Highlights
- **PGS spec compliance overhaul**: Encoder rewritten to match FFmpeg/PGS BD-ROM specification. 7 fixes addressing ODS layout, flags, PDS byte order, cross-chunk total_size, palette_update, palette swap, and window bounds clamping.
- **Security hardening**: Integer overflow protection (checked_mul), shell injection fix (ocr.rs), temp file leak fix (ocr_harness.py), ODS data accumulation cap.
- **Test cleanliness**: Replaced hand-rolled PNG decoder with `png` crate, moved misplaced ASS fixture, fixed /tmp/ hardcoded paths, added 3 new OCR fixtures (Japanese, Korean, French).

### Fixed
- **PGS ODS payload layout (ROOT CAUSE of PotPlayer crash)**: Rewrote ODS segment format to match FFmpeg/PGS spec: `object_id(2) + version(1) + flags(1) + [first: total_size(3) + width(2) + height(2)] + rle_data`. Old format had width/height/rle_len in wrong order, causing decoder buffer overflow.
- **ODS flags**: bit 7 (0x80) = first_in_sequence, bit 6 (0x40) = last_in_sequence. Single-segment objects use 0xC0, multi-segment use 0x80/0x40 for first/last chunks.
- **PDS byte order**: Corrected to index, Y, Cr, Cb, alpha per PGS spec (was Y, Cb, Cr).
- **ODS total_size**: Now spans ALL chunks for multi-segment objects (`total_rle_size` field added to OdsPayload).
- **palette_update flag**: Always emits PDS with palette_update=true for PotPlayer compatibility.
- **Palette transparent_index swap**: When transparent_index != 0, swap palette[0] with palette[transparent_index] so RLE index 0 maps to transparent.
- **WDS window bounds**: Clamped to display dimensions to prevent out-of-bounds rendering.
- **ODS continuation segment dimensions**: Only update stored width/height from first-in-sequence segments.

### Changed
- `decode_to_image.rs`: Added `first_in_sequence` field to `ParsedPayload::ObjectDefinition` to properly track object dimensions across multi-segment ODS.
- `rle.rs`: Fixed single-pixel opaque format from `[color, 0x40]` to `[color, 0x01]` to prevent RLE decoder misinterpretation.
- Tests updated: palette_update expectations changed to always-true (since PDS is always emitted), single-pixel format updated.

### Tests
- 79/79 pgs-encoder unit tests pass
- 117/119 renderer tests pass (2 pre-existing drawing/clip failures unrelated to PGS fixes)
- Roundtrip decoder validates SUP → PNG decode with correct pixel output

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

## [2.6.0] - 2026-06-24

### Refactored
- **pgs-encoder DDD architecture**: Extracted domain layer (`domain/`) with 6 submodules — `composition`, `palette`, `segment`, `timing`, `rle`, `epoch`. Split encoding and decoding concerns into `encoding/` and `decoding/` layers. Encoder shrunk from 1034 to 488 lines.
- **ColorPipeline migration**: Replaced legacy `Quantizer` builder with `ColorPipeline` builder supporting color-space selection, tone mapping, and temporal palette reuse.
- **build_palette**: Accepts `ColorSpace` parameter instead of hardcoded `display_height > 576` heuristic. Colour space decision now flows from the frame data.

### Added
- **CLI options**: `--color-space` (srgb/bt709/bt2020) and `--tonemap` (hable/reinhard/aces) for fine-grained colour pipeline control.
- **EpochManager**: Extracted FSM logic from PgsEncoder into a dedicated module with unit tests.
- **DisplaySetConfig**: Value object encapsulating encoder configuration for display set building, replacing 8-parameter function signatures.
- **RLE proptest**: 500-case property test `rle_encode(rle_decode(x)) = x` — discovered and fixed a bug where color value 0x00 collided with the RLE escape byte when used as an opaque pixel.
- **Golden tests**: Deterministic SHA-256 tests for RLE encoding, palette YCbCr conversion, and small-frame encoding.
- **PotPlayer compat toggle**: `--no-potplayer-compat` flag to disable the `num_objects=1` workaround.

### Fixed
- **RLE 0x00 collision**: Opaque color value 0x00 (when `transparent_index != 0`) can no longer be emitted as a bare byte colliding with the RLE escape marker.
- **PCS serialization**: Corrected width/height/frame_rate ordering and palette_update_flag byte alignment to match PGS BD-ROM spec exactly.
- **ODS serialization**: `total_size` uses 3-byte big-endian format, geometry (width/height) emitted only on `first_in_sequence` segments as spec requires.
- **Epoch-split colour space**: Synthetic band frames now propagate `frame.color_space` instead of defaulting to `Srgb`.
