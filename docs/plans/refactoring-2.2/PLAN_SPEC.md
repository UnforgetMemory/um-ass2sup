# 2.2 Plan: subtitle-renderer rebuild + PGS 编码器补完

## Context

Rebuild `crates/subtitle-renderer/` to consume `ass_core::SubtitleDocument` instead of `ass_parser::AssFile`, handle all 53 `OverrideTag` variants in `build_context`, and split the 2362-line monolith `renderer/mod.rs` into modular tag-handler submodules — following the same 9-module pattern as `ass-core/override_tag/`.

**Key gaps found during exploration:**
- build_context matches ~30/53 OverrideTag variants; 7 are genuinely missing (FontSizeRelative, ScaleReset, ClipDrawingCurrent, ClipInverseDrawingCurrent, Karaoke tag, Charset, Unknown)
- 16 more need updated Rust types for ass_core's `TaggedOverride` wrapper
- `animation.rs` uses `ass_parser::parse_override_tag()` — needs to switch to `ass_core`
- Font/shaper/rasterizer/effects/transform modules are largely independent (type imports only)
- All downstream consumers (`ass2sup-cli`, benches, tests) still use `ass_parser` types

**Assumptions (confirmed by user):**
- FontSizeRelative: relative add to current font_size
- ScaleReset: reset to style's scale_x/scale_y via StyleRef
- ClipDrawingCurrent/ClipInverseDrawingCurrent: implement properly with path-based clipping
- Test snapshots: hex RGBA in unit tests, PNG in integration tests
- Fps: added as extra parameter, timestamp_ms stays primary time source

## Topology Lock (Phase 1 — no code yet)

| ID | Component | Outcome | Evidence (code location) |
|----|-----------|---------|--------------------------|
| T1 | Cargo.toml + lib.rs | ass-parser → ass-core dep swap; updated public exports | `Cargo.toml`, `src/lib.rs` |
| T2 | RenderContext | Fields for all 53 tags; RenderConfig immutable | `src/context.rs` |
| T3 | render_ass pipeline | `&SubtitleDocument` + `Fps` input; event-level catch_unwind; margin Option→f32 normalization | `src/renderer/mod.rs` |
| T4 | build_context modules | 10 handlers (position/font/color/border/geometry/clip/karaoke/reset/transform/misc) + orchestrator | `src/renderer/context/*.rs` |
| T5 | Support modules | animation/compositing/drawing/text_layout/effects/karaoke updated to ass_core types | `src/renderer/animation.rs` etc. |
| T6 | Support infra | font/shaper/rasterizer/transform — type imports only | `src/font.rs`, `src/shaper.rs` etc. |
| T7 | Error module | EventError type; render_ass returns OK always (errors logged) | `src/error.rs` |
| T8 | Test suite | All tests pass with ass_core types; 53-tag coverage tests | `tests/*.rs`, `src/renderer/context/*.rs` |

## Task Dependency Graph

| Task | Depends On | Reason |
|------|------------|--------|
| 1. Cargo.toml + ass_core dep | None | Foundation — must be first for any new imports |
| 2. RenderContext (all 53 tags) | 1 | Context type must exist before handlers can set its fields |
| 3. Error module | 1 | Error types needed by pipeline |
| 4. build_context: position handler | 2 | No import deps between handlers; only depend on RenderContext |
| 5. build_context: font handler | 2 | Same |
| 6. build_context: color handler | 2 | Same |
| 7. build_context: border handler | 2 | Same |
| 8. build_context: geometry handler | 2 | Same |
| 9. build_context: clip handler | 2 | Same |
| 10. build_context: karaoke handler | 2 | Same |
| 11. build_context: reset handler | 2 | Same |
| 12. build_context: transform handler | 2 | Same |
| 13. build_context: misc handler | 2 | Same |
| 14. build_context orchestrator (mod.rs) | 4-13 | Orchestrator wires all handlers together |
| 15. Support modules (animation, compositing, drawing, text_layout) | 1 | Type updates only |
| 16. Support infra (font, shaper, rasterizer, transform, karaoke) | 1 | Type updates only |
| 17. render_ass pipeline + effects handling | 3, 14, 15, 16 | Consumes all lower-level components |
| 18. Test suite (all tests → ass_core) | 17 | Must compile and run against built renderer |
| 19. 53-tag coverage tests | 17 | Integration-level tests for full tag matrix |

## Parallel Execution Graph

**Wave 1 (Start immediately):**
├── Task 1: Cargo.toml + dep swap (no blocking deps)
├── Task 2: RenderContext full-53 (no blocking deps)
├── Task 3: Error module (no blocking deps)

**Wave 2 (After Tasks 2, 3):**
├── Task 4: position handler (dep: 2)
├── Task 5: font handler (dep: 2)
├── Task 6: color handler (dep: 2)
├── Task 7: border handler (dep: 2)
├── Task 8: geometry handler (dep: 2)
├── Task 9: clip handler (dep: 2)
├── Task 10: karaoke handler (dep: 2)
├── Task 11: reset handler (dep: 2)
├── Task 12: transform handler (dep: 2)
├── Task 13: misc handler (dep: 2)
├── Task 15: support modules update (dep: 1)
├── Task 16: support infra update (dep: 1)

