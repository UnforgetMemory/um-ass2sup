# ass2sup

[![CI](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml/badge.svg)](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml)
[![Audit](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/audit.yml/badge.svg)](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/audit.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.3.0-blue.svg)](https://github.com/UnforgetMemory/um-ass2sup/releases)

A Rust-based subtitle converter that transforms **ASS/SSA/SRT** subtitle files into Blu-ray **SUP/PGS** format. Built as a modular workspace of focused crates for parsing, validation, rendering, color quantization, and encoding.

## Features

- **Multi-format input**: ASS, SSA, and SRT subtitle parsing with full AST representation
- **PGS/SUP output**: Blu-ray compatible subtitle encoding (MPEG-2 PGS subtitle streams)
- **BDN XML support**: Blu-ray Disc Movie XML generation for subtitle authoring pipelines
- **Advanced ASS rendering**: Font management (fontdb + rustybuzz), text shaping, ASS effects (`\move`, `\fad`, `\t`, karaoke)
- **Color quantization**: Palette reduction to 256 colors for PGS compatibility
- **Validation**: Syntax checking and overlap detection with configurable severity levels
- **Karaoke support**: Per-syllable dual-layer rendering (`\k`, `\kf`, `\ko`, `\kt`)
- **Time animation**: `\move` interpolation, `\fad`/`\fade` alpha gradients, `\t` attribute transforms
- **Frame caching**: Thread-safe hash-based RenderedFrame cache for performance
- **NTSC-aware encoding**: Non-integer frame rate PTS (23.976/29.97) with exact 1001/1000 factor
- **Multi-window mode**: Split large PGS display sets at transparent row boundaries
- **Embedded fonts**: ASS `[Fonts]` section parsing with 6-level font fallback chain
- **Parallel processing**: Rayon-powered multi-file batch conversion
- **Progress reporting**: CLI progress bars via indicatif

## Project Structure

The workspace is organized into focused crates:

| Crate | Description |
|---|---|
| `ass-parser` | ASS/SSA/SRT subtitle file parser producing a typed AST |
| `subtitle-validator` | Syntax validation, style checking, and overlap detection |
| `subtitle-renderer` | Renders subtitles to RGBA bitmaps (font shaping, effects, transforms) |
| `color-quantizer` | Downsamples RGBA frames to ≤255 color palettes for PGS encoding |
| `pgs-encoder` | Encodes quantized frames into PGS/SUP binary format |
| `bdn-xml` | Generates BDN XML event files + PNG assets for Blu-ray authoring |
| `ass2sup-cli` | CLI application wiring all crates together (binary: `ass2sup`) |

## Installation

### Prerequisites

- Rust ≥ 1.75 (install via [rustup](https://rustup.rs/))
- System fontconfig library (for font matching)

### Build from source

```bash
git clone https://github.com/UnforgetMemory/um-ass2sup.git
cd ass2sup
cargo build --release
```

The binary will be at `target/release/ass2sup`.

## Usage

### Convert a single file

```bash
ass2sup input.ass -o output.sup
```

### Batch convert all files in a directory

```bash
ass2sup *.ass -d ./output/
```

### Specify display resolution and framerate

```bash
ass2sup input.ass -o output.sup -r 1920x1080 -f 25.0
```

### Run validation before converting

```bash
ass2sup input.ass -o output.sup --validate --overlap-warn
```

### Validate only (no conversion)

```bash
ass2sup input.ass --check   # exit 0 if OK, 1 if errors
```

### Convert ASS/SSA to SRT (downgrade)

```bash
ass2sup input.ass --to-srt -o output.srt
# SRT→SRT roundtrip is a lossless self-check:
ass2sup input.srt --to-srt -o out.srt && diff input.srt out.srt
```

### Convert to BDN XML + per-frame PNGs (Blu-ray authoring)

```bash
ass2sup input.srt --to-bdn -d ./bdn_output/
# produces BDN.xml + 0001.png, 0002.png, ...
```

### Parallel frame quantization (multi-core, opt-in)

```bash
ass2sup input.ass -o output.sup --parallel-frames   # rayon-parallel quantize
```

### CLI Options

| Flag | Description | Default |
|---|---|---|
| `-o, --output` | Output SUP file path (single file) | — |
| `-d, --output-dir` | Output directory (batch mode) | — |
| `-r, --resolution` | Display resolution `WxH` | `1920x1080` |
| `-f, --fps` | Frames per second | `23.976` |
| `--validate` | Run subtitle validation before conversion | off |
| `--overlap-warn` | Enable overlap detection warnings | off |
| `--overlap-mode` | Overlap detection mode (`strict`/`lenient`) | `lenient` |
| `--quantizer` | Quantizer algorithm (`median-cut`) | `median-cut` |
| `--max-colors` | Maximum palette colors (1–255) | `255` |
| `--dither` | Dithering method (`none`/`floyd-steinberg`/`ordered`) | `floyd-steinberg` |
| `--check` | Parse and validate only, no conversion (exit 0/1) | off |
| `--to-srt` | Output SRT format (ASS→SRT downgrade, also SRT self-check) | off |
| `--to-bdn` | Output BDN XML + per-frame PNGs (Blu-ray authoring) | off |
| `--parallel-frames` | Parallel quantize via rayon (single-file mode) | off |
| `--parallel` | Parallel file processing (batch mode) | off |
| `--dry-run` | Parse and validate only, no write | off |
| `--force` | Convert even if validation fails | off |
| `--font` | Default font for SRT input | `Arial` |
| `--font-size` | Default font size for SRT input | `48.0` |
| `--glob` | Glob pattern for batch input | — |
| `--recursive` | Recursive directory traversal with `--glob` | off |
| `--max-files` | Max files processed in glob mode | unlimited |
| `--quiet` | Suppress progress bar | off |
| `--color` | Color output mode (`auto`/`always`/`never`) | `auto` |
| `-v, --verbose` | Enable verbose logging | off |

## Example Pipeline

```bash
# Validate a subtitle file
ass2sup movie.ass --validate --overlap-warn

# Convert to SUP with 29.97fps NTSC framerate
ass2sup movie.ass -o movie.sup -r 1920x1080 -f 29.97

# Batch convert an entire season
ass2sup s01/*.ass -d ./sup_output/ -f 23.976
```

## Architecture

```
Input (.ass/.srt)
       │
       ▼
┌──────────────┐
│  ass-parser  │  Parse → typed AST (events, styles, fonts)
└──────┬───────┘
       │
       ▼
┌──────────────────────┐
│  subtitle-validator  │  Syntax check, overlap detection (optional)
└──────┬───────────────┘
       │
       ▼
┌──────────────────────┐
│  subtitle-renderer   │  Font shaping (rustybuzz) → RGBA bitmap per frame
└──────┬───────────────┘
       │
       ▼
┌──────────────────────┐
│   color-quantizer    │  RGBA → indexed palette (≤255 colors + alpha)
└──────┬───────────────┘
       │
       ▼
┌──────────────────────┐
│    pgs-encoder       │  Encode → PGS/SUP binary segments (PCS, WDS, PDS, ODS)
└──────┬───────────────┘
       │
       ▼
  Output (.sup)

  Alternative path (authoring):
       │
       ▼
┌──────────────────────┐
│      bdn-xml        │  Generate BDN XML + PNG assets
└─────────────────────┘
```

## Examples

Runnable examples live in each crate's `examples/` directory:

```bash
cargo run --example parse_ass       -p ass-parser
cargo run --example quantize_image  -p color-quantizer
cargo run --example encode_sup      -p pgs-encoder
```

## License

Dual-licensed under **MIT** or **Apache-2.0**.
