# 🎯 Color Quantizer

> **RGBA → indexed color pipeline with color science, dithering, and palette optimization**

---

## 📋 Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Color Science Module](#color-science-module)
- [Dithering Methods](#dithering-methods)
- [Quantization Methods](#quantization-methods)
- [Pipeline Orchestration](#pipeline-orchestration)
- [Temporal Palette Reuse](#temporal-palette-reuse)
- [Performance](#performance)

---

## Overview

The `color-quantizer` crate converts full RGBA bitmaps (4 bytes per pixel) into indexed-color images with a palette of ≤256 colors (255 + 1 transparent). This is a critical step because Blu-ray PGS supports a maximum of 256 palette entries per subtitle.

The quantizer sits between the rendering backends (which produce RGBA bitmaps) and the PGS encoder (which consumes indexed frames). It supports:

- **Two quantization algorithms**: Median-cut (primary), Naarahara (alternative)
- **Three dithering methods**: Floyd-Steinberg (default), Ordered, None
- **Temporal palette reuse** across adjacent frames to reduce PDS overhead
- **Optional HDR→SDR tone mapping**
- **k-d tree acceleration** for palette mapping (2.57× speedup)

---

## Architecture

```
crates/color-quantizer/src/
  color/                          # Color science
    space.rs                      # Color space definitions (sRGB, BT.709, BT.2020, etc.)
    transfer.rs                   # Transfer functions (gamma, PQ, HLG)
    delta_e.rs                    # Perceptual color difference (CIE76, CIE94, CIEDE2000)
    tonemap.rs                    # HDR → SDR tone mapping operators
    mod.rs
  dither/                         # Dithering methods
    floyd_steinberg.rs            # Floyd-Steinberg error diffusion
    ordered.rs                    # Ordered Bayer dither
    adaptive.rs                   # Adaptive dither
    mod.rs
  quantize/                       # Palette generation
    median_cut.rs                 # Median-cut palette
    nearest.rs                    # K-D tree nearest-color lookup
    palette.rs                    # Palette management utilities
    temporal.rs                   # Temporal palette reuse across frames
    naarahara.rs                  # Naarahara palette mapping
    mod.rs
  frame/                          # Frame abstraction
    mod.rs
    owned.rs                      # OwnedQuantizedFrame
    view.rs                       # QuantizedFrameView
    iter.rs                       # Pixel iterator
  pipeline.rs                     # ColorPipeline orchestration
  error.rs                        # Domain error types
  lib.rs                          # Crate root
```

---

## Color Science Module

The `color/` module provides foundational color science for accurate quantization.

### Color Spaces

```rust
pub enum ColorSpace {
    Srgb,       // Standard RGB (sRGB, IEC 61966-2-1)
    Bt709,      // ITU-R BT.709 (HDTV)
    Bt2020,     // ITU-R BT.2020 (UHDTV)
    LinearSrgb, // Linear sRGB (for accurate arithmetic)
    Xyz,        // CIE XYZ
    Lab,        // CIE L*a*b* (perceptual uniformity)
}
```

Color space conversions enable:
- Accurate perceptual color difference computation
- Tone mapping between HDR and SDR color spaces
- Linear-light dithering (avoiding gamma-induced hue shifts)

### Transfer Functions

```rust
pub enum TransferFunction {
    Gamma { gamma: f32 },  // Power-law gamma
    Srgb,                   // sRGB piecewise transfer
    Pq,                     // Perceptual Quantizer (SMPTE ST 2084)
    Hlg,                    // Hybrid Log-Gamma (ARIB STD-B67)
}
```

### Delta-E

Three perceptual difference metrics:

| Metric | Standard | Use Case |
|---|---|---|
| CIE76 | ΔE*ab | Simple, fast |
| CIE94 | ΔE*94 | Graphics, textiles |
| CIEDE2000 | ΔE*00 | Most accurate, preferred |

### Tone Mapping

Optional HDR→SDR tone mapping via `ToneMapOperator`:

```rust
pub enum ToneMapOperator {
    Reinhard,        // Reinhard global tone mapping
    Filmic,          // Filmic (ACES-like) tone mapping
    Hable,           // Hable's Uncharted 2 filmic
    Linear,          // Simple linear compression
}
```

---

## Dithering Methods

Dithering distributes quantization error to prevent banding in smooth gradients.

### Floyd-Steinberg (Default)

The classic error-diffusion dither. Distributes quantization error to four neighbors:

```
    X     7/16
 3/16  5/16  1/16
```

**Implementation details:**
- Error buffers use `i16` (4× less memory than `f64`)
- Operates in linear-light to avoid gamma-induced hue shifts
- Processes pixels in row-major order
- All four RGBA channels independently diffused

```rust
pub fn dither(rgba: &[u8], width: u32, height: u32, palette: &[[u8; 4]]) -> Vec<u8>;
```

### Ordered Dither

Bayer matrix dithering with a fixed 8×8 threshold matrix. Computationally cheaper than error diffusion:

```rust
pub fn dither(rgba: &[u8], width: u32, height: u32, palette: &[[u8; 4]]) -> Vec<u8>;
```

### Adaptive Dither

A hybrid approach that applies dithering adaptively based on local image content.

### No Dither

Simple nearest-color mapping without error diffusion. Fastest but prone to banding in gradients:

```rust
// Modes: None uses direct find_nearest_index per pixel
(DitherMethod::None, _) => {
    // Pixel-by-pixel nearest palette color lookup
}
```

---

## Quantization Methods

### Median-Cut (Default)

The primary quantization algorithm. Splits the color space recursively along the median of the longest axis:

```rust
pub fn quantize(pixels: &[[u8; 4]], max_colors: usize) -> Vec<[u8; 4]>;
```

**Algorithm:**
1. Collect all opaque pixels (alpha > 0)
2. If pixels already fit in `max_colors`, deduplicate and return
3. Otherwise, apply median-cut: recursively split color boxes at the median of the longest dimension
4. Each final box contributes its average color to the palette

### Naarahara

An alternative palette mapping algorithm (named after the Nara period). Provides different color distribution characteristics compared to median-cut.

### k-d Tree Nearest-Color Lookup

After palette generation, pixel mapping uses a **k-d tree** for O(log n) nearest-color lookups instead of O(n) linear search:

```rust
pub fn find_nearest_index(color: &[u8; 4], palette: &[[u8; 4]]) -> u8;
```

This provides a **2.57× speedup** (measured: 1080p quantization drops from ~908 ms to ~353 ms).

### Palette Management

```rust
// Palette construction
let palette = if has_transparent {
    full_palette.push([0u8, 0, 0, 0]); // transparent entry at index 0
};
full_palette.extend(median_cut_palette);

// Transparent index is always 0
let transparent_index = 0u8;
```

---

## Pipeline Orchestration

The `ColorPipeline` struct orchestrates the full quantization flow:

```rust
pub struct ColorPipeline {
    pub max_colors: usize,          // Max palette entries (default: 255)
    pub dither: DitherMethod,       // Dithering method (default: FloydSteinberg)
    pub color_space: ColorSpace,    // Source color space (default: Srgb)
    pub quantize_method: QuantizeMethod, // Quantization algorithm (default: MedianCut)
    pub tonemap: Option<ToneMapOperator>, // Optional HDR→SDR tone mapping
}
```

### Builder API

```rust
let pipeline = ColorPipeline::new()
    .with_max_colors(128)
    .with_dither(DitherMethod::None)
    .with_color_space(ColorSpace::Bt709);
```

### Quantization Flow

```
RGBA bytes
  │
  ├── [Optional] Tone mapping (HDR → SDR)
  │     ↓
  ├── Collect opaque pixels (filter alpha > 0)
  │     ↓
  ├── Build palette
  │     ├── If ≤ max_colors unique: deduplicate
  │     └── Else: MedianCut or Naarahara
  │     ↓
  ├── Insert transparent entry (index 0) if needed
  │     ↓
  └── Map pixels → indices
        ├── FloySteinberg: error diffusion
        ├── Ordered: Bayer matrix
        └── None: direct k-d tree lookup
              ↓
        QuantizedFrame {
            palette: Vec<Rgba>,
            indices: Vec<u8>,
            transparent_index: 0,
            width, height, color_space, ...
        }
```

### QuantizedFrame

```rust
pub struct QuantizedFrame {
    pub width: u32,
    pub height: u32,
    pub palette: Vec<Rgba>,
    pub indices: Vec<u8>,           // Width × height index buffer
    pub transparent_index: u8,
    pub x: u16,                     // Subtitle position X
    pub y: u16,                     // Subtitle position Y
    pub color_space: ColorSpace,
    pub pts_ms: u64,                // Presentation timestamp (ms)
    pub duration_ms: u64,           // Display duration (ms)
}
```

---

## Temporal Palette Reuse

Between adjacent frames, subtitle content often changes minimally. The quantizer can **reuse the previous frame's palette** to avoid emitting a redundant PDS segment:

```rust
pub fn quantize_with_prev(
    &self,
    rgba: &[u8],
    width: u32, height: u32,
    prev_frame: Option<&QuantizedFrame>,
) -> QuantizedFrame;
```

### Reuse Decision

1. For each pixel in the new frame, check if its nearest color in the previous palette is within a threshold (∆E ≤ 30)
2. If **all** pixels map within threshold, reuse the previous palette entirely
3. Otherwise, fall back to full quantization

### Benefits

- Reduces PDS overhead between adjacent frames
- Particularly effective for fade transitions
- Lower data rate in the SUP stream

---

## Performance

### Benchmark Results

| Benchmark | Size | Median Time | Notes |
|---|---|---|---|
| `quantizer_medium_320x180` | 320×180 | 13.1 ms | Full pipeline: quantize + dither + palette |
| `quantizer_large_1920x1080` | 1080p | 353 ms | After k-d tree acceleration (2.57×) |

### Optimization Techniques

| Technique | Impact |
|---|---|
| **k-d tree nearest-color** | 2.57× speedup for palette mapping |
| **HashSet palette dedup** | O(n²) → O(n) for unique color collection |
| **i16 error buffers** | 4× less memory than f64 in Floyd-Steinberg |
| **Rayon parallelism** | Per-frame parallel quantization (opt-in) |

---

## Continue Reading

- [🏛️ Architecture](architecture.md) — Where the quantizer fits in the pipeline
- [📦 PGS Encoder Design](pgs-encoder.md) — How quantized frames are encoded
- [🎨 Rendering Backends](rendering-backends.md) — How RGBA frames are produced