**Wave 3 (After Tasks 4-16):**
├── Task 14: build_context orchestrator (dep: 4-13)
├── Task 17: render_ass pipeline (dep: 3, 14, 15, 16)

**Wave 4 (After Tasks 14, 17):**
├── Task 18: test suite update (dep: 17)
├── Task 19: 53-tag coverage tests (dep: 17)

**Critical path:** Task 1 → 2 → 4-13 → 14 → 17 → 18/19
**Estimated speedup:** ~60% parallel

## Tasks

### Task 1: Cargo.toml + lib.rs — swap dependency to ass_core

**Description**: Change `subtitle-renderer/Cargo.toml` from `ass-parser` to `ass-core` dependency. Update `src/lib.rs` public exports for any changed types (none expected — Renderer, RenderConfig, etc. stay same). Remove unused `parking_lot` if only used by pixmap_pool.

**TDD**: No behavior tests needed for dep swap (compile gate).

**Category**: `quick`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — Rust dep management.
**Depends On**: None
**Acceptance Criteria**: `cargo check -p subtitle-renderer` compiles

### Task 2: RenderContext — update for all 53 tags

**Description**: Add fields to `RenderContext` in `src/context.rs` for every missing OverrideTag variant:
- `font_size_relative_delta: f32` (tracks cumulative relative adjustments from `\fs+N`/`\fs-N`)
- `clip_drawing_current: bool`
- `clip_drawing_current_commands: Option<Vec<DrawingCommand>>`
- `clip_drawing_current_scale: f32`
- `clip_inverse_drawing_current: bool`
- `charset: u8`
- `animation_skip: bool` (already present? verify)
- `drawing_mode: u8` (already present? verify)

Document that RenderContext is the intermediate state between style defaults + override tags, consumed by render_event.

**TDD**: Write tests verifying default values for all new fields.

**Category**: `unspecified-low`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — Rust struct extensions.
**Depends On**: Task 1
**Acceptance Criteria**: New fields compile, defaults are safe (0.0, false, None)

### Task 3: Error module — EventError type

**Description**: In `src/error.rs`, add `EventError` type:
```rust
pub enum EventError {
    ShapeFailed(String),
    FontMissing(String),
    Overflow(String),
}
```

Add `EventResult<T> = Result<T, EventError>`.

**TDD**: Write tests for error display + from-impls.

**Category**: `quick`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — Rust error type.
**Depends On**: Task 1
**Acceptance Criteria**: EventError compiles, displays properly

### Task 4: build_context — position handler

**Description**: Create `src/renderer/context/position.rs` with `apply_position_tag(ctx: &mut RenderContext, tag: &OverrideTag, ...)`.

Handle:
- `Pos { x, y }` → ctx.x, ctx.y (apply scale), set `has_pos`
- `Move { x1, y1, x2, y2, t1, t2 }` → store move params, set `has_pos`, defer interpolation
- `Origin { x, y }` → ctx.origin_x, ctx.origin_y (apply scale)

Export `PositionState` struct for deferred move interpolation state.

**TDD**: Write tests per tag verifying correct field mutation + scale application.

**Category**: `unspecified-low`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — Rust logic, simple match patterns.
**Depends On**: Task 2
**Acceptance Criteria**: All 3 tags correctly update ctx fields; tests pass

### Task 5: build_context — font handler

**Description**: Create `src/renderer/context/font.rs` with `apply_font_tag(ctx: &mut RenderContext, tag: &OverrideTag, ...)`.

Handle:
- `FontName(name)` → ctx.font_name
- `FontSize(fs)` → ctx.font_size = fs * scale_y (absolute)
- `FontSizeRelative(delta)` → ctx.font_size += delta * scale_y (relative)
- `Bold(b)` → ctx.bold
- `BoldWeight(w)` → ctx.bold = w >= 700
- `Italic(i)` → ctx.italic
- `Underline(u)` → ctx.underline
- `Strikeout(s)` → ctx.strikeout

**TDD**: Write tests per tag.
- FontSizeRelative: `\fs+10` at base 48 → 58. `\fs-5` at 48 → 43.

**Category**: `unspecified-low`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — simple field mutations.
**Depends On**: Task 2
**Acceptance Criteria**: All 8 tags correctly update ctx; FontSizeRelative tests pass

### Task 6: build_context — color handler

**Description**: Create `src/renderer/context/color.rs` with `apply_color_tag(ctx: &mut RenderContext, tag: &OverrideTag)`.

Handle all 9 color/alpha tags:
- `PrimaryColor(c)` → ctx.primary_color = c.to_rgba()
- `SecondaryColor(c)` → ctx.secondary_color = c.to_rgba()
- `OutlineColor(c)` → ctx.outline_color = c.to_rgba()
- `ShadowColor(c)` → ctx.shadow_color = c.to_rgba()
- `Alpha(a)` → set alpha on all 4 colors: `255 - a`
- `PrimaryAlpha(a)` → ctx.primary_color[3] = `255 - a`
- `SecondaryAlpha(a)` → ctx.secondary_color[3] = `255 - a`
- `OutlineAlpha(a)` → ctx.outline_alpha[3] = `255 - a`
- `ShadowAlpha(a)` → ctx.shadow_color[3] = `255 - a`

