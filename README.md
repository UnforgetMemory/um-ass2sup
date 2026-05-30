# ass2sup

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

## License

Dual-licensed under **MIT** or **Apache-2.0**.
