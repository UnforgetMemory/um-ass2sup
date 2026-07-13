# 🔤 Font System

> **FontRegistry, shaping, rasterization, and font fallback in the native backend**

---

## 📋 Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [FontRegistry](#fontregistry)
- [FontDatabase](#fontdatabase)
- [FontDiscovery](#fontdiscovery)
- [Font Index](#font-index)
- [Font Fallback Chain](#font-fallback-chain)
- [SimpleShaper](#simpleshaper)
- [GlyphRasterizer](#glyphrasterizer)
- [PixmapPool](#pixmappool)
- [Type System](#type-system)
- [Performance Considerations](#performance-considerations)

---

## Overview

The font subsystem is the heart of the `subtitle-renderer` crate's native backend. It is a **self-built, pure-Rust** replacement for the traditional fontconfig + FreeType + HarfBuzz stack. Built on `swash`, it handles everything from font discovery to glyph rasterization without any C/C++ dependencies.

```
                        ┌──────────────────┐
                        │   FontDiscovery   │  Platform-specific font scanning
                        └────────┬─────────┘
                                 │ font files
                                 ▼
                        ┌──────────────────┐
                        │   FontDatabase    │  Load, parse, store font data
                        └────────┬─────────┘
                                 │ indexed fonts
                                 ▼
                        ┌──────────────────┐
                        │    FontIndex      │  (Family, Weight, Style) → Vec<FontId>
                        └────────┬─────────┘
                                 │ query
                                 ▼
                        ┌──────────────────┐
                        │   FontRegistry   │  Unified facade
                        └────────┬─────────┘
                                 │ font data
                    ┌────────────┼────────────┐
                    ▼            ▼            ▼
            ┌────────────┐ ┌────────────┐ ┌────────────┐
            │ shape()    │ │ query()    │ │ rasterize()│
            │ SimpleShaper│ │            │ │ GlyphRast. │
            └────────────┘ └────────────┘ └────────────┘
```

---

## Architecture

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

The rendering integration lives in:

```
crates/subtitle-renderer/src/renderer/
  font_registry_renderer.rs    # render_event_font_registry() — full event rendering
  font_registry_karaoke.rs     # Karaoke-specific rendering
  layout_font_registry.rs      # Text layout (horizontal/vertical shaping)
  mod.rs                       # PixmapPool, composite utilities
```

---

## FontRegistry

`FontRegistry` is the central facade for all font operations. It owns:

- A `FontDatabase` for system fonts
- A `FontDatabase` for user-specified fonts
- A `FontIndex` for fast lookups

### Query Flow

```
FontRegistry::query(family, weight, style)
  │
  ├── 1. Exact match in font index
  │       (family_hash, weight, style) → FontId
  │
  ├── 2. If not found, walk the 8-level fallback chain
  │
  └── 3. Return FontId → caller gets font data via get_font_data()
```

### Key Methods

```rust
impl FontRegistry {
    /// Query for a font matching the given parameters.
    pub fn query(&self, query: &FontQuery) -> Option<FontId>;

    /// Get font data bytes by FontId.
    pub fn get_font_data(&self, id: FontId) -> Option<&[u8]>;

    /// Find a font by exact family name (fast path).
    pub fn find_exact(&self, family: &str, weight: FontWeight, style: FontStyle) -> Option<FontId>;

    /// Register fonts from a directory.
    pub fn register_directory(&mut self, path: &Path) -> Result<usize, FontError>;

    /// Register a single font from raw bytes.
    pub fn register_font(&mut self, data: Vec<u8>) -> Result<FontId, FontError>;
}
```

---

## FontDatabase

Stores loaded font data and parsed metadata:

```rust
pub struct FontDatabase {
    entries: Vec<FontEntry>,
    next_id: u32,
}

struct FontEntry {
    id: FontId,
    data: Vec<u8>,
    family: String,
    weight: FontWeight,
    style: FontStyle,
    stretch: FontStretch,
}
```

Loading parses the font binary to extract family name, weight, and style using swash's `FontRef`. The raw data is kept in memory for later use by the shaper and rasterizer.

---

## FontDiscovery

Platform-specific font path scanning:

```rust
pub struct FontDiscovery;

impl FontDiscovery {
    /// Scan system font directories and return discovered font file paths.
    pub fn scan_system_fonts() -> Result<Vec<PathBuf>, FontError>;

    /// Scan a specific directory for font files.
    pub fn scan_directory(path: &Path) -> Result<Vec<PathBuf>, FontError>;
}
```

### Linux

Uses `fontconfig` via system library to enumerate installed fonts (`/usr/share/fonts/`, `~/.local/share/fonts/`, etc.).

### macOS

Searches standard macOS font directories (`/System/Library/Fonts/`, `~/Library/Fonts/`).

### Windows

Searches Windows font directories (`C:\Windows\Fonts\`).

### Accepted File Types

| Extension | Format |
|---|---|
| `.ttf` | TrueType |
| `.otf` | OpenType |
| `.ttc` | TrueType Collection |

---

## Font Index

A hash map for fast font lookups:

```rust
pub struct FontIndex {
    entries: HashMap<(u64, FontWeight, FontStyle), Vec<FontId>>,
}
```

Keyed by:
- **Family name hash** (64-bit hash of normalized family name)
- **FontWeight** (100..900, with named constants)
- **FontStyle** (Normal, Italic, Oblique)

Multiple fonts can share the same (family, weight, style) — the first registered takes priority.

---

## Font Fallback Chain

When the requested font is not available, the registry walks an **8-level fallback chain**:

### The 8 Levels

```
Level 1: Exact match
  ├── (family, weight, style) matches exactly in index
  │
Level 2: Suffix-strip match
  ├── Strip common suffixes ("-Bold", " Bold", " MT", "WGL")
  │   and retry exact match
  │
Level 3: Alias lookup
  ├── Check hardcoded alias table
  │   ("Arial" ←→ "Helvetica", "Meiryo" ←→ "Yu Gothic", etc.)
  │
Level 4: Hardcoded CJK fonts
  ├── Try known CJK font names:
  │   "Noto Sans CJK SC", "Source Han Sans", "SimSun",
  │   "Microsoft YaHei", "Hiragino Sans GB", "WenQuanYi Micro Hei"
  │
Level 5: Cross-platform CJK scan
  ├── Scan all registered fonts for CJK coverage
  │
Level 6: Generic family fallback
  ├── Map CSS/spec generic families:
  │   "sans-serif" → SansSerif
  │   "serif" → Serif
  │   "monospace" → Monospace
  │
Level 7: SansSerif fallback
  ├── Any font with SansSerif classification
  │
Level 8: Any font
  └── The first registered font in the database
```

### Alias Table

The alias table maps common font family names across platforms:

| Requested | Resolves To |
|---|---|
| `Arial` | `Helvetica`, `Liberation Sans` |
| `Times New Roman` | `Times`, `Liberation Serif` |
| `Courier New` | `Courier`, `Liberation Mono` |
| `SimHei` | `Noto Sans CJK SC` |
| `Meiryo` | `Yu Gothic` |
| `Malgun Gothic` | `Noto Sans KR` |

---

## SimpleShaper

A basic glyph shaper using swash. Maps characters to glyphs and records advance widths.

```rust
pub fn shape(text: &str, font_data: &[u8], font_size: f32) -> Result<Vec<ShapedGlyph>, FontError>;
```

### Shaping Process

```
text: "Hello"
  │
  ├── 'H' → charmap.map('H') → glyph_id 43  → advance: 27.4px
  ├── 'e' → charmap.map('e') → glyph_id 69  → advance: 16.2px
  ├── 'l' → charmap.map('l') → glyph_id 76  → advance: 8.1px
  ├── 'l' → charmap.map('l') → glyph_id 76  → advance: 8.1px
  └── 'o' → charmap.map('o') → glyph_id 79  → advance: 16.2px
```

### Output: ShapedGlyph

```rust
pub struct ShapedGlyph {
    pub glyph_id: u16,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
}
```

### Limitations

- **No kerning** (no pair adjustment kerning)
- **No complex shaping** (no Arabic, Devanagari, or other complex-script shaping)
- **No ligatures** (No "fi" → ﬁ glyph substitution)
- **No bidirectional text** handling

For left-to-right scripts (Latin, CJK, Greek, Cyrillic), these limitations are acceptable for subtitle rendering.

---

## GlyphRasterizer

Converts a glyph ID + font data into an alpha bitmap using swash's scale and render pipeline.

```rust
pub fn rasterize(font_data: &[u8], glyph_id: u16, size: f32) -> Result<RasterizedGlyph, FontError>;
```

### Rasterization Process

```
GlyphRasterizer::rasterize(font_data, glyph_id=43, size=48.0)
  │
  ├── FontRef::from_index(font_data, 0) — parse font
  ├── ScaleContext::builder(font).size(48.0).hint(false).build()
  │     → Creates a swash scaler with hinting disabled
  ├── Render::new(&[Source::Outline])
  │     .format(Format::Alpha)
  │     .render(&mut scaler, glyph_id)
  │     → Produces alpha bitmap
  │
  └── RasterizedGlyph {
        data: Vec<u8>,    // alpha values, row-major
        width: u32,       // bitmap width
        height: u32,      // bitmap height
        left: i32,        // bearing X (from glyph origin)
        top: i32,         // bearing Y (from glyph origin)
      }
```

### Output: RasterizedGlyph

```rust
pub struct RasterizedGlyph {
    pub data: Vec<u8>,   // Alpha channel values (not RGBA — just coverage)
    pub width: u32,
    pub height: u32,
    pub left: i32,
    pub top: i32,
}
```

The rasterizer uses **alpha-only** rendering (Format::Alpha), producing single-byte coverage values rather than full RGBA. This reduces memory traffic and simplifies compositing.

### Glyph Cache

The renderer integrates with swash's `CacheKey` for glyph cache lookups, avoiding re-rasterization of identical glyphs within the same rendering pass.

---

## PixmapPool

An 8-slot buffer cache that avoids repeated heap allocations for rendering layers:

```rust
pub struct PixmapPool {
    pool: Vec<Pixmap>,
    max_size: usize,  // default: 8
}
```

### Usage in Rendering

```
render_event_font_registry()
  │
  ├── pool_get(lw, lh) → Pixmap (or None if pool exhausted)
  │
  ├── Fill layer with fill/outline/shadow
  │
  ├── composite_over() to place glyphs
  │
  ├── transform_layer() if affine transforms needed
  │
  └── pool_put(layer) → return to pool
```

Each event renders into a cached Pixmap, minimizing allocation churn. After effects (shadow, outline, blur) and compositing, the buffer is returned to the pool.

---

## Type System

### FontId

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontId(u32);
```

A simple incrementing identifier for registered fonts.

### FontWeight

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontWeight(pub u16);
// 100=Thin, 200=ExtraLight, 300=Light, 400=Normal/Regular,
// 500=Medium, 600=SemiBold, 700=Bold, 800=ExtraBold, 900=Black
```

### FontStyle

```rust
pub enum FontStyle {
    Normal,
    Italic,
    Oblique,
}
```

### FontFace

```rust
pub struct FontFace {
    pub family: String,
    pub weight: FontWeight,
    pub style: FontStyle,
    pub stretch: FontStretch,
}
```

### FontQuery

```rust
pub struct FontQuery<'a> {
    pub family: &'a str,
    pub weight: FontWeight,
    pub style: FontStyle,
}
```

---

## Performance Considerations

### No Heap Allocation in Hot Paths

The glyph rendering loop, compositing, and transform operations avoid heap allocation. Pre-allocated buffers from `PixmapPool` are reused across events.

### SIMD Compositing

- **`composite_over`**: Uses `wide::u32x4` for Porter-Duff over compositing in 4-pixel chunks
- **`apply_to_pixmap`**: Uses `wide::f32x4` for bilinear interpolation in affine transforms

### Rendering Resource Structure

```rust
pub struct FontRegistryRenderResources {
    pub registry: Mutex<FontRegistry>,
    pub pixmap_pool: Mutex<PixmapPool>,
    pub font_map: HashMap<String, Vec<String>>,
}
```

All wrapped in `Mutex` for thread-safe parallel rendering with rayon.

---

## Continue Reading

- [🏛️ Architecture](architecture.md) — Where the font system fits in the pipeline
- [🎨 Rendering Backends](rendering-backends.md) — Native backend overview
- [🛠️ Development Guide](development.md) — Build and test commands