**TDD**: Test `Alpha` cascades to all 4 colors. Each per-color-alpha tag only touches its target.

**Category**: `unspecified-low`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — color manipulation.
**Depends On**: Task 2
**Acceptance Criteria**: All 9 tags work; Alpha cascade test passes

### Task 7: build_context — border handler

**Description**: Create `src/renderer/context/border.rs` with `apply_border_tag(ctx: &mut RenderContext, tag: &OverrideTag)`.

Handle:
- `Border(w)` → ctx.outline_width = w, reset ctx.outline_x_width = 0, ctx.outline_y_width = 0
- `BorderX(w)` → ctx.outline_x_width = w
- `BorderY(w)` → ctx.outline_y_width = w
- `Shadow(d)` → ctx.shadow_depth = d, reset ctx.shadow_x = 0, ctx.shadow_y = 0
- `ShadowX(d)` → ctx.shadow_x = d
- `ShadowY(d)` → ctx.shadow_y = d

**TDD**: Test Border() resets X/Y. Test Shadow() resets X/Y. Independent prop checks.

**Category**: `unspecified-low`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — simple field mutations.
**Depends On**: Task 2
**Acceptance Criteria**: All 6 tags work; reset semantics verified

### Task 8: build_context — geometry handler

**Description**: Create `src/renderer/context/geometry.rs` with `apply_geometry_tag(ctx: &mut RenderContext, tag: &OverrideTag)`.

Handle:
- `Scale { x, y }` → ctx.scale_x, ctx.scale_y
- `ScaleReset` → reset to style's scale_x, scale_y (from StyleRef)
- `Rotation { x, y, z }` → ctx.rotation = z; ctx.perspective_x = x; ctx.perspective_y = y
- `Shear { x, y }` → ctx.shear_x, ctx.shear_y
- `Spacing(s)` → ctx.spacing
- `Blur(r)` / `GaussianBlur(r)` → ctx.blur

**TDD**: Test ScaleReset retrieves style values. Test Rotation maps fields correctly.

**Category**: `unspecified-low`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — geometry math, style ref lookup.
**Depends On**: Task 2
**Acceptance Criteria**: All 8 tags work; ScaleReset fetches from style

### Task 9: build_context — clip handler

**Description**: Create `src/renderer/context/clip.rs` with `apply_clip_tag(ctx: &mut RenderContext, tag: &OverrideTag)`.

Handle all 6 clip variants:
- `Clip { x1, y1, x2, y2 }` → ctx.clip_x1/y1/x2/y2, clip_enabled=true, clip_inverse=false
- `ClipInverse { x1, y1, x2, y2 }` → same but clip_inverse=true
- `ClipDrawing { scale, commands }` → ctx.clip_drawing_commands, clip_drawing_scale, clip_enabled=true, clip_inverse=false
- `ClipInverseDrawing { scale, commands }` → same but clip_inverse=true
- `ClipDrawingCurrent { scale, commands }` → ctx.clip_drawing_current_commands, etc.
- `ClipInverseDrawingCurrent { scale, commands }` → same but inverse=true

**TDD**: Test each variant sets correct flags. Test non-rect clip sets drawing_commands.

**Category**: `unspecified-low`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — conditional flags, command storage.
**Depends On**: Task 2
**Acceptance Criteria**: All 6 clip variants set correct state

### Task 10: build_context — karaoke handler

**Description**: Create `src/renderer/context/karaoke.rs` with `apply_karaoke_tag(ctx: &mut RenderContext, tag: &OverrideTag)`.

Handle `Karaoke { style, duration }` tag. This sets karaoke-syllable state per syllable. The handler sets `ctx.karaoke_active = true` so the pipeline knows to invoke `KaraokeRenderer`.

**TDD**: Simple boolean test.

**Category**: `quick`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming`.
**Depends On**: Task 2
**Acceptance Criteria**: Karaoke tag sets karaoke_active flag

### Task 11: build_context — reset handler

**Description**: Create `src/renderer/context/reset.rs` with `apply_reset_tag(ctx: &mut RenderContext, tag: &OverrideTag, style: &Style)`.

Handle:
- `Reset(style_name)` → look up style by name, reset all ctx fields to style defaults; if empty string uses event's own style
- `ResetAll` → reset to event's own style defaults

**TDD**: Test Reset with explicit style name, with empty name, and ResetAll.

**Category**: `unspecified-low`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming`.
**Depends On**: Task 2
**Acceptance Criteria**: Reset and ResetAll correctly restore style defaults; animation_skip cleared

### Task 12: build_context — transform handler

**Description**: Create `src/renderer/context/transform.rs` with `apply_transform_tag(ctx: &mut RenderContext, tag: &OverrideTag, ...)`.

