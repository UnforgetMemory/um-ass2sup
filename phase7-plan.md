# Phase 7 Execution Plan

## Overview
7 directions, 12 R-fixes, organized into 5 parallel waves over ~10 weeks.

## Wave 1 (Week 1-2) — Bug Fixes + Foundation [3 PARALLEL AGENTS]

### Agent A: Critical Bug Fixes (R.1-R.5, R.8)
- **Category**: `deep`
- **Files**: `renderer.rs`, `context.rs`, `effects.rs`
- **Tasks**:
  1. R.1: Fix `\iclip` — add `clip_inverse_enabled` to context, invert clip logic in render_event
  2. R.2: Fix `\move` — pass `timestamp_ms` to `build_context`, implement time interpolation
  3. R.3: Fix `\fad`/`\fade` — add alpha time calculation in render_event
  4. R.4: Fix rotation/scale/shear — implement affine transform in render_event (tiny_skia Transform)
  5. R.5: Fix `\t` Transform — dual-context interpolation architecture
  6. R.8: Fix Rotation origin — separate `\frz` from `\org`
  7. R.7: Fix `\r` Reset — style reset in build_context

### Agent B: Coordinate Transform System (T1.1, T1.3, T1.5)
- **Category**: `deep`
- **Files**: `renderer.rs`, `effects.rs`, `rasterizer.rs`
- **Tasks**:
  1. T1.1: Affine transform system — rotate/scale/shear using tiny_skia Transform
  2. T1.3: `\iclip` inverted clipping (depends on R.1 fix)
  3. T1.5: Shadow blur enhancement — `apply_shadow_with_blur()`
  4. T1.5: Independent X/Y outline width — `outline_width_x/y` fields
  5. R.6: Shadow blur + `\xbord`/`\ybord` independent

### Agent C: Test Infrastructure + Error Recovery
- **Category**: `deep`
- **Files**: `tests/fixtures/`, `lib.rs` (ass-parser), `main.rs` (cli)
- **Tasks**:
  1. Collect 10+ real ASS test files (karaoke, effects, multi-style)
  2. Create `tests/fixtures/` directory with `.ass` + `.expected.json`
  3. T7.1: Implement `parse_lenient()` in ass-parser — collect errors, skip bad events
  4. T7.2: Bad frame detection — `render_ass` returns `Option`, generate transparent fallback
  5. T7.3: CLI error reporting with file/line/context
  6. R.11: Fix empty pixel set panic in quantizer

## Wave 2 (Week 3-4) — Time Animation System [SERIAL — most complex]

### Agent A: Time Animation (T1.2a-f)
- **Category**: `deep`
- **Files**: `renderer.rs`, `context.rs`
- **Dependencies**: Wave 1 Agent A (R.2, R.3, R.5 fixes)
- **Tasks**:
  1. T1.2a: `\move` time interpolation (x1,y1→x2,y2 over t1..t2)
  2. T1.2b: `\fad` fade in/out (alpha gradient over duration_in/duration_out)
  3. T1.2c: `\fade` complex 3-segment alpha (alpha_start→alpha_mid→alpha_end)
  4. T1.2d: `\t` attribute animation — dual-context interpolation with accel
  5. T1.2e: `\clip` animated clipping (time-varying clip region)
  6. T1.2f: `parse_nested_tag()` for `\t` inner tag string parsing
  7. T1.4: Text wrapping — `\q` wrap_style implementation

## Wave 3 (Week 5-6) — Karaoke + PGS + Fonts [3 PARALLEL AGENTS]

### Agent A: Karaoke Animation (T5.1-T5.2)
- **Category**: `deep`
- **Files**: `renderer.rs`, new `karaoke.rs`
- **Dependencies**: Wave 2 (time animation system)
- **Tasks**:
  1. T5.1: `\k` instant switch — render full text in secondary, current syllable in primary
  2. T5.1: `\kf` fill — left-to-right clip sweep using pixmap clipping
  3. T5.1: `\ko` outline — highlight outline effect
  4. T5.1: `\kt` timing — per-syllable timing control
  5. T5.2: Syllable boundary calculation from karaoke_segments

### Agent B: PGS Compatibility (T2.1-T2.4)
- **Category**: `deep`
- **Files**: `encoder.rs`, `types.rs`
- **Tasks**:
  1. T2.1: NTSC-aware PTS — `ms_to_90khz_ntsc()` for 23.976/29.97
  2. T2.2: Multi-window mode — split large objects into max 2 windows
  3. T2.3: PGS decoder buffer limit — split ODS if RLE > ~1.5MB
  4. T2.4: Epoch continuity — NormalCase composition state for static subtitles
  5. R.10: Fix PTS drift

### Agent C: Font System (T3.1-T3.3)
- **Category**: `deep`
- **Files**: `font.rs`
- **Tasks**:
  1. T3.1: Fontconfig weight/width matching (fontdb → weight mapping)
  2. T3.2: Embedded font support — parse ASS `[Fonts]` section, load base64 font data
  3. T3.3: Font fallback chain — try multiple fonts before giving up

## Wave 4 (Week 7-8) — Performance + Verification [2 PARALLEL AGENTS]

### Agent A: Performance (T4.1-T4.3)
- **Category**: `deep`
- **Files**: `renderer.rs`, `lib.rs` (quantizer)
- **Dependencies**: Wave 1-3 complete
- **Tasks**:
  1. T4.1: Frame cache — hash (event set + timestamp) → cached RenderedFrame
  2. T4.2: Rayon parallel rendering — independent events rendered in parallel
  3. T4.3: Palette reuse between frames — track previous palette, skip re-quantization

### Agent B: E2E Verification (T6.1-T6.3)
- **Category**: `deep`
- **Files**: `tests/`, CI config
- **Tasks**:
  1. T6.1: Expand test ASS file collection to 50+
  2. T6.2: SUP playback verification — ffmpeg extraction + pHash comparison
  3. T6.3: Golden file tests — deterministic input → byte-exact SUP output

## Wave 5 (Week 9-10) — Integration + Polish

### All Agents: Bug fixes, documentation, release prep
- Fix all remaining issues from Waves 1-4
- Update CLI with new options (forced subtitle, NTSC mode, etc.)
- Write user documentation
- Tag v0.2.0 release

## Dependencies Graph
```
Wave 1 (parallel):
  Agent A (R-fixes) ─────────┐
  Agent B (transforms) ──────┤
  Agent C (tests+recovery) ──┤
                             ▼
Wave 2 (serial):
  Agent A (time animation) ──┐
                             ▼
Wave 3 (parallel):
  Agent A (karaoke) ─────────┐
  Agent B (PGS compat) ──────┤
  Agent C (fonts) ───────────┤
                             ▼
Wave 4 (parallel):
  Agent A (performance) ─────┐
  Agent B (E2E verification)─┤
                             ▼
Wave 5 (integration + release)
```

## Risk Mitigation
- \t Transform: start with numeric field interpolation, defer string field switching
- PGS buffer: reference SUPer Python impl for splitting strategy
- Karaoke \kf: use pixmap clip instead of per-pixel clipping for performance
- Affine transform: bilinear interpolation + optional supersampling
- Real ASS files: collect 50+ from open source repos for regression testing
