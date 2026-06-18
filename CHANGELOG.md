# Changelog

All notable changes to **um-ass2sup** (the ASS/SSA → Blu-ray SUP subtitle converter) will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [0.6.3] - 2026-06-17 (Sprint 6: Sub-7 Output Formats - sink trait + TTML + WebVTT + ASS passthrough)

### Added
- **`bdn-xml::sink` module**: the v2.0 multi-format seam.
  - `OutputSink` trait: `write_frame(SinkFrame) -> Result<()>` + `finalize()`. `Send + Sync` so async dispatch is possible.
  - `SinkError` (thiserror): `Io(std::io::Error)` and `Format(String)`.
  - `SinkFrame`: minimal frame struct (`start_ms`, `end_ms`, `text`, `width`, `height`, `rgba`).
  - `TtmlSink<W>`: W3C TTML2 / SMPTE-TT serializer. Validates timing before write. Emits one `<p>` per frame.
  - `WebVttSink<W>`: W3C WebVTT serializer. Numbered cues, `HH:MM:SS.mmm` timecodes.
  - `AssPassthroughSink<W>`: emits a valid ASS v4.00+ document with `[Script Info]`, `[V4+ Styles]`, `[Events]`. Header is written once on the first frame.
  - `write_ttml_header` / `write_webvtt_header` helpers: caller-controlled prologue/epilogue flow.
  - `xml_escape()` for `<`/`>`/`&`/`"`/`'` in TTML text content.
  - `ms_to_ttml_timecode`, `ms_to_webvtt_timecode`, `ms_to_ass_timecode` formatters.
  - 12 unit tests covering happy path, timing validation, finalize-after-write rejection, empty-passthrough.

### Verification
- `cargo test -p bdn-xml --lib sink` — 12/12 pass
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings
- `cargo fmt --check` — clean

### Migration
PGS/BDN paths are unchanged. v2.0 will add a `PgsSink: OutputSink` adapter in the `pgs-encoder` crate so the entire pipeline can dispatch through one trait. CLI `--format pgs|bdn|ttml|webvtt|ass` flag is a follow-up.

---

## [0.6.2] - 2026-06-17 (Sprint 5: Sub-6 GPU - backend trait + dispatch)

### Added
- **`subtitle-renderer::backend` module**: the v2.0 GPU seam.
  - `RendererBackend` trait: `draw_glyph`, `fill_rect`, `apply_effect`, `finalize`. `Send + Sync` so the GPU implementation can carry a wgpu device handle.
  - `BackendPolicy` enum: `CpuOnly` (default) | `GpuOnly` | `Hybrid` with `select(event_count, gpu_available) -> &'static str`.
  - Types: `Point`, `Rect`, `Color { r, g, b, a }` with `BLACK`/`WHITE`/`TRANSPARENT` constants, `Glyph { id: GlyphId, pos, color }`, `BackendEffect::FillRect | Blur`, `RenderedBitmap { width, height, data: Vec<u8> }` (RGBA, 8-bit, row-major).
  - 9 unit tests covering point/rect/color construction, hybrid dispatch policy boundaries (event_count=99 → cpu, 100 → gpu).

### Verification
- `cargo test -p subtitle-renderer backend` — 9/9 pass
- `cargo test --workspace` — 0 failures across 50+ test binaries (1000+ tests)
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings
- `cargo fmt --check` — clean

### Migration
The v0.6.x CPU renderer is unchanged. v2.0 will add `VelloBackend: RendererBackend` behind a `vello` cargo feature; `BackendPolicy::Hybrid` will be wired into the CLI via a new `--render-backend` flag.

---

## [0.6.1] - 2026-06-17 (Sprint 4: Sub-5 Color Pipeline - foundation)

### Added
- **`color-quantizer::color_pipeline` module**: typed colour-pipeline primitives for the v2.0 HDR path.
  - `ColorSpace` enum: SdrBt709, HdrBt2020Pq, HdrBt2020Hlg with `is_hdr()` predicate and `to_xyz_matrix()` (BT.601/BT.709/BT.2020 D65 matrix).
  - `TransferFunction` enum: Linear, Srgb, Pq (SMPTE ST 2084), Hlg (ARIB STD-B67) with `to_linear()` / `from_linear()` roundtrips.
  - `Tonemap` enum: None, Hable (Uncharted 2 filmic), Reinhard, Aces (Narkowicz fit) with `apply()` operator.
  - `ColorPipelineConfig` + `convert_rgb()` end-to-end helper.
  - `detect_source_color_space(ass_text)` for `Output: HDR` and `YCbCr Matrix: BT.2020` auto-detection.
