<p align="center">
  <img src=".github/logo.png" alt="ass2sup" width="200">
</p>

# AGENTS.md · 🤖 Project Instructions for AI Coding Agents

> **ASS/SSA/SRT → Blu-ray SUP/PGS subtitle converter**  
> Rust workspace · **8 crates** · **v2.7.1** · **Two rendering backends**

<p align="center">
  <a href="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml"><img src="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/audit.yml"><img src="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/audit.yml/badge.svg" alt="Audit"></a>
  <a href="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/release.yml/badge.svg"><img src="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/release.yml/badge.svg" alt="Release"></a>
  <img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License">
  <img src="https://img.shields.io/badge/rust-1.85%2B-orange.svg" alt="Rust 1.85+">
  <img src="https://img.shields.io/badge/version-2.7.1-blue.svg" alt="v2.7.1">
</p>

---

## 📋 Table of Contents

| § | Section | 章节 |
|---|---------|------|
| 1 | [🚀 Project Overview](#-project-overview) | 项目概览 |
| 2 | [🏗️ Build Modes](#️-build-modes) | 构建模式 |
| 3 | [📦 Workspace Layout](#-workspace-layout) | 工作区结构 |
| 4 | [⚙️ Rendering Stack (native-backend)](#️-rendering-stack-native-backend) | 渲染堆栈 |
| 5 | [🔤 Font Pipeline](#-font-pipeline) | 字体管线 |
| 6 | [📡 System Dependencies](#-system-dependencies) | 系统依赖 |
| 7 | [🔧 Build & Verify Commands](#-build--verify-commands) | 构建与验证 |
| 8 | [🎯 Quality Gates](#-quality-gates) | 质量门禁 |
| 9 | [🧪 Testing](#-testing) | 测试体系 |
| 10 | [🔄 CI Workflows](#-ci-workflows) | CI 工作流 |
| 11 | [📐 Style Conventions](#-style-conventions) | 代码规范 |
| 12 | [📁 Font Subsystem (v3, swash-native)](#-font-subsystem-v3-swash-native) | 字体子系统 |
| 13 | [🧩 pgs-encoder Architecture (DDD)](#-pgs-encoder-architecture-ddd) | PGS 编码器架构 |
| 14 | [🎨 color-quantizer Architecture](#-color-quantizer-architecture) | 色彩量化器架构 |
| 15 | [🏥 Surgical Fix Protocol](#-surgical-fix-protocol) | 精准修复协议 |
| 16 | [📎 Post-fix Verification Artifacts](#-post-fix-verification-artifacts) | 修复后验证 |
| 17 | [⚡ Performance Constraints](#-performance-constraints) | 性能约束 |
| 18 | [🧠 Memory Model](#-memory-model) | 内存模型 |

---

## 🚀 Project Overview

> 项目概览

ASS/SSA/SRT → Blu-ray SUP/PGS subtitle converter. Rust workspace, **8 crates**, v2.7.1.
**Two rendering backends**, selectable at build time via Cargo features:

- **`native-backend`** (default): self-built `FontRegistry` + `SimpleShaper` + `GlyphRasterizer` on swash — zero external font/shaper deps
- **`libass-backend`**: libass C library via FFI — delegates shaping/rasterization to libass

---

## 🏗️ Build Modes

> 构建模式

```bash
# Default (native only)
cargo build --release

# libass only
cargo build --release --no-default-features -F libass-backend

# Both (runtime --backend flag)
cargo build --release --no-default-features -F native-backend,libass-backend
```

---

## 📦 Workspace Layout

> 工作区结构

```
crates/
  ass-core/                       # ASS/SSA/SRT parser → strong AST (hand-written, 0 external deps)
  subtitle-validator/             # Syntax/overlap checks (depends on ass-core)
  subtitle-renderer/              # [feature=native-backend] RGBA bitmap rendering — FontRegistry + swash + tiny-skia
  libass-sys/                     # [feature=libass-backend] Manual FFI bindings for libass v0.17, header-only
  subtitle-renderer-libass/       # [feature=libass-backend] libass-based rendering pipeline
  color-quantizer/                # RGBA → indexed color (k-d tree accelerated, Floyd-Steinberg dither)
  pgs-encoder/                    # Indexed frames → PGS/SUP binary segments (DDD: domain/ + encoding/)
  bdn-xml/                        # Blu-ray mastering XML + PNG output
  ass2sup-cli/                    # CLI binary (ass2sup), feature-gated backend dispatch
```

Also at repo root: `ass2sup-libass/` — separate parallel workspace for libass-only builds (not part of main workspace).

### Crate Dependency Details

> 依赖详情

| Crate | Key deps | Doc lint |
|---|---|---|
| `ass-core` | thiserror, tracing | — (unsafe_code = "deny") |
| `subtitle-validator` | ass-core, thiserror | `#![warn(missing_docs)]` |
| `subtitle-renderer` | swash, tiny-skia, wide, parking_lot | — |
| `libass-sys` | — (no build.rs deps) | — |
| `subtitle-renderer-libass` | libass-sys, color-quantizer, pgs-encoder, bdn-xml | `#![warn(missing_docs)]` |
| `color-quantizer` | thiserror, tracing | `#![warn(missing_docs)]` |
| `pgs-encoder` | color-quantizer, png | — |
| `bdn-xml` | quick-xml, png | — |
| `ass2sup-cli` | clap, rayon, indicatif, glob, walkdir, serde, strsim | `#![warn(missing_docs)]` |

---

## ⚙️ Rendering Stack (native-backend)

> 渲染堆栈 — NO fontdb / NO cosmic-text / NO rustybuzz

```
Trace: ass-core parse → RenderContext (build_context) → shape_horizontal/vertical (SimpleShaper/swash)
  → glyph rasterization (GlyphRasterizer/swash) → composite_glyph → effects (blur/shadow/outline)
  → transform_layer (AffineTransform for scale/rotate/shear/perspective) → composite_subregion
```

---

## 🔤 Font Pipeline

> 字体管线

```
shape:    SimpleShaper::shape(text, font_data, font_size) → Vec<ShapedGlyph>
          Maps chars→glyph_id via swash FontRef.charmap(), records advance width
resolve:  FontRegistry.query() → FontId → get_font_data() → Vec<u8>
          Uses name-parsed weight/style fallback + font_map per-style fallback chain
rasterize: GlyphRasterizer::rasterize(font_data, glyph_id, font_size) → RasterizedGlyph
           Uses swash CacheKey for glyph cache lookup
composite: composite_glyph(layer, rasterized, x, y, color) — Porter-Duff over per pixel
```

---

## 📡 System Dependencies

> 系统依赖

Linux requires `libfontconfig1-dev` and `fonts-dejavu-core` for native-backend tests:

```bash
sudo apt-get install -y libfontconfig1-dev fonts-dejavu-core
```

libass-backend additionally requires `libass.so` at link time (v0.17+). The
`links/` directory at the repo root contains a pre-built copy for CI; for
production use, install via system package manager:

```bash
# Debian/Ubuntu
sudo apt-get install libass9
# macOS
brew install libass
```

---

## 🔧 Build & Verify Commands

> 构建与验证 (CI order)

```bash
# Full verification (CI order)
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo bench --workspace --no-run     # compile benchmarks only
cargo doc --workspace --no-deps
# Generate (not check) release binary
cargo build --release
```

There is no Makefile or task runner. Run commands directly.

### Single Crate Work

> 单 crate 操作

```bash
cargo test -p ass-core
cargo test -p pgs-encoder -- test_rle   # single test by name
cargo clippy -p color-quantizer --all-targets -- -D warnings
cargo run --release -p ass2sup-cli -- input.ass -o output.sup
```

---

## 🎯 Quality Gates

> 质量门禁

- **MSRV**: Rust 1.85 (enforced in CI, `Cargo.toml` `rust-version`)
- **Edition**: 2021
- **clippy**: `-D warnings` (zero warnings enforced across workspace)
- **fmt**: `cargo fmt --all -- --check` (no drift allowed)
- **doc**: `#![warn(missing_docs)]` on 4/8 crates (subtitle-validator, subtitle-renderer-libass, color-quantizer, ass2sup-cli); ass-core additionally denies `unsafe_code`
- **Profile**: `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`
- **cargo-deny**: `deny.toml` enforces license whitelist, no unknown registries/git sources
- **cargo-audit**: weekly + push/PR, `--deny warnings`, known advisory `RUSTSEC-2025-0119` ignored

---

## 🧪 Testing

> 测试体系

- **700+ unit/integration tests** across workspace (all pass: 700+ ok, 2 ignored)
- **proptest** in: ass-core, color-quantizer, pgs-encoder, bdn-xml
- **insta snapshots** in: `crates/ass2sup-cli/tests/snapshots/` (update with `cargo insta review`)
- **fuzz targets**: `crates/ass-core/fuzz/` (3 targets), `crates/color-quantizer/fuzz/` (1), `crates/pgs-encoder/fuzz/` (1)
- **benches**: `cargo bench --workspace` (criterion, html_reports)
- **Examples**: `cargo run --release --example parse_ass -p ass-core` (and similar for color-quantizer, pgs-encoder)

---

## 🔄 CI Workflows

> CI 工作流

- `ci.yml`: 4 jobs — check (rustfmt) → clippy → test (+ bench compile) → MSRV 1.85 (on push/PR to master)
- `audit.yml`: cargo-audit + cargo-deny (weekly Monday 06:00 UTC + push/PR)
- `release.yml`: cross-platform build matrix (Linux x86_64/aarch64, macOS ARM, Windows) + dry-run publish + GitHub Release on tag push

---

## 📐 Style Conventions

> 代码规范

- License: Apache-2.0
- Workspace dependencies managed in root `Cargo.toml` `[workspace.dependencies]`
- Fuzz crates excluded from workspace: `exclude = ["crates/*/fuzz"]`
- No `unwrap()`/`expect()` outside tests and CLI main
- `#[expect(clippy::*)]` over `#[allow(clippy::*)]` with justification

---

## 📁 Font Subsystem (v3, swash-native)

> 字体子系统

```
crates/subtitle-renderer/src/font/
  types.rs      # FontId, FontWeight, FontStyle, FontFace, FontQuery
  index.rs      # FontIndex — HashMap<(FamilyHash, Weight, Style), Vec<FontId>>
  database.rs   # FontDatabase — load/parse/store font data
  discovery.rs  # FontDiscovery — platform-specific font path scanning
  registry.rs   # FontRegistry — unified facade over system_db + user_db + index
  shaper.rs     # SimpleShaper — swash-based glyph shaping
  rasterizer.rs # GlyphRasterizer — swash-based glyph → alpha bitmap
  telemetry.rs  # FontEvent structured logging
  error.rs      # FontError domain errors
```

Cross-platform font fallback: 8-level chain (exact match → suffix-strip → alias → hardcoded CJK → cross-platform CJK scan → generic → SansSerif → any).

---

## 🧩 pgs-encoder Architecture (DDD, Wave 1 completed)

> PGS 编码器架构 — 领域驱动设计

```
crates/pgs-encoder/src/
  domain/                         # Pure domain model — no I/O, no encoding knowledge
    composition.rs                # CompositionState, ObjectComposition, WindowDef
    epoch.rs                      # EpochManager — object versioning, epoch lifecycle
    palette.rs                    # PaletteEntry, YCbCr conversion, color swap
    segment.rs                    # Segment, SegmentPayload (PCS/WDS/PDS/ODS/END), SupFile
    rle.rs                        # RLE encode, chunk_rle_data
    timing.rs                     # Frame rate codes, ms_to_90khz conversion
    mod.rs                        # Re-exports
  encoding/                       # Encoding—how domain objects serialize to binary
    display_set.rs                # DisplaySet builder: EpochStart/NormalCase/EpochContinue/PaletteOnly
    encoder.rs                    # PgsEncoder — frame → display set pipeline
    sup.rs                        # SUP file writer
    mod.rs                        # Re-exports
  color.rs                        # Color type re-exports
  encoder.rs                      # Legacy encoder (partial; new logic in encoding/)
  epoch.rs                        # Legacy epoch (partial; new logic in domain/epoch.rs)
  lib.rs                          # Crate root
  rle.rs                          # Legacy RLE (partial)
  types.rs                        # Legacy type aliases
```

### Key Architectural Constraints (from project memory)

> 关键架构约束

- `MAX_OBJECT_REFS=2`: PotPlayer crashes on PCS with >2 objects — chunks(2) splits multi-object display sets
- PotPlayer requires `palette_update=true` on all PCS; `num_objects=0` in palette_clear causes PotPlayer crash
- Show PCS for fade events must use alpha=0 palette (via `encode_multi_object_display_set_with_alpha(Some(0))`) to prevent 1-frame full-alpha flash
- `composition_number` increments after every `encode_frame` (wrapping_add), including NormalCase

---

## 🎨 color-quantizer Architecture

> 色彩量化器架构

```
crates/color-quantizer/src/
  color/                          # Color science
    space.rs                      # Color space definitions
    transfer.rs                   # Transfer functions
    delta_e.rs                    # Perceptual color difference
    tonemap.rs                    # Tone mapping
    mod.rs
  dither/                         # Dithering methods
    floyd_steinberg.rs            # Floyd-Steinberg error diffusion
    ordered.rs                    # Ordered Bayer dither
    adaptive.rs                   # Adaptive dither
    mod.rs
  quantize/                       # Palette generation
    median_cut.rs                 # Median-cut palette
    nearest.rs                    # K-D tree nearest-color lookup
    palette.rs                    # Palette management
    temporal.rs                   # Temporal palette reuse across frames
    naarahara.rs                  # Nara hair a palette mapping
    mod.rs
  frame/                          # Frame abstraction
    mod.rs
    owned.rs
    view.rs
    iter.rs
  pipeline.rs                     # Quantization pipeline orchestration
  error.rs                        # Domain error types
  lib.rs                          # Crate root
```

**Key**: k-d tree `find_nearest_index` accelerates palette mapping (2.57×). Temporal palette reuse reduces PDS overhead between adjacent frames.

---

## 🏥 Surgical Fix Protocol

> 精准修复协议 — 每个非平凡修复必须遵循

```
1. FULL-CHAIN INVESTIGATION
   - Trace the exact code path from ASS parse → RenderContext → shape → rasterize → composite
   - Identify ROOT CAUSE with file:line evidence — never treat symptoms
   - Verify with pixel-level ground truth (reference SUP comparison) where applicable

2. PLAN THEN CUT
   - Define the surgical boundary: what changes, what must NOT change
   - Single root cause per operation (multiple independent bugs = parallel ops)
   - Zero collateral damage — fix ONLY the broken path

3. VERIFY
   - cargo fmt + clippy + test (full workspace)
   - Generate .output/ artifacts from .localref/ for end-to-end verification
```

---

## 📎 Post-fix Verification Artifacts

> 修复后验证产出

After every completed fix:

```bash
# Generate SUP from .localref/ ASS files to .output/
timestamp=$(date +%Y%m%d-%H%M%S)
for ass in .localref/*.ass; do
  base=$(basename "$ass" .ass)
  cargo run --release -p ass2sup-cli -- "$ass" -o ".output/${base}-${timestamp}.sup"
  # BDN XML + PNG sequence for pixel-level inspection
  cargo run --release -p ass2sup-cli -- "$ass" --to-bdn -d ".output/${base}-${timestamp}/"
done
```

Only run when `.localref/` contains `.ass` files and after a fix that affects rendering output.

Output naming: `{original-name}-{YYYYMMDD-HHMMSS}.sup` + `{original-name}-{YYYYMMDD-HHMMSS}/` (BDN XML + PNG seq).

Run in foreground (not background) — completion reminder will deliver the result.

---

## ⚡ Performance Constraints

> 性能约束

- **No heap allocation in hot render paths** (glyph loop, composite, transform)
- **PixmapPool**: reuse Pixmap buffers via pool_get/pool_put (8 cached entries, wrapped in Mutex)
- **AffineTransform**: SIMD (wide::f32x4) bilinear interpolation in `apply_to_pixmap`
- **composite_over**: SIMD (wide::u32x4) Porter-Duff over for 4-pixel chunks
- **Parallel rendering**: rayon-based `par_iter()` in `build_display_set` — each worker holds 1 frame at a time (~8.3 MB at 1080p), no intermediate `Vec<RenderedFrame>`
- **Small palette dedup**: `HashSet<u32>` in quantizer, O(n²) → O(n)
- **k-d tree quantizer**: `find_nearest_index` for palette mapping acceleration (2.57×)

---

## 🧠 Memory Model

> 内存模型

- Renderer owns: `FontRegistryRenderResources` (registry + pool + font_map, all wrapped in `Mutex`)
- `build_context` produces one `RenderContext` per event per timestamp
- `render_event_font_registry` allocates one `layer: Pixmap` per event (pool_get → fill/outline/shadow → composite → pool_put)
- `transform_layer` allocates output buffer (the transform is approx 1:1 or smaller)
- Peak memory: `max_events_per_timestamp × layer_size + output_buffer`, typically < 50 MB at 1080p
