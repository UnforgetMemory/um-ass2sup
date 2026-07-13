# 🏛️ Architecture Overview

> **Pipeline design, crate responsibilities, and data flow in um-ass2sup v3.0.0**

---

## 📋 Table of Contents

- [Pipeline Overview](#pipeline-overview)
- [End-to-End Data Flow](#end-to-end-data-flow)
- [Crate Breakdown](#crate-breakdown)
- [Rendering Backend Dispatch](#rendering-backend-dispatch)
- [Memory Model](#memory-model)
- [Performance Constraints](#performance-constraints)

---

## Pipeline Overview

```
            ┌────────────┐
            │  Input     │  ASS / SSA / SRT
            └─────┬──────┘
                  │
                  ▼
         ┌─────────────────┐
         │    ass-core     │  → typed AST
         └────────┬────────┘
                  │ optional validation
                  ▼
         ┌──────────────[ Rendering Backend ]──────────────┐
         │                                                 │
         ▼                                                 ▼
   ┌──────────────────┐                         ┌──────────────────────┐
   │  native-backend  │                         │   libass-backend     │
   │  swash +         │                         │   libass FFI         │
   │  tiny-skia       │                         │   (libass-sys)       │
   └────────┬─────────┘                         └──────────┬───────────┘
            │                                               │
            ▼                                               ▼
         ┌──────────────────────────────────────────────────────┐
         │              color-quantizer                        │
         │  RGBA → indexed (≤255 + alpha), k-d tree accelerated │
         │  palette reuse · Floyd-Steinberg/Ordered/None dither │
         └──────────────────────┬───────────────────────────────┘
                                │
                                ▼
         ┌──────────────────────────────────────────────────────┐
         │                  pgs-encoder                         │
         │  Quantized frames → PGS segments (PCS/WDS/PDS/ODS)  │
         │  DDD domain: domain/ (model) + encoding/ (serialize) │
         └─────────────────┬────────────────────────────────────┘
                           │
                  ┌────────┴──────────┐
                  ▼                   ▼
            ┌──────────┐        ┌────────────┐
            │  .sup    │        │  BDN XML   │
            │  SUP/PGS │        │  + PNG seq │
            └──────────┘        └────────────┘
```

---

## End-to-End Data Flow

### Native Backend Path

```
1. Input file read (ASS/SSA/SRT)
       │
2. ass-core: Parse → typed AST
   ├─ ScriptInfo / Styles / Fonts / Events / Dialogue sections
   └─ SubtitleFormat::detect() auto-detects format
       │
3. subtitle-validator (optional): Validate syntax + detect overlaps
       │
4. subtitle-renderer (native):
   ├─ build_context() → RenderContext per event per timestamp
   ├─ FontRegistry.query() → font data
   ├─ SimpleShaper::shape(text, font_data, font_size) → Vec<ShapedGlyph>
   ├─ GlyphRasterizer::rasterize(font_data, glyph_id, font_size) → RasterizedGlyph
   ├─ composite_glyph() → Porter-Duff over compositing
   ├─ effects: blur, shadow, outline, karaoke
   └─ transform_layer → AffineTransform (scale/rotate/shear/perspective)
       │
5. color-quantizer:
   ├─ Tone mapping (optional HDR→SDR)
   ├─ Median-cut palette generation
   ├─ K-D tree nearest-color lookup (2.57× acceleration)
   ├─ Dithering (Floyd-Steinberg / Ordered / None)
   └─ Temporal palette reuse across frames
       │
6. pgs-encoder:
   ├─ EpochManager decides display set kind (EpochStart/NormalCase/etc.)
   ├─ build_display_set() → PCS + WDS + PDS + ODS segments
   ├─ RLE encoding
   └─ SUP file writer
       │
7. Output: .sup or BDN XML + PNG
```

### Libass Backend Path

```
1. Input file read
       │
2. ass-core: Parse → typed AST
       │
3. ass-core → ASS-formatted string → libass via FFI
       │
4. subtitle-renderer-libass:
   ├─ libass renders RGBA bitmaps (shaping + rasterization in C)
   └─ Receives RGBA frame buffer
       │
5. color-quantizer: (same as native)
6. pgs-encoder: (same as native)
7. Output: .sup or BDN XML + PNG
```

Both backends converge after rendering: the quantizer and encoder are **shared code**.

---

## Crate Breakdown

### Main Workspace (8 crates)

| Crate | Responsibility | Key External Dependencies | Doc Lint |
|---|---|---|---|
| **`ass-core`** | ASS/SSA/SRT parser, strong typed AST | thiserror, tracing | `unsafe_code = "deny"` |
| **`subtitle-validator`** | Syntax validation, event overlap detection | ass-core, thiserror | `#![warn(missing_docs)]` |
| **`subtitle-renderer`** | [native] RGBA bitmap rendering, font subsystem | swash, tiny-skia, wide, parking_lot | — |
| **`libass-sys`** | [libass] Manual FFI bindings for libass v0.17 | — (no build.rs deps) | — |
| **`subtitle-renderer-libass`** | [libass] libass-based rendering pipeline | libass-sys, color-quantizer, pgs-encoder, bdn-xml | `#![warn(missing_docs)]` |
| **`color-quantizer`** | RGBA → indexed color, k-d tree quantization | thiserror, tracing | `#![warn(missing_docs)]` |
| **`pgs-encoder`** | Quantized frames → PGS/SUP binary segments | color-quantizer, png | — |
| **`bdn-xml`** | Blu-ray mastering XML descriptor + PNG output | quick-xml, png | — |
| **`ass2sup-cli`** | CLI binary, feature-gated backend dispatch | clap, rayon, indicatif, serde | `#![warn(missing_docs)]` |

### Standalone Workspace

| Workspace | Purpose |
|---|---|
| `ass2sup-libass/` | Separate Cargo workspace for libass-only builds (excluded from main workspace to avoid native-backend deps) |

---

## Rendering Backend Dispatch

The CLI crate (`ass2sup-cli`) selects a rendering backend at compile time via Cargo features:

```bash
# Default: native only
cargo build --release

# libass only
cargo build --release --no-default-features -F libass-backend

# Both (runtime --backend flag)
cargo build --release --no-default-features -F native-backend,libass-backend
```

In a dual-backend build, the `--backend` CLI flag selects between `native` and `libass` at runtime. The rendered RGBA bitmap from either backend feeds into the shared `color-quantizer` → `pgs-encoder` pipeline.

---

## Memory Model

### Renderer Ownership

```
FontRegistryRenderResources
  ├── registry: Mutex<FontRegistry>     # Thread-safe font index + data store
  ├── pixmap_pool: Mutex<PixmapPool>   # 8 cached Pixmap buffers
  └── font_map: HashMap<String, Vec<String>>  # Font name → file path mapping
```

### Per-Frame Flow

1. `build_context()` — produces one `RenderContext` per event per timestamp
2. `render_event_font_registry()` — allocates one `layer: Pixmap` per event:
   - `pool_get()` → fill/outline/shadow → composite → `pool_put()`
3. `transform_layer()` — allocates output buffer (transform is approx 1:1 or smaller)

### Peak Memory

```
max_events_per_timestamp × layer_size + output_buffer
```

Typically **< 50 MB at 1080p**.

### Constraints

- **No heap allocation in hot render paths** (glyph loop, composite, transform)
- **PixmapPool**: reuse Pixmap buffers via pool_get/pool_put (8 cached entries, wrapped in Mutex)
- **Parallel rendering**: rayon-based `par_iter()` in build_display_set — each worker holds 1 frame at a time (~8.3 MB at 1080p), no intermediate `Vec<RenderedFrame>`

---

## Performance Constraints

| Constraint | Implementation |
|---|---|
| **No heap allocation** in hot paths | Glyph loop, composite, transform use stack + pre-allocated buffers |
| **SIMD compositing** | `wide::u32x4` Porter-Duff over for 4-pixel chunks |
| **SIMD transforms** | `wide::f32x4` bilinear interpolation in `apply_to_pixmap` |
| **Buffer pooling** | `PixmapPool` with 8 cached entries, `Mutex`-protected |
| **k-d tree acceleration** | `find_nearest_index` for palette mapping (2.57× vs linear search) |
| **Palette dedup** | `HashSet<u32>` reduces O(n²) → O(n) |
| **Parallel quantization** | rayon-based per-frame quantization (opt-in via `--parallel-frames`) |

---

## Key Architectural Decisions

1. **Two independent backends** — not one abstracted pipeline — because swash and libass have fundamentally different font metrics, hinting, and rasterization outputs. Abstracting would leak backend-specific behaviors.

2. **Shared quantizer + encoder** — both backends produce RGBA bitmaps; the quantization and PGS encoding pipeline is backend-agnostic.

3. **Domain-Driven Design in pgs-encoder** — the `domain/` layer contains pure models with no I/O or encoding knowledge; the `encoding/` layer handles serialization. This separation makes the segment types testable in isolation.

4. **Self-built FontRegistry** — rather than relying on system fontconfig for all operations, the native backend parses font files directly and maintains its own fallback chain. This eliminates runtime dependencies and gives full control over font resolution.

5. **PotPlayer compatibility baked in** — `MAX_OBJECT_REFS=2` splitting and `palette_update=true` on all PCS are default behaviors, not opt-in flags.

---

## Continue Reading

- [🎨 Rendering Backends](rendering-backends.md) — Deep dive into backend differences
- [📦 PGS Encoder Design](pgs-encoder.md) — DDD architecture details
- [🎯 Color Quantizer](color-quantizer.md) — Quantization pipeline
- [🔤 Font System](font-system.md) — Font subsystem internals
- [🛠️ Development Guide](development.md) — Build, test, contribute