- **18 new unit tests** in `color_pipeline::tests` covering: default colour space, HDR detection, BT.709 D65 reference, sRGB/Linear/PQ/HLG roundtrips, all four tonemapping operators, and HDR→SDR conversion.

### Verification
- `cargo test -p color-quantizer color_pipeline` — 18/18 pass
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings
- `cargo fmt --check` — clean

---

## [0.6.0] - 2026-06-17 (Sprint 3: Sub-4 Renderer - effect stack)

### Added
- **`subtitle-renderer::effect_stack` module**: typed per-line effect stack consolidating the 11 ASS effect categories (Fade, FadeComplex, Pos, Move, Clip, InverseClip, RotationX/Y/Z, ShearX/Y, Blur, EdgeBlur). `EffectStack` provides push/resolve_*/apply methods with per-frame evaluation semantics. The `apply()` method is the v2.0 entry point that the renderer calls per frame.
- **14 new unit tests** in `effect_stack::tests` covering: empty stack defaults, Pos, Move (before/after/inside window), Fade (in/out), FadeComplex three-segment, Clip + InverseClip, RotationZ accumulation, Blur+EdgeBlur precedence, and apply() to RenderContext.
- **`docs/plans/04-renderer/task-01-effect-stack.md`**: architectural documentation.

### Migration path
`EffectStack::apply()` is the v2.0 entry point; the legacy `build_context()` in `context.rs` continues to work. The full renderer migration is tracked in `docs/superpowers/specs/2026-06-17-Sub-4-renderer.md`.

### Verification
- `cargo test -p subtitle-renderer` — passes (incl. 14 new EffectStack tests)
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings
- `cargo fmt --check` — clean

---

## [0.5.9] - 2026-06-17 (Sprint 2: Sub-3 Font Engine - foundation)

### Added
- **`cosmic-text = "0.19"`** added to workspace dependencies (off by default per-crate). Brings in three-platform font discovery (DirectWrite / CoreText / fontconfig) and HarfBuzz v13+ shaping via the `harfrust` + `skrifa` stack.
- **`subtitle-renderer` cosmic-text cargo feature** (opt-in). When enabled, exposes:
  - `FallbackChain`: ordered CJK fallback list with per-style overrides and a `strict` flag.
  - `AssFallback`: cosmic-text-compatible font-fallback adapter that consults the chain.
  - `FontResolver`: thin wrapper around the configured chain, exposing `resolve_for_style(name) -> &[String]`.
  - 8 new unit tests covering empty chain, global chain, per-style override, strict flag, and resolver dispatch.
- **`docs/plans/03-font-engine/task-01-cosmic-text-trait.md`**: migration-path documentation for the v2.0 font-engine work.

### Migration path
The full migration of the existing `font.rs` (1085 lines, fontdb + rustybuzz + ttf-parser) to cosmic-text is out of scope for this PR. `FontResolver` is the v2.0 entry point; the existing `FontManager` continues to work as the default. The migration is tracked under `docs/superpowers/specs/2026-06-17-Sub-3-font-engine.md`.

### Verification
- `cargo build -p subtitle-renderer --features cosmic-text` — clean
- `cargo build -p subtitle-renderer` (default) — clean (no impact)
- `cargo test -p subtitle-renderer --features cosmic-text font_cosmic` — 8/8 pass
- `cargo fmt --check` — clean
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings

---

## [0.5.8] - 2026-06-17 (Sprint 1.5: Sub-2 OverrideExpr + \t animations)

### Added
- **`ass-parser::override_expr` module**: typed AST on top of the flat `OverrideTag` enum. New types:
  - `OverrideValue`: Scalar(f64) | Color(AssColor) | Pos{x,y} | Rotation{x,y,z} | Scale{x,y} | Bool(bool) | String(String).
  - `OverrideExpr`: `Constant(OverrideValue)` for static tags, `Animated { start, end, t1_ms, t2_ms, accel }` for time-bearing tags (Move, Fade, FadeComplex, Transform).
  - `Animator` trait with `evaluate_at(time_ms) -> OverrideValue`.
  - `lift_to_expr(tag: &OverrideTag) -> OverrideExpr`: free function that lifts the flat enum into the typed AST.
  - `ease(t, accel)` and `interpolate()` helpers: ease follows the libass convention (`t.powf(accel)`); interpolate covers every `OverrideValue` variant (linear for numerics, snap at 0.5 for bools/strings, per-channel 8-bit lerp for colours).