Handle `Transform { inner_tag, t1, t2, accel }`:
- Parse inner tag using `ass_core::override_tag::parse_tag()` (not `ass_parser::parse_override_tag`)
- If inner has `Pos`, initialize ctx position from alignment-derived values
- Delegate to existing `animation::apply_transform_tag` for interpolation

**TDD**: Test that animation module is called correctly. Test Pos-in-Transform alignment init.

**Category**: `unspecified-low`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — animation integration, forwarded calls.
**Depends On**: Task 2
**Acceptance Criteria**: Transform correctly forwards to animation; Pos init works

### Task 13: build_context — misc handler

**Description**: Create `src/renderer/context/misc.rs` with `apply_misc_tag(ctx: &mut RenderContext, tag: &OverrideTag)`.

Handle:
- `AlignmentVsfilter(a)` → ctx.alignment
- `AlignmentNumpad(a)` → ctx.alignment
- `WrapStyle(w)` → ctx.wrap_style
- `WritingMode(m)` → ctx.writing_mode
- `Charset(c)` → ctx.charset
- `AnimationSkip` → ctx.animation_skip = true
- `BaselineOffset(o)` → ctx.baseline_offset
- `DrawingMode(l)` → ctx.drawing_mode
- `Unknown(s)` → tracing::warn!(tag = %s, "unrecognized tag ignored")

**TDD**: Test each tag sets the right field; Unknown logs warning (tracing test).

**Category**: `unspecified-low`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — simple field setters.
**Depends On**: Task 2
**Acceptance Criteria**: All 9 misc tags handled; Unknown warns via tracing

### Task 14: build_context orchestrator (mod.rs)

**Description**: Create `src/renderer/context/mod.rs` — single `pub fn build_context(...)` that:
1. Initialize `RenderContext` from style values + event margins
2. Apply resolution scaling (config.width/script_width, config.height/script_height)
3. Iterate `event.override_tags` — unwrap `TaggedOverride` to get `(OverrideTag, Span)`
4. Dispatch each tag to the appropriate handler module via a match
5. Apply deferred position/animation (Move interpolation, Fade/FadeComplex alpha, fallback alignment)
6. Return `RenderContext`

**TDD**: Write integration tests that construct an Event with various override tags and verify resulting RenderContext. Test Fade/FadeComplex alpha interpolation with known timestamps.

**Category**: `unspecified-high`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — coordination, tag dispatch, time interpolation.
**Depends On**: Tasks 4-13
**Acceptance Criteria**: All 53 tags correctly dispatched; integration tests pass

### Task 15: Support modules — update for ass_core types

**Description**: Update `src/renderer/animation.rs`, `compositing.rs`, `drawing.rs`, `text_layout.rs`:

- **animation.rs**: Change `parse_override_block` from `ass_parser::parse_override_tag` to `ass_core::override_tag::parse_tags`. Update type signatures for `compute_fad_alpha`, `compute_fade_complex`, `interpolate_move`, `apply_transform_tag`.
- **compositing.rs**: Update imports. No structural changes expected.
- **drawing.rs**: `parse_drawing_commands` and `parse_drawing_level` — update to ass_core types if they use `DrawCommand` from ass_core.
- **text_layout.rs**: Update imports. Verify `wrap_text`, `wrap_text_vertical`, `remap_alignment_vertical`, `alignment_to_pos` still work.

