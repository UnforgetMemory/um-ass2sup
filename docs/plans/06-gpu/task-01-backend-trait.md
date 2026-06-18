# Task 6.1 — RendererBackend trait + dispatch policy (Sprint 5 foundation)

## Status: ✅ COMPLETED (2026-06-17)

## Goal

Introduce the `RendererBackend` trait and the `BackendPolicy` selector
that abstract the CPU (tiny-skia) and GPU (vello, future) rendering
paths behind a single interface.

## Files changed

| File | Change |
|------|--------|
| `crates/subtitle-renderer/src/backend.rs` (new) | `RendererBackend` trait, `BackendPolicy` enum, `Point`/`Rect`/`Color`/`Glyph`/`GlyphId`/`RenderedBitmap`/`BackendEffect` types, 9 unit tests |
| `crates/subtitle-renderer/src/lib.rs` | `pub mod backend;` + 9-type re-export |

## Types

- `Point { x, y }`, `Rect { x, y, width, height }` — 2D primitives
- `Color { r, g, b, a }` with `BLACK`/`WHITE`/`TRANSPARENT` constants
- `GlyphId(u32)` — opaque backend-specific glyph identifier
- `Glyph { id, pos, color }` — minimum descriptor for a single glyph
- `BackendEffect::FillRect { rect, color } | Blur { rect, radius }` — post-processing primitives
- `RenderedBitmap { width, height, data: Vec<u8> }` — output format (RGBA, 8-bit per channel, row-major)
- `RendererBackend` trait: `draw_glyph`, `fill_rect`, `apply_effect`, `finalize`
- `BackendPolicy::{CpuOnly, GpuOnly, Hybrid}` with `select(event_count, gpu_available) -> &'static str`

## Dispatch policy

| Event count | GPU available | CpuOnly | GpuOnly | Hybrid |
|-------------|---------------|---------|---------|--------|
| any | false | cpu | cpu | cpu |
| < 100 | true | cpu | gpu | cpu |
| ≥ 100 | true | cpu | gpu | gpu |

Default is `CpuOnly`. This matches the v2.0 plan's hybrid strategy:
"任务 < 100 事件 → CPU, 任务 ≥ 100 事件 → GPU, 任务 ≥ 5000 事件 → GPU 强制."

## Verification gates

- [x] 9 unit tests in `backend::tests`
- [x] `cargo test -p subtitle-renderer backend` — 9/9 pass
- [x] `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings
- [x] `cargo fmt --check` — clean
- [x] No regressions in existing 240 renderer tests

## Follow-up

- **Task 6.2 — vello evaluation + PoC**: spike to confirm vello works
  on Linux/macOS/Windows, output a valid PGS, and measure baseline
  performance.
- **Task 6.3 — vello integration**: implement `RendererBackend for
  VelloBackend`.
- **Task 6.4 — hybrid pipeline**: wire `BackendPolicy::Hybrid` into
  `Renderer::new` so the CLI flag controls dispatch.
- **Task 6.5 — criterion benchmark**: prove ≥ 10x speedup on > 5000
  event corpus.
- **Task 6.6 — GPU failure fallback**: detect GPU initialization
  failure and downgrade to CPU without a CLI flag.

## Architectural note

The trait is intentionally minimal (4 methods). A vello implementation
must be `Send + Sync` (it carries a wgpu device handle). The CPU
implementation is naturally `Send + Sync` because it holds only a
`tiny_skia::Pixmap`. Adding the trait is a non-breaking change: the
existing `Renderer` and `RenderedFrame` types continue to work, and
the trait is the *seam* for future GPU work, not a replacement for
the v0.6 renderer.