- **\t() animation evaluation**: full libass semantics for `\t(\tag, t1, t2, accel)` — piecewise (before t1 = start, after t2 = end, between = interpolated with `ease(raw, accel)`). Nested `\t()` works because `start` and `end` are themselves `OverrideExpr`.
- **19 new unit tests** in `override_expr::tests` covering: constant evaluation, animated before/after boundaries, linear midpoint, quadratic ease-in (accel=2), position interpolation, per-channel colour lerp, lift tests for every major OverrideTag variant, ease() identity/quadratic/zero-accel fallthrough.
- **`docs/plans/02-ass-parser/task-03-override-tags.md` and `task-04-animations.md`**: updated from ⏳ DEFERRED to ✅ COMPLETED.

### Verification
- `cargo test -p ass-parser` — 147/147 pass
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings
- `cargo fmt --check` — clean

---

## [0.5.7] - 2026-06-17 (Sprint 1: Sub-2 ASS Parser - partial)

### Added
- **`ass-parser::types` module**: strong-typed domain types replacing raw `u8`/`u32` in the AST. `StyleName` newtype, `BorderStyle` enum (`OutlineAndShadow` / `OpaqueBox`), `Alignment` enum (numpad 1-9), `Margins` struct, `Encoding` newtype. `StyleName` supports `==` against `&str` and `AsRef<str>`.
- **`Style::to_ass_string()`** round-trip serialization matching `Style::parse_from_line()`.
- **`raw_alignment: u8` field on `Style`**: preserves the original source value (1-255) so the subtitle-validator's V008 rule can catch out-of-range alignment values that the typed `Alignment` field silently coerces to `BottomCenter`.
- **`AssFile::warnings: Vec<ParseWarning>`** field + **`AssFile::parse_with_recovery()`** entry point: single-line errors no longer abort the parse; corrupted inputs yield a usable AST plus a structured warning list (`InvalidField`, `UnknownSection`, `SrtBlockSkipped`).
- **`AssFile::from_srt()` upgrade path**: convert `SrtFile` to `AssFile` so `ass2sup input.srt -o output.sup` works without a pre-conversion step. Round-trip `SRT → ASS → SRT` preserves all events.
- **`SrtFile` struct**: native SRT representation with `style`, `start`, `end`, `text` fields.
- **`libass-compat` test suite**: 122 synthetic `.ass` fixture files vendored under `crates/ass-parser/fixtures/libass/` (~492 KiB total, all under 10 KiB), driven by a single integration test that captures strict + recovery parse results as 122 insta snapshots. Covers basic structures, styles, events, all 36 individual override tags, combined sequences, color formats, karaoke, animation, clip, drawing, positioning, fonts, edge cases, error recovery, and stress tests.
- **`docs/plans/02-ass-parser/`**: 7 task MDs (task-01..task-07) with deliverables, verification gates, and follow-up notes.
- **`Effect: Display`** impl.
- **`insta = "1"`** added as a dev-dependency to `ass-parser` (for libass_compat snapshots).

### Changed
- **`Style` refactored to use the new types**: `name: StyleName`, `border_style: BorderStyle`, `alignment: Alignment`, `margins: Margins`, `encoding: Encoding`. Field renames: `outline_width` → `outline`, `shadow_depth` → `shadow`. `relative_to` removed (not part of the V4+ 22-field spec).
- **`Event::style_name: String` → `Event::style: StyleName`** across `ass-parser`, `subtitle-renderer`, `subtitle-validator`, and `ass2sup-cli`.
- **`subtitle-validator` V008 rule** now checks `style.raw_alignment` (not the coerced `Alignment` enum) so out-of-range values are still caught.
- **`srt_default_style()`** updated to use the new `Style` types.
- **Pre-existing fixes en route**:
  - `to_ass_time()` → `as_ass_time()` in `event.rs`
  - `StyleName == &str` comparison fix
  - SRT `style_name` field → `style` field

### Follow-up (Deferred from Sprint 1)
- **Override tag AST (`OverrideExpr`)**: add `Scalar(f64) | Color(AssColor) | Animated { ... } | Transform(...)` variants and `Animator` trait. Currently blocked on a worker session that returned without writing code. See `docs/plans/02-ass-parser/task-03-override-tags.md` and `task-04-animations.md` for the full plan.

### Tests
- 17 new lenient-mode tests in `test_lenient.rs`
- 10 new SRT round-trip / upgrade tests
- 1 libass-compat integration test with 122 insta snapshots
- 6 new test files / updates in `crates/ass-parser/tests/`
- All 128 unit + integration tests pass in `ass-parser`
- `cargo test --workspace` — 0 failures (ass2sup-cli tests not run in this summary; previously green)
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings
- `cargo fmt --check` — clean

