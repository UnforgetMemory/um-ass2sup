<p align="center">
  <a href="https://github.com/UnforgetMemory/um-ass2sup">
    <img src=".github/logo.png" alt="ass2sup" width="200">
  </a>
</p>

<h1 align="center">ass2sup</h1>

<p align="center">
  <a href="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml"><img src="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml/badge.svg" alt="CI"></a>
  <a href="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/audit.yml"><img src="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/audit.yml/badge.svg" alt="Audit"></a>
  <a href="https://github.com/UnforgetMemory/um-ass2sup/releases"><img src="https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/release.yml/badge.svg" alt="Release"></a>
  <a href="LICENSE-APACHE"><img src="https://img.shields.io/badge/license-Apache--2.0-blue.svg" alt="License: Apache-2.0"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/rust-1.85%2B-orange.svg" alt="Rust 1.85+"></a>
  <a href="https://github.com/UnforgetMemory/um-ass2sup/releases"><img src="https://img.shields.io/badge/version-2.7.1-blue.svg" alt="Version"></a>
</p>

<p align="center">
  [**简体中文**](README.md) | <strong>English</strong>
</p>

> A Rust subtitle converter that transforms **ASS / SSA / SRT** into Blu-ray **SUP / PGS** bitmap subtitle streams, with **BDN XML** mastering output.

---

## 📋 Table of Contents