**TDD**: Update existing unit tests (in each module's `#[cfg(test)]`) to compile and pass with ass_core types.

**Category**: `unspecified-high`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — type-migration, ass_core API usage.
**Depends On**: Task 1
**Acceptance Criteria**: All modules compile; existing unit tests pass

### Task 16: Support infra — update type imports

**Description**: Update `src/font.rs`, `shaper.rs`, `rasterizer.rs`, `transform.rs`, `effects.rs`, `karaoke.rs`:

- **font.rs**: `FontManager` — no API changes expected. Update imports from `ass_parser` to `ass_core` if any. Re-check CJK warmup: it uses `ttf_parser::Face` directly, not through ass types. Verify.
- **shaper.rs**: Update any `ass_parser` imports. Verify `Shaper::shape` still accepts same text/buffer types.
- **rasterizer.rs**: No ass type imports expected.
- **transform.rs**: `AffineTransform` — no imports from ass_parser.
- **effects.rs**: `apply_gaussian_blur`, `apply_shadow`, `composite_over` — no ass type deps.
- **karaoke.rs**: `KaraokeRenderer` — currently imports `KaraokeSegment` from `ass_parser::karaoke`. Change to `ass_core::KaraokeSegment` (struct API is identical). Update tests.

**TDD**: Each module's existing tests must compile and pass.

**Category**: `unspecified-low`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — import-only updates.
**Depends On**: Task 1
**Acceptance Criteria**: All infra modules compile; existing tests pass

### Task 17: render_ass pipeline — full rebuild

**Description**: Rewrite `src/renderer/mod.rs`:
- `Renderer::render_ass(&self, doc: &SubtitleDocument, fps: &Fps, timestamp_ms: u64) -> RenderedFrame`
  - Create pixmap from pool
  - Filter events: iterate `doc.events`, filter by `start_ms ≤ timestamp_ms < end_ms` and `EventType::Dialogue`
  - Sort by layer
  - For each event in try/catch (std::panic::catch_unwind for panic safety):
    - Find style by `event.style.as_str()` in `doc.styles`
    - Call `context::build_context(event, style, doc.metadata, config, timestamp_ms)`
    - Call `render_event(pixmap, event, &ctx, timestamp_ms, event_start_ms)`
    - On error: `tracing::warn!(...)` and continue
  - Post-process: apply Fade/FadeComplex alpha multiplier from build_context output
  - Return `RenderedFrame { pts_ms, duration_ms, width, height, bitmap }`
- `Renderer::render_ass_cached` — update to new signature
- Keep `render_event`, `render_drawing`, `render_karaoke` as private methods (moved from current mod.rs)
- Remove old `EventExt` trait (needed? Check if anything external uses it)

**TDD**: Write render integration tests:
- Basic render (no tags) produces correct-size bitmap
- Single tag (PrimaryColor override) renders with expected color
- Event outside time range is skipped
- Event with Fade animation interpolates alpha correctly

**Category**: `unspecified-high` (high complexity — pipeline coordination, error isolation)
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — pipeline integration, event filtering, error isolation.
**Depends On**: Tasks 3, 14, 15, 16
**Acceptance Criteria**: `render_ass` compiles, produces RenderedFrame with correct dimensions; basic tests pass

### Task 18: Test suite — update all tests

**Description**: Update every test file in `crates/subtitle-renderer/tests/`:
- `test_context.rs` → use `ass_core::SubtitleDocument`, `ass_core::Event`, `ass_core::OverrideTag`
- `test_renderer.rs` → update fixture builders, construct `SubtitleDocument` instead of `AssFile`
- Any other test files

Key API mapping for test helpers:
| Old (ass_parser) | New (ass_core) |
|---|---|
| `AssFile::new()` | `SubtitleDocument::default()` then push styles/events |
| `Event { start: Timestamp::from_ms(x), end: ..., style_name, text, ... }` | `Event { start_ms: x, end_ms: y, style: StyleRef::new("Default"), text_raw, ... }` |
| `AssFile::find_style()` → `doc.styles.iter().find()` | |
| `event.is_visible_at(ts)` | manual `ts >= start_ms && ts < end_ms` |

**TDD**: Compile-and-run existing test suites. Fix compilation errors due to type changes.

**Category**: `unspecified-high`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — test migration.
**Depends On**: Task 17
**Acceptance Criteria**: All existing tests compile and pass with ass_core types

### Task 19: 53-tag coverage tests

**Description**: Write integration tests that verify every OverrideTag variant produces the expected `RenderContext`:

- Test file: `src/renderer/context/tests.rs` or `tests/test_53_tags.rs`
- One test per tag variant:
  ```rust
  #[test]
  fn test_tag_fontsize() {
      let event = EventBuilder::new()
          .override_tags(vec![TaggedOverride::new(OverrideTag::FontSize(72.0), Span::default())])
          .build();
      let ctx = build_context(&event, &style, &metadata, &config, 0, 0, 10000);
      assert!((ctx.font_size - 72.0 * scale_y).abs() < 0.001);
  }
  ```
- Group by handler module for readability

**TDD**: Write tests first, then ensure all handler modules make them pass.

**Category**: `unspecified-low`
**Skills**: `["programming"]`
**Skills eval**: ✅ `programming` — thorough testing.
**Depends On**: Task 17
**Acceptance Criteria**: 53 test cases, one per OverrideTag variant, all pass

## Testing infrastructure

For the TDD approach, use a helper builder:

```rust
// In tests/common/mod.rs

struct EventBuilder {
    tags: Vec<TaggedOverride>,
    start_ms: u64,
    end_ms: u64,
    style: StyleRef,
}

impl EventBuilder {
    fn new() -> Self { ... }
    fn tag(mut self, tag: OverrideTag) -> Self { ... }
    fn build(self) -> Event { ... }
}

fn make_test_renderer() -> Renderer {
    Renderer::new(RenderConfig {
        width: 1920,
        height: 1080,
        script_width: 1920,
        script_height: 1080,
        ..Default::default()
    })
}
```

## Commit Strategy

Each task is one commit (19 commits). Use Conventional Commits:

```
1.  chore(subtitle-renderer): swap ass-parser dep to ass-core
2.  feat(subtitle-renderer): extend RenderContext for all 53 override tags
3.  feat(subtitle-renderer): add EventError for per-event isolation
4.  feat(renderer/context): position handler (Pos, Move, Origin)
5.  feat(renderer/context): font handler (FontName, FontSize, FontSizeRelative, Bold, etc.)
6.  feat(renderer/context): color handler (9 color/alpha variants)
7.  feat(renderer/context): border handler (Border, Shadow, X/Y variants)
8.  feat(renderer/context): geometry handler (Scale, Rotation, Shear, Spacing, Blur)
9.  feat(renderer/context): clip handler (6 clip variants)
10. feat(renderer/context): karaoke handler
11. feat(renderer/context): reset handler (Reset, ResetAll)
12. feat(renderer/context): transform handler (Transform with animation)
13. feat(renderer/context): misc handler (charset, animation_skip, drawing_mode, etc.)
14. feat(renderer/context): build_context orchestrator
15. refactor(subtitle-renderer): update support modules for ass_core types
16. refactor(subtitle-renderer): update font/shaper/rasterizer/effects for ass_core
17. feat(subtitle-renderer): rebuild render_ass pipeline with SubtitleDocument + error isolation
18. test(subtitle-renderer): update all tests to ass_core types
19. test(subtitle-renderer): add 53-tag coverage tests
```

## Success Criteria

1. `cargo check -p subtitle-renderer` — zero errors
2. `cargo clippy -p subtitle-renderer --all-targets -- -D warnings` — clean
3. `cargo test -p subtitle-renderer` — all tests pass
4. `cargo test -p subtitle-renderer --doc` — doc tests pass
5. All 53 OverrideTag variants produce correct RenderContext state
6. A single event rendering failure does not prevent other events from rendering
7. Fps integration does not break existing downstream consumers (once they migrate)

---

## 渲染器模块架构

```
crates/subtitle-renderer/src/
│
├── lib.rs                             ← 重导出 + Renderer/RenderConfig 公开 API
│
├── context.rs / renderer/context/     ★ 核心：build_context（10处理模块）
│   ├── mod.rs                         ← orchestrator: 1次match → 53变体 → 分派
│   ├── position.rs                    ← Pos, Move, Origin
│   ├── font.rs                        ← FontName, FontSize, FontSizeRelative, Bold, BoldWeight, Italic, Underline, Strikeout
│   ├── color.rs                       ← PrimaryColor, SecondaryColor, OutlineColor, ShadowColor, Alpha, PrimaryAlpha... (9)
│   ├── border.rs                      ← Border, BorderX, BorderY, Shadow, ShadowX, ShadowY (6)
│   ├── geometry.rs                    ← Scale, ScaleReset, Rotation, Shear, Spacing, Blur, GaussianBlur (7)
│   ├── clip.rs                        ← Clip, ClipInverse, ClipDrawing, ClipInverseDrawing, ClipDrawingCurrent, ClipInverseDrawingCurrent (6)
│   ├── karaoke.rs                     ← Karaoke (flag)
│   ├── reset.rs                       ← Reset, ResetAll
│   ├── transform.rs                   ← Transform (内嵌标签 → animation.rs 插值)
│   └── misc.rs                        ← AlignmentVsfilter, AlignmentNumpad, WrapStyle, WritingMode, Charset, AnimationSkip, BaselineOffset, DrawingMode, Unknown
│
├── renderer/
│   ├── mod.rs                         ← render_ass 管线（主入口）
│   ├── animation.rs                   ← 动画插值（Move, Fade, FadeComplex, Transform）
│   ├── compositing.rs                 ← 图层合成
│   ├── drawing.rs                     ← 绘图命令解析
│   └── text_layout.rs                 ← 文字布局/换行
│
├── font.rs                            ← FontManager (fontdb + CJK fallback)
├── shaper.rs                          ← rustybuzz HarfBuzz 塑形
├── rasterizer.rs                      ← tiny-skia 光栅化
├── transform.rs                       ← AffineTransform 矩阵变换
├── effects.rs                         ← 高斯模糊、阴影合成
├── karaoke.rs                         ← KaraokeRenderer 卡拉OK分色
└── error.rs                           ← EventError + EventResult
```

## 字体渲染策略（痛点修补）

### 当前 CJK 问题（memory #488）

> v0.5.5 移除了 `font_has_cjk_glyphs` 从 scoring match 中，以避免 Windows 上 30+ 秒的 TTF 解析阻塞。但后果是：如果 fontdb 的模糊匹配对 CJK 字体族名返回了一个仅拉丁字体，CJK 字符将显示 tofu（□）。

### 2.2 修复方案

| 问题 | 方案 |
|------|------|
| fontdb 模糊匹配返回仅拉丁字体 | 在 MatchResult 后增加 `has_cjk_glyph` 快速验证（使用 `ttf_parser::Face::glyph_index(0x4E2D)` 缓存到 100ms 内）|
| CJK 回退链硬编码 | 保留现有 8 级链（scoring→suffix-strip→fontconfig→hardcoded→query_cjk_capable_any→generic→SansSerif→any） |
| 预暖缓存 | CJK 能力查询结果按 font_id 缓存（已有：memory #485） |
| FontConfig 热启动 | `load_fontconfig()` 返回 `bool` 后自动 fallback 到目录扫描（已有：memory #474） |

### font fallback 决策树

```text
query_with_fallback_inner(family_name, style, size):
  ├── 1. scoring match (fontdb 全文检索)
  │     ├── ✅ 匹配 → check has_cjk_glyph (100ms) → return
  │     └── ❌ 无匹配 → continue
  ├── 2. suffix-strip name match
  │     ├── ✅ → check has_cjk_glyph → return
  │     └── ❌ → continue
  ├── 3. fontconfig alias resolution
  ├── 4. hardcoded CJK list (Noto Sans CJK, WenQuanYi, IPAGothic, NanumGothic...)
  │     └── each → check has_cjk_glyph → first match return
  ├── 5. query_cjk_capable_any (扫描 db.faces())
  ├── 6. hardcoded generic list (Liberation Sans, DejaVu Sans...)
  └── 7. Family::SansSerif → any available face → return
```

## 日志策略

### tracing event 层次设计

| 级别 | 用途 | 触发条件 | 示例 |
|------|------|---------|------|
| `TRACE` | 每事件调试 | 每帧每事件 | `applying tag={tag} span={span}` |
| `DEBUG` | 管线阶段 | 每帧一次 | `build_context: {n_tags} tags` |
| `INFO` | 产线状态 | 每条事件 | `rendered event #{idx} at {pts}ms` |
| `WARN` | 问题但继续 | 标签未知、回退、边缘 | `font fallback: {family}→{fallback}` |
| `ERROR` | 事件失败 | catch_unwind 捕获 | `event #{idx} skipped: {error}` |

### 禁止原始数据输出

- ❌ `tracing::debug!("text_raw = {}", event.text_raw);`
- ❌ `tracing::warn!("bitmap = {:?}", bitmap);`
- ✅ `tracing::debug!(event = %event.idx, n_tags = tags.len(), "build_context");`
- ✅ `tracing::warn!(family = %name, fallback = %fb, "font fallback");`

## PGS 编码器规范补完 (2.3)

> 这部分在 renderer 重建之后（2.3），但架构在 2.2 中确定。

### 已识别缺口

| 规范项 | 当前状态 | 问题 |
|--------|---------|------|
| `CompositionState::EpochContinue(0xC0)` | ❌ 枚举缺失 | `EpochContinue` 注释有写但枚举无此变体，导致某些播放器（PotPlayer）崩溃 |
| PDS YCbCr 字节序 | ⚠️ Y,Cr,Cb | 部分播放器要求 Y,Cb,Cr。需要确认规范并做兼容 |
| ODS flags 字段 | ⚠️ 0xC0 | 旧代码用 0x80，差异需要对齐到参考SUP |
| ODS total_size 格式 | ⚠️ 3字节 LE | 需要与参考实现一致 |
| `palette_update` 逻辑 | ⚠️ 始终 true | 应基于 `palette_hash` 检测变化 |
| Epoch 管理策略 | ⚠️ 每帧 EpochStart | 正确：首帧 EpochStart → 后续 AcquirePoint/NormalCase → 无变化时 EpochContinue |
| 多对象 ODS 序列 | ⚠️ 基础支持 | ODS 拆分在部分播放器上可能不兼容 |

### 修复后的 DisplaySet 生命周期

```text
Epoch Start ─── 清屏，完整PCS+WDS+PDS+ODS
Normal Case ──── 同一epoch内，部分更新（PCS+WDS+PDS+ODS）
Acquire Point ── 可重新同步（PCS+WDS+PDS+ODS）
Palette Only ── 仅改透明度（PDS version++），淡入淡出用
Epoch Continue ─ 画面不变，只延长 PTS（PCS+END，无ODS无PDS）
```

### 当前 encode_frame 输出

```text
DisplaySet 1: [PCS][WDS][PDS][ODS...][END]  ← EpochStart + 字幕
DisplaySet 2: [PDS...][END]                  ← Palette clear (淡出)
```

### 目标输出

```text
DisplaySet 1: [PCS][WDS][PDS][ODS...][END]  ← EpochStart (首帧)
DisplaySet 2: [PCS][WDS][PDS][ODS...][END]  ← NormalCase (后续帧)
...
DisplaySet N: [PCS][END]                    ← EpochContinue (无变化)
DisplaySet N+1: [PDS...][END]              ← Palette Only (淡入/淡出)
```

### PGS 编码器模块化（2.3）

```text
pgs-encoder/src/
├── encoder/
│   ├── mod.rs        ← encode_frame 管线调度 (EpochStart/NormalCase选择)
│   ├── epoch.rs      ← DisplaySet 类型选择 + composition_number 递增管理
│   ├── palette.rs    ← PDS 构建 + version 递增 + palette_hash 变化检测
│   ├── object.rs     ← ODS 构建 + 分块 + flags/total_size 合规
│   └── segment.rs    ← Segment 序列化 (to_bytes) + CRC 计算
├── rle.rs            ← RLE 编码 (保留现有)
├── decoder.rs        ← SUP 解码 (保留现有)
└── color.rs          ← RGBA↔YCbCr 转换 (保留现有)
```

---

## 测试策略

### 三层测试金字塔

```text
┌─────────────────────────────────────────────────────┐
│  E2E (少)                                           │
│  53-tag 覆盖测试                                     │
│  └─ test_53_tags.rs — 每标签一个测试                    │
│  └─ test_renderer.rs — 完整渲染对比                    │
│  └─ test_context.rs — RenderContext 快照               │
├─────────────────────────────────────────────────────┤
│  集成 (一些)                                          │
│  └─ render_ass 管线 — 全事件过滤 → 图层排序测试          │
│  └─ build_context — 多标签组合测试                     │
│  └─ animation — Fade/Transform 插值测试               │
│  └─ karaoke — 卡拉OK分色 + 时间切片测试                │
├─────────────────────────────────────────────────────┤
│  单元 (多)                                            │
│  └─ 每个 tag handler: 输入 OverrideTag → 输出 RenderContext│
│  └─ font fallback: CJK 检测 → 正确回退                 │
│  └─ text_layout: 对齐/换行/垂直书写                     │
│  └─ effects: 模糊/阴影/合成                            │
│  └─ error: EventError 显示 + transform                 │
└─────────────────────────────────────────────────────┘
```

### EventBuilder 辅助测试结构

```rust
// tests/common/mod.rs
use ass_core::*;

pub struct EventBuilder {
    tags: Vec<TaggedOverride>,
    start_ms: u64,
    end_ms: u64,
    style: StyleRef,
    text: String,
}

impl EventBuilder {
    pub fn new() -> Self { ... }
    pub fn tag(mut self, tag: OverrideTag) -> Self { self.tags.push(TaggedOverride::new(tag, Span::default())); self }
    pub fn text(mut self, t: &str) -> Self { self.text = t.to_string(); self }
    pub fn build(self) -> Event { Event::default()... }

    /// Build + build_context directly
    pub fn into_context(self, ...) -> RenderContext { ... }
}

pub fn make_test_document(event: &Event) -> SubtitleDocument { ... }
pub fn make_test_renderer() -> Renderer { ... }
```

### 53 标签覆盖验证清单

| 模块 | 标签 | 测试数 | 验证方式 |
|------|------|--------|---------|
| position | Pos, Move, Origin | 6 | field assertion + scale apply |
| font | FontName, FontSize, FontSizeRelative, Bold, BoldWeight, Italic, Underline, Strikeout | 12 | field assertion |
| color | 1c, 2c, 3c, 4c, alpha, 1a, 2a, 3a, 4a | 12 | rgba field assertion |
| border | bord, xbord, ybord, shad, xshad, yshad | 8 | field assertion + reset |
| geometry | fscx, fscy, fsc, frx, fry, frz, fr, fax, fay, fsp, be, blur | 14 | field assertion + ScaleReset |
| clip | clip, iclip, clip(), iclip() + @variants | 8 | flag + drawing_commands |
| karaoke | k, kf, K, ko, kt | 5 | flag assertion |
| reset | r, r(StyleName), r() | 4 | field restoration check |
| transform | t() | 4 | animation delegate |
| misc | an, a, q, writing-mode, fe, !, p, pbo, unknown | 12 | field + warning |
| **总计** | | **85** (含组合测试) | |

## 退出标准

### 编译质量门

| 检查 | 命令 | 结果 |
|------|------|------|
| 编译 | `cargo check -p subtitle-renderer` | ✅ 零错误 |
| Clippy | `cargo clippy -p subtitle-renderer --all-targets -- -D warnings` | ✅ 零警告 |
| Format | `cargo fmt -- --check` | ✅ 零漂移 |
| Doc | `cargo doc -p subtitle-renderer --no-deps --document-private-items` | ✅ 零错误 |

### 测试质量门

| 检查 | 目标 |
|------|------|
| 单元测试 | 100% 通过 |
| proptest | 无 panic |
| Doc 测试 | 100% 通过 |
| 53 标签覆盖测试 | 85+ 测试全部通过 |

### 行为质量门

| 项目 | 验证方式 |
|------|---------|
| 事件级隔离 | `catch_unwind` 测试：一个事件 panic → 其他事件正常渲染 |
| CJK 回退验证 | 纯 CJK ASS 文件渲染 → 无 tofu (OCR 验证) |
| 帧精度 | `Fps{24000,1001}` → `ms_to_90khz` 纯整数无抖动 |
| 淡入淡出 | `\fad(200,200)` → 首帧 alpha=0, 中间渐变, 尾帧 alpha=0 |
| 多事件渲染 | 3 个重叠事件正确排序 + 合成 |
| 原始文本保留 | `\N` 不被转换，文本原样传递给 shaper |

### 代码质量门

| 项目 | 门限 |
|------|------|
| 最大单文件 | < 250 纯 LOC (ass-core 标准) |
| 模块化 | build_context 10 个处理模块各司其职 |
| 错误隔离 | render_ass 永不 panic（永远返回 RenderedFrame 或错误日志） |
| 日志 | 无原始数据输出到 log |
| unsafe | 零（现有 renderer 已零 unsafe） |

---

## new session 执行指南

### 前置条件

```bash
git checkout dev-2026-06-22
```

### 执行顺序（严格按波次）

```
Wave 1: Tasks 1-3 并行
Wave 2: Tasks 4-13 + 15-16 并行 (等待 T2/T3)
Wave 3: Tasks 14 + 17
Wave 4: Tasks 18-19

每任务：
  RED (写测试) → GREEN (实现) → 验证
```

### 验证每步

```bash
cargo check -p subtitle-renderer && cargo clippy -p subtitle-renderer --all-targets -- -D warnings && cargo test -p subtitle-renderer
```