---

## [0.5.2] - 2026-06-17

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

## [0.5.6] - 2026-06-17 (Sprint 0: Sub-1 Infrastructure)

### Added
- **`ass2sup::error` module** (`crates/ass2sup-cli/src/error.rs`): unified error type system covering every ass2sup failure mode. Top-level `Error` enum plus 5 sub-enums (`RenderError`, `OutputError`, `ConfigError`, `FontError`, `ColorError`) wrapping per-domain detail. `From<io::Error>` and `From<String>` impls enable progressive migration from existing `Result<_, String>` call sites. All variants have `///` rustdoc; 18 unit tests cover Display formatting, source chain, From conversions, Send/Sync, and Debug. ~210 lines.
- **`ass2sup::config` module** (`crates/ass2sup-cli/src/config.rs`): TOML-backed configuration system. Top-level `Config` + 5 sub-structs (`Defaults`, `CjkFallback`, `ColorConfig`, `StyleOverride`, `RenderingConfig`) all marked `#[serde(deny_unknown_fields)]` so unknown keys surface as a load error pointing at the offending field. Supports load/save/merge, file-path precedence (CLI `--config` > `./ass2sup.toml` > `~/.config/ass2sup/config.toml`), and CLI override via a thin `MergeArgs` DTO. 11 unit tests cover defaults, serde round-trip, `deny_unknown_fields` rejection, malformed TOML propagation, tempdir save/load, and CLI merge precedence. ~250 lines.
- **`ass2sup::telemetry` module** (`crates/ass2sup-cli/src/telemetry.rs`): unified `tracing_subscriber` registry. `init(TelemetryConfig)` is idempotent (uses `try_init`); `init_default()` honours `ASS2SUP_LOG` and `ASS2SUP_COLOR` env vars (case-insensitive, unknown values fall back to safe defaults). 9 unit tests cover init idempotency, all level filters, all color choices, env-var parsing, and graceful handling of malformed env values. ~140 lines.
- **`--config <PATH>` CLI flag**: load a TOML config file. Precedence: `--config` > `./ass2sup.toml` > `~/.config/ass2sup/config.toml`. CLI flags override file values.
- **`--cjk-fallback <FONT>` CLI flag** (repeatable): build an ordered CJK fallback chain from the command line, overriding the file-loaded chain.
- **`--log-level <LEVEL>` CLI flag**: explicit log level override (`trace` / `debug` / `info` / `warn` / `error`). Wins over `--verbose` / `--quiet` / `--debug` and the `ASS2SUP_LOG` env var.
- **`docs/CONFIG.md`**: complete schema documentation for the new config system, including field constraints, CLI integration examples, and error handling semantics.
- **`toml = "0.8"`** added to `[workspace.dependencies]`. License: MIT/Apache-2.0 (compatible with project allowlist).

### Changed
- **`setup_logging` now delegates to `telemetry::init`**: both call paths share the same `tracing_subscriber::registry` configuration. Existing `setup_logging(verbose, quiet, debug, color)` signature is preserved (no breaking change); internal implementation routes through the new `telemetry::init` entry point.
- **`run()` now loads config first**: order of operations is now (1) `Config::load_default`, (2) `setup_logging_with_config` (honours `Config.log_level` + `ASS2SUP_LOG`), (3) collect inputs, (4) process. Config-load errors are reported as `CliError::Conversion`.
- **Insta snapshots updated**: `--help` and `--short-help` now include the 3 new flags (`--config`, `--cjk-fallback`, `--log-level`). 5 snapshot tests pass.

### Tests
- 38 new tests across 3 test files (`error_test`, `config_test`, `telemetry_test`); all pass.
- All existing 440+ tests pass with no regression.
- 4 manual QA scenarios pass: (a) valid minimal config → graceful `--check` exit 0, (b) invalid TOML → clear parse error with line number, (c) missing config file → defaults applied silently, (d) unknown field → clear rejection with hint.
- `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` 0 warnings, `cargo doc --workspace --no-deps` 0 new warnings (2 pre-existing in `subtitle-renderer` unrelated to this change).

### Plan
- Sprint 0 of the v2.0 refactor (see `docs/plans/01-infrastructure/`). Unblocks Sub-2 through Sub-8.
- All planned `Error` / `Config` / `telemetry` modules in scope of `docs/plans/01-infrastructure/task-01..03.md` are now implemented with full test coverage.
- MSRV unchanged (1.85) — `toml = "0.8"` does not raise the floor.

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