- [🚀 Overview](#-overview)
- [🏗️ Differences from Traditional Toolchains](#%EF%B8%8F-differences-from-traditional-toolchains)
- [🎯 Dual Rendering Backends](#-dual-rendering-backends)
- [⚡ Highlights](#-highlights)
- [📦 Quick Start](#-quick-start)
- [🏛️ Architecture](#%EF%B8%8F-architecture)
- [📁 Workspace Layout](#-workspace-layout)
- [🔧 Installation](#-installation)
- [💻 Usage](#-usage)
- [📖 CLI Reference](#-cli-reference)
- [📚 Library Usage](#-library-usage)
- [📊 Performance & Benchmarks](#-performance--benchmarks)
- [🧪 Testing & Quality](#-testing--quality)
- [🔒 Security](#-security)
- [🤝 Contributing](#-contributing)
- [📄 License](#-license)
- [🙏 Acknowledgments](#-acknowledgments)

---

## 🚀 Overview

`ass2sup` converts open subtitle formats (ASS/SSA/SRT) into PGS/SUP bitmap subtitle streams natively supported by Blu-ray players, with BDN XML mastering as a secondary output.

**Use cases:**
- Authoring BDMV discs with custom-rendered multi-language subtitle tracks
- Automating pipelines that process hundreds of TV-series subtitle files
- Retaining ASS effects (karaoke, `\move`, `\fad`, `\t`) with frame-accurate timing
- Precise per-frame PTS calibration for non-integer framerates (23.976 / 29.97)

---

## 🏗️ Differences from Traditional Toolchains

### Background

Traditional ASS→SUP conversion pipelines typically follow:

```
ASS → AviSynth (avs2pipe) → easyavs2bdnxml/easyavs2sup → SUP
```

These tools rely on **libass** (via a VSFilter compatibility layer) for subtitle rendering. The resulting SUP glyph sizes, advance widths, and overall appearance all follow libass conventions. **`um-ass2sup` is not a drop-in Rust replacement for any tool in that pipeline. It is an architecturally distinct codebase.**

### Fundamental Differences

| Dimension | Traditional (easyavs2bnxml, etc.) | um-ass2sup native-backend |
|---|---|---|
| Rendering engine | libass (via VSFilter/AviSynth) | **swash** (pure-Rust font engine) |
| Glyph metrics | FreeType hinted advance | **swash unhinted raw metrics** |
| Output appearance | Smaller glyphs, tighter spacing | **Larger, wider glyphs** (+18% wider, +27% taller)¹ |
| Deployment | Python / AviSynth / VSFilter dependency chain | **Single static binary, zero runtime deps** |
| Font resolution | System FreeType + fontconfig | **Self-built FontRegistry, pure-Rust rasterization** |
| Synthetic bold | `FT_Outline_Embolden()` VSFilter semantics | **swash built-in embolden** (different parameters) |
| Design goal | VSFilter compatibility | Blu-ray compliance + performance |

¹ Measured: DejaVu Sans 60px / Outline=2 / Shadow=2 — native-backend bounding box 274×42 px vs libass 232×33 px.

### Implications

- **native-backend SUP output appears visibly larger and bolder than libass-rendered subtitles** on playback. This is an inherent difference between the swash and FreeType hinting engines — not a bug.
- For pixel-identical output matching ffmpeg, mpv, or VLC, use the **libass-backend** build (`--no-default-features -F libass-backend`).
- An experimental `--compat-vsfilter` flag applies a ~0.764× font-size scaling factor to bring swash output closer to VSFilter sizing in practice — though glyph outlines and spacing will still differ.
- **There is no byte-level SUP compatibility between `um-ass2sup` and easyavs2bnxml**: they use different quantizers, different palette strategies, and different display-set segmentation logic. The same ASS input will produce structurally different SUP files.

---

## 🎯 Dual Rendering Backends

`ass2sup` offers two rendering paths, selected at compile time via Cargo features:

### native-backend (default)

Pure Rust, zero C/C++ runtime dependencies:

```
swash (shaping + rasterization) → tiny-skia (bitmap compositing)
```

- `FontRegistry` + `SimpleShaper` + `GlyphRasterizer` atop swash
- 8-level font fallback chain (exact match → suffix-strip → alias → hardcoded CJK → cross-platform scan → generic → SansSerif → any)
- SIMD acceleration via `wide`: Porter-Duff compositing, affine transform bilinear interpolation
- Ideal for lightweight deployments that don't require libass compatibility

### libass-backend

Invokes system libass (v0.17+) via FFI:

```
libass.so (shaping + rasterization) → quantization → PGS encoding
```

- Perfect ASS specification compatibility
- Rendering matches other libass-based tools (ffmpeg, mpv, VLC)
- Ideal for workflows requiring pixel-identical ASS output

### Build

```bash
# Default (native backend)
cargo build --release

# libass only
cargo build --release --no-default-features -F libass-backend

# Both (runtime --backend switch)
cargo build --release --no-default-features -F native-backend,libass-backend
```

---

## ⚡ Highlights

### Input & Parsing
- ASS v4+, SSA v4, SubRip (`.srt`) auto-detection via `SubtitleFormat::detect`
- Hand-written parser, zero external parsing dependencies
- Full AST preserving Style/Dialogue/Font information
- SRT self-check: `ass2sup in.srt --to-srt -o out.srt && diff in.srt out.srt`

### Rendering
- **native-backend**: swash-based shaping, 8-level font fallback, full ASS effects support
- **libass-backend**: libass native rendering, spec-perfect compatibility
- ASS effects: karaoke (`\k`/`\kf`/`\ko`/`\kt`), motion (`\move`), fade (`\fad`/`\fade`), transform (`\t`), 3D rotation, anisotropic borders, vector clip, scroll banners

### Quantization & Encoding
- Median-Cut quantizer with k-d tree nearest-color lookup
- Three dithering modes: None / Floyd-Steinberg / Ordered
- Inter-frame palette reuse — reduces PDS overhead
- Full PGS display sets (PCS/WDS/PDS/ODS), NTSC 1001/1000 factor
- PotPlayer `MAX_OBJECT_REFS=2` compatibility: automatic multi-object display set splitting
- Fade-in/fade-out PDS-only optimization (no ODS redraw)
- Parallel quantization via rayon (opt-in)

### Output
- SUP (`.sup`): Blu-ray Disc subtitle stream
- BDN XML + PNG: Blu-ray mastering XML descriptor
- SRT downgrade: ASS → SRT for debugging

---

## 📦 Quick Start

```bash
# Single file
ass2sup input.ass -o output.sup

# With validation
ass2sup input.ass -o output.sup --validate --overlap-warn

# Batch
ass2sup s01/*.ass -d ./sup_output/ --parallel
```

---

## 🏛️ Architecture

```
            ┌────────────┐
            │  Input     │  ASS / SSA / SRT
            └─────┬──────┘
                  │
                  ▼
         ┌─────────────────┐
         │    ass-core     │  → typed AST
         └────────┬────────┘
                  │ optional
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
         │  DDD domain: domain/ (model) + encoding/ (serialize)  │
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

## 📁 Workspace Layout

### Main workspace (8 crates)

| Crate | Responsibility | Key deps | Doc lint |
|---|---|---|---|
| **`ass-core`** | ASS/SSA/SRT parser, typed AST | thiserror, tracing | `unsafe_code = "deny"` |
| **`subtitle-validator`** | Syntax/overlap detection | ass-core, thiserror | `#![warn(missing_docs)]` |
| **`subtitle-renderer`** | [native] RGBA bitmap rendering | swash, tiny-skia, wide, parking_lot | — |
| **`libass-sys`** | [libass] libass v0.17 FFI (header-only) | — | — |
| **`subtitle-renderer-libass`** | [libass] libass rendering pipeline | libass-sys, color-quantizer, pgs-encoder, bdn-xml | `#![warn(missing_docs)]` |
| **`color-quantizer`** | RGBA → indexed, k-d tree | thiserror, tracing | `#![warn(missing_docs)]` |
| **`pgs-encoder`** | Frames → PGS/SUP (DDD: domain/ + encoding/) | color-quantizer, png | — |
| **`bdn-xml`** | Blu-ray mastering XML + PNG | quick-xml, png | — |
| **`ass2sup-cli`** | `ass2sup` binary, feature-gated backend | clap, rayon, indicatif, serde | `#![warn(missing_docs)]` |

### Standalone workspace

- `ass2sup-libass/` — separate Cargo workspace for libass-only builds (outside main workspace)

---

## 🔧 Installation

### Prerequisites

- **Rust 1.85+** ([rustup](https://rustup.rs/))
- Linux native-backend: `sudo apt install libfontconfig1-dev fonts-dejavu-core`
- Linux libass-backend: `sudo apt install libass9`
- macOS: `brew install libass`

### Build from source

```bash
git clone https://github.com/UnforgetMemory/um-ass2sup.git
cd um-ass2sup
cargo build --release
```

Binary: `target/release/ass2sup`.

### Install to `$PATH`

```bash
cargo install --path crates/ass2sup-cli --locked
```

---

## 💻 Usage

### Single file

```bash
ass2sup input.ass -o output.sup
# Custom resolution and framerate
ass2sup input.ass -o output.sup -r 1280x720 -f 25.0
# Select rendering backend (dual-backend build)
ass2sup input.ass -o output.sup --backend libass
```

### Batch

```bash
ass2sup *.srt -d ./out/
ass2sup --glob "subs/**/*.ass" --recursive -d ./out/
ass2sup --glob "subs/**/*.ass" --recursive --parallel -d ./out/
```

### Validation & downgrade

```bash
# Validate only (CI-friendly, exit 0/1)
ass2sup input.ass --check
# Validate with overlap warnings
ass2sup input.ass --check --validate --overlap-warn --overlap-mode strict
# ASS → SRT downgrade
ass2sup input.ass --to-srt -o output.srt
# SRT self-check
ass2sup input.srt --to-srt -o out.srt && diff input.srt out.srt
```

### BDN XML mastering

```bash
ass2sup input.ass --to-bdn -d ./bdn_out/
```

Output:

```
bdn_out/
└── input/
    ├── BDN.xml
    ├── 0001.png
    ├── 0002.png
    └── ...
```

### Multi-core acceleration

```bash
# Per-file parallel quantization
ass2sup input.ass -o output.sup --parallel-frames
# Multi-file parallel
ass2sup --glob "subs/**/*.ass" --parallel -d ./out/
```

---

## 📖 CLI Reference

| Option | Description | Default |
|---|---|---|
| `-o, --output <OUTPUT>` | Output SUP path (single file) | — |
| `-d, --output-dir <DIR>` | Output directory (batch) | — |
| `-r, --resolution <WxH>` | Display resolution | `1920x1080` |
| `-f, --fps <FLOAT>` | Framerate | `23.976` |
| `--backend <BACKEND>` | Render backend (dual-backend build) `native` / `libass` | `native` |
| `--validate` | Run validation before conversion | off |
| `--overlap-warn` | Event overlap detection | off |
| `--overlap-mode <MODE>` | Overlap mode `strict` / `lenient` | `lenient` |
| `--quantizer <ALGO>` | Quantizer algorithm | `median-cut` |
| `--max-colors <1-255>` | Max palette colors | `255` |
| `--dither <METHOD>` | Dithering method | `floyd-steinberg` |
| `--check` | Validate only, no write (exit 0/1) | off |
| `--to-srt` | Output SRT | off |
| `--to-bdn` | Output BDN XML + PNG | off |
| `--parallel-frames` | Per-file parallel quantization | off |
| `--parallel` | Multi-file parallel | off |
| `--dry-run` | Validate only, no write | off |
| `--force` | Convert despite validation failure | off |
| `--font <NAME>` | Default font for SRT input | `Arial` |
| `--font-size <PT>` | Default font size for SRT input | `48.0` |
| `--glob <PATTERN>` | Glob pattern for input files | — |
| `--recursive` | Recursive with `--glob` | off |
| `--max-files <N>` | Max files in glob mode | unlimited |
| `--quiet` | Suppress progress bar | off |
| `--color <MODE>` | Color output `auto` / `always` / `never` | `auto` |
| `-v, --verbose` | Verbose logging | off |
| `-h, --help` | Print help | — |
| `-V, --version` | Print version | — |

Input files exceeding **100 MiB** are rejected (`MAX_INPUT_SIZE_BYTES`) to prevent accidental video ingestion.

---

## 📚 Library Usage

Each crate is independently reusable. `Cargo.toml`:

```toml
[dependencies]
ass-core            = "2.7"
subtitle-validator  = "2.7"
subtitle-renderer   = { version = "2.7", features = ["..."] }
color-quantizer     = "2.7"
pgs-encoder         = "2.7"
bdn-xml             = "2.7"
```

Or path dependencies:

```toml
[dependencies]
ass-core = { path = "../ass2sup/crates/ass-core" }
```

Parse + validate example:

```rust
use ass_core::AssFile;
use subtitle_validator::{validate, ValidationStage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = std::fs::read_to_string("input.ass")?;
    let ass  = AssFile::parse(&text)?;
    let report = validate(&ass, ValidationStage::Full);
    if report.has_errors() {
        eprintln!("validation failed: {}", report);
        std::process::exit(1);
    }
    println!("OK: {} events", ass.events.len());
    Ok(())
}
```

More examples:

```bash
cargo run --example parse_ass       -p ass-core
cargo run --example quantize_image  -p color-quantizer
cargo run --example encode_sup      -p pgs-encoder
```

---

## 📊 Performance & Benchmarks

Full data in [BENCHMARKS.md](BENCHMARKS.md). Representative values (Linux / Rust 1.85):

| Benchmark | Size | Median | Notes |
|---|---|---|---|
| `rle_small_64x32` | 64×32 | 2.84 µs | Single-segment RLE |
| `rle_large_1920x1080` | 1080p | 2.45 ms | Single-segment RLE |
| `quantizer_medium_320x180` | 320×180 | 13.1 ms | Quantize + dither + palette |
| `quantizer_large_1920x1080` | 1080p | 353 ms | After k-d tree (2.57×) |
| `pgs_encode_medium_320x180` | 320×180 | 90.3 µs | PGS encoding |
| `pgs_encode_ntsc_320x180` | 320×180 | 91.1 µs | NTSC 1001/1000 factor |

```bash
cargo bench --workspace
```

---

## 🧪 Testing & Quality

- **700+ unit/integration tests** (`cargo test --workspace`, all passing)
- **proptest**: ass-core (parse determinism, SRT roundtrip, ASS lenient recovery), color-quantizer, pgs-encoder, bdn-xml
- **insta snapshots**: `crates/ass2sup-cli/tests/snapshots/`
- **cargo-fuzz**: ass-core (3 targets), color-quantizer (1), pgs-encoder (1)
- **criterion benches**: `cargo bench --workspace` (HTML reports)
- **clippy `-D warnings`** — zero warnings across workspace
- **`cargo fmt --all -- --check`** — no drift allowed
- **`#[expect(clippy::*)]`** preferred over `#[allow(clippy::*)]` with justification

```bash
# Full verification
cargo check --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo bench --workspace --no-run
cargo doc --workspace --no-deps
```

---

## 🔒 Security

- **SECURITY.md**: vulnerability reporting (GitHub Security Advisories — **not** public issues)
- **deny.toml**: cargo-deny (advisories / bans / licenses / sources)
- **audit.yml**: weekly Monday 06:00 UTC + push/PR automatic audit
- Known ignored: `RUSTSEC-2025-0119` (`number_prefix` unmaintained, transitive via `indicatif`)

See [SECURITY.md](SECURITY.md).

---

## 🤝 Contributing

PRs and issues welcome. Before submitting:

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings
- [ ] `cargo doc --workspace --no-deps` — zero missing docs
- [ ] `cargo fmt --all -- --check` — no drift
- [ ] New public APIs have `///` rustdoc
- [ ] `CHANGELOG.md` updated

---

## 📄 License

[`Apache-2.0`](LICENSE-APACHE)

```
Copyright (c) 2024-2026 The um-ass2sup authors
```

See `LICENSE-APACHE` for full terms.

---

## 🙏 Acknowledgments

Built on the shoulders of:

### Rust ecosystem
- [`swash`](https://github.com/dfrg/swash) — font shaping and rasterization
- [`tiny-skia`](https://github.com/RazrFalcon/tiny-skia) — pure-Rust Skia bitmap compositing
- [`clap`](https://github.com/clap-rs/clap) — CLI argument parsing
- [`rayon`](https://github.com/rayon-rs/rayon) — data parallelism
- [`wide`](https://github.com/lokathor/wide) — SIMD acceleration
- [`parking_lot`](https://github.com/Amanieu/parking_lot) — fast mutex
- [`quick-xml`](https://github.com/tafia/quick-xml) — XML serialization
- [`png`](https://github.com/image-rs/image-png) — PNG encoding
- [`criterion`](https://github.com/bheisler/criterion.rs) — benchmarking
- [`proptest`](https://github.com/proptest-rs/proptest) — property-based testing
- [`indicatif`](https://github.com/console-rs/indicatif) — progress bars

### External libraries
- [`libass`](https://github.com/libass/libass) — ASS subtitle renderer (v0.17+, optional backend)
- [`fontconfig`](https://www.freedesktop.org/wiki/Software/fontconfig/) — font discovery (Linux)

### Standards reference
- [Blu-ray Disc Read-Only Format](https://www.blu-raydisc.info/) — PGS/SUP specification

Thanks to all [contributors](https://github.com/UnforgetMemory/um-ass2sup/graphs/contributors).

---

<p align="center">
  <sub>Built with <code>cargo</code> · tracked on <code>master</code></sub>
</p>
