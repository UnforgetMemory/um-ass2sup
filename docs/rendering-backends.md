# 🎨 Rendering Backends

> **Native (swash/tiny-skia) vs libass FFI — architecture, differences, and build modes**

---

## 📋 Table of Contents

- [Overview](#overview)
- [native-backend (Default)](#native-backend-default)
- [libass-backend](#libass-backend)
- [Build Modes](#build-modes)
- [Rendering Differences](#rendering-differences)
- [When to Use Which](#when-to-use-which)
- [The `--compat-vsfilter` Flag](#the---compat-vsfilter-flag)
- [Backend Comparison Table](#backend-comparison-table)

---

## Overview

`um-ass2sup` is architecturally unique among ASS→SUP converters in offering **two independent rendering backends**, selectable at compile time. This is not a plugin system — each backend is a separate crate with its own rendering pipeline, font subsystem, and dependencies.

Both backends produce RGBA bitmaps that feed into the **shared** color quantizer and PGS encoder pipeline. The quantizer and encoder are completely backend-agnostic.

---

## native-backend (Default)

### Technology Stack

```
swash (shape + rasterize) → tiny-skia (composite)
```

- **swash**: Pure-Rust font engine providing glyph shaping and rasterization
- **tiny-skia**: Pure-Rust port of the Skia graphics library for bitmap compositing
- **wide**: SIMD acceleration via explicit SIMD vectors (f32x4, u32x4)

### Components

| Component | Crate Location | Role |
|---|---|---|
| `FontRegistry` | `subtitle-renderer/src/font/registry.rs` | Font index and data store |
| `SimpleShaper` | `subtitle-renderer/src/font/shaper.rs` | Character→glyph mapping via swash |
| `GlyphRasterizer` | `subtitle-renderer/src/font/rasterizer.rs` | Glyph→alpha bitmap via swash |
| `render_event_font_registry` | `subtitle-renderer/src/renderer/font_registry_renderer.rs` | Full event rendering pipeline |
| `PixmapPool` | `subtitle-renderer/src/renderer/mod.rs` | 8-slot buffer cache |

### Rendering Stack Detail

```
ass-core parse
  → RenderContext (build_context)
    → shape_horizontal / shape_vertical (SimpleShaper/swash)
      → glyph rasterization (GlyphRasterizer/swash)
        → composite_glyph (Porter-Duff over)
          → effects (blur/shadow/outline)
            → transform_layer (AffineTransform: scale/rotate/shear/perspective)
              → composite_subregion
```

### Key Characteristics

- **Zero C/C++ runtime dependencies** — single static binary
- **SIMD-accelerated**: `wide::u32x4` for Porter-Duff compositing, `wide::f32x4` for affine transform bilinear interpolation
- **Self-built font system**: no fontdb, no cosmic-text, no rustybuzz — everything is custom on top of swash
- **Unhinted glyph metrics**: swash produces raw, unhinted glyph outlines — resulting in wider, taller glyphs compared to FreeType-hinted output

---

## libass-backend

### Technology Stack

```
libass.so (shape + rasterize) → quantize → encode
```

- **libass v0.17+**: The standard ASS subtitle rendering library (C)
- **libass-sys**: Hand-written FFI bindings (no build.rs, header-only)
- **subtitle-renderer-libass**: Wraps libass output and feeds it into the shared quantizer

### Components

| Component | Crate Location | Role |
|---|---|---|
| `libass-sys` | `libass-sys/` | Raw FFI declarations for libass API |
| `subtitle-renderer-libass` | `subtitle-renderer-libass/` | Libass rendering pipeline → RGBA frame |

### Rendering Pipeline

```
ass-core parse
  → Convert AST to ASS-formatted string
    → libass_ass_read_memory() → libass renders frame
      → libass generates RGBA bitmap via ass_render_frame()
        → RGBA frame buffer → color-quantizer
```

### Key Characteristics

- **Perfect ASS specification compatibility** — matches ffmpeg, mpv, VLC rendering
- **System library dependency**: requires `libass.so` at link time
- **FreeType-hinted glyphs**: glyphs are smaller and tighter than swash output
- **Delegates shaping and rasterization to C code** — no Rust font pipeline involved

---

## Build Modes

### Default: Native Only

```bash
cargo build --release
```

Features: `native-backend` (default). Produces a static binary with no runtime dependencies.

### Libass Only

```bash
cargo build --release --no-default-features -F libass-backend
```

Requires `libass.so` installed on the system. Uses the separate `ass2sup-libass/` workspace for dependency isolation.

### Dual Backend

```bash
cargo build --release --no-default-features -F native-backend,libass-backend
```

Builds both backends. Selectable at runtime via:

```bash
ass2sup input.ass -o output.sup --backend native
ass2sup input.ass -o output.sup --backend libass
```

### System Dependencies

| Dependency | Required For | Install (Debian/Ubuntu) | Install (macOS) |
|---|---|---|---|
| `libfontconfig1-dev` | native-backend (font discovery) | `sudo apt install libfontconfig1-dev` | — (included) |
| `fonts-dejavu-core` | native-backend (test fonts) | `sudo apt install fonts-dejavu-core` | — |
| `libass9` | libass-backend | `sudo apt install libass9` | `brew install libass` |

---

## Rendering Differences

The two backends produce **visibly different** output for the same ASS input. This is not a bug — it is a fundamental consequence of using different font engines.

### Glyph Metrics

| Metric | native-backend (swash) | libass-backend (FreeType) |
|---|---|---|
| Hinting | None (unhinted raw outlines) | Full FreeType hinting |
| Glyph width | **Wider** (+18% measured) | Standard |
| Glyph height | **Taller** (+27% measured) | Standard |
| Synthetic bold | swash built-in embolden | `FT_Outline_Embolden()` VSFilter semantics |

> Measured: DejaVu Sans 60px / Outline=2 / Shadow=2 — native-backend bounding box 274×42 px vs libass 232×33 px.

### Implications

- **native-backend SUP output appears visibly larger and bolder** than libass-rendered subtitles on playback
- For pixel-identical output matching ffmpeg, mpv, or VLC, use the **libass-backend**
- **There is no byte-level SUP compatibility** between the two backends: they use different quantizers (same code but different RGBA input), different palette strategies, and the rendering differences propagate through the entire pipeline

---

## When to Use Which

### Choose native-backend when:
- You want a **static binary** with zero runtime dependencies
- You are processing subtitles for your own use and appearance is subjective
- You value **performance** and **deployment simplicity** over compatibility
- You are authoring BDMV discs and want larger, more readable subtitles

### Choose libass-backend when:
- You need **pixel-identical output** to ffmpeg/mpv/VLC
- You are collaborating with others who use traditional toolchains
- The ASS file relies on **obscure or edge-case ASS features** only libass implements correctly
- You need **VSFilter-compatible** subtitle dimensions

---

## The `--compat-vsfilter` Flag

An experimental flag that applies a ~0.764× font-size scaling factor to make swash output closer to VSFilter sizing:

```bash
ass2sup input.ass -o output.sup --compat-vsfilter
```

**Important caveats:**
- Only the font size is scaled — glyph outlines, spacing, and positioning still differ
- This is an approximation, not a pixel-perfect compatibility mode
- The constant 0.764 is empirically derived and may not generalize to all fonts and sizes

---

## Backend Comparison Table

| Dimension | native-backend | libass-backend |
|---|---|---|
| Rendering engine | swash (pure Rust) | libass via FFI (C) |
| Font system | Self-built FontRegistry | System fontconfig + FreeType |
| Glyph metrics | Unhinted, wider/taller | FreeType-hinted, standard |
| Dependencies | None at runtime | `libass.so` (v0.17+) |
| Binary size | Static, ~5-10 MB | Dynamic link to libass (~1 MB) |
| SIMD acceleration | `wide` crate (f32x4, u32x4) | Delegated to libass |
| ASS compatibility | Full (all major features) | Full + edge cases |
| Output appearance | Larger, bolder | Standard VSFilter-like |
| Build time | Faster (pure Rust) | Slower (C compilation) |
| Platform support | Linux, macOS, Windows | Linux, macOS (where libass is available) |

---

## Continue Reading

- [🏛️ Architecture](architecture.md) — Full pipeline overview
- [🔤 Font System](font-system.md) — Native backend font subsystem
- [🛠️ Development Guide](development.md) — Build instructions and commands
