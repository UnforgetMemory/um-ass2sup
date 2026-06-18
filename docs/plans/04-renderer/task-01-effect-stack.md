# Task 4.x — Per-line effect stack (Sprint 3 foundation)

## Status: ✅ COMPLETED (2026-06-17)

## Goal

Introduce a typed `EffectStack` that consolidates the 11 ASS effect categories
into a single data structure with per-frame evaluation semantics.

## Files changed

| File | Change |
|------|--------|
| `crates/subtitle-renderer/src/effect_stack.rs` (new) | `RendererEffect` enum (Fade, FadeComplex, Pos, Move, Clip, InverseClip, RotationX/Y/Z, ShearX/Y, Blur, EdgeBlur) + `EffectStack` (push/len/is_empty/resolve_*/apply) + 14 unit tests |
| `crates/subtitle-renderer/src/lib.rs` | `pub mod effect_stack;` + re-exports |

## API surface

```rust
pub enum RendererEffect {
    Fade { fade_in_ms: u32, fade_out_ms: u32 },
    FadeComplex { a1, a2, a3: u8, t1_ms, t2_ms, t3_ms, t4_ms: u32 },
    Pos { x, y: f32 },
    Move { x1, y1, x2, y2: f32, t1_ms, t2_ms: u32 },
    Clip { x1, y1, x2, y2: f32 },
    InverseClip { x1, y1, x2, y2: f32 },
    RotationZ(f32),
    RotationX(f32),
    RotationY(f32),
    ShearX(f32),
    ShearY(f32),
    Blur(f32),
    EdgeBlur(f32),
}

pub struct EffectStack { effects: Vec<RendererEffect> }
impl EffectStack {
    pub fn push(&mut self, effect: RendererEffect);
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn resolve_pos(&self, time_ms: u64, event_duration_ms: u64) -> (f32, f32);
    pub fn resolve_alpha(&self, time_ms: u64, event_duration_ms: u64) -> f32;
    pub fn resolve_clip(&self) -> Option<(f32, f32, f32, f32, bool)>;
    pub fn resolve_blur(&self) -> f32;
    pub fn resolve_rotation_z(&self) -> f32;
    pub fn apply(&self, ctx: &mut RenderContext, time_ms: u64, event_duration_ms: u64);
}
```

## Composition semantics

- **Pos + Move**: `Move` wins for the duration of the animation; `Pos` is the fallback.
- **Clip + InverseClip**: later one wins (last-push semantics, libass convention).
- **Fade + FadeComplex**: `FadeComplex` wins when set; `Fade` provides the simple two-segment curve otherwise.
- **Blur + EdgeBlur**: later one wins.

## Verification gates

- [x] 14 unit tests in `effect_stack::tests`
- [x] `cargo test -p subtitle-renderer` — passes
- [x] `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings
- [x] `cargo fmt --check` — clean

## Migration path

The existing `context.rs` `build_context()` still does the legacy tag-by-tag
evaluation. `EffectStack::apply()` is the v2.0 entry point that the renderer
will call per-frame. The full migration is tracked in
`docs/superpowers/specs/2026-06-17-Sub-4-renderer.md`.
