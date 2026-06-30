# ass2sup-libass — libass-based ASS to SUP/PGS converter

A Rust DDD project that converts ASS/SSA/SRT subtitles to Blu-ray SUP/PGS format using **libass** for rendering instead of a custom font/shaping pipeline.

## Why this project?

Existing ASS→SUP tooling is fragmented:
- **EasyASS + BDSup2Sub++** — mature but multi-tool pipeline
- **Subtitle Edit** — GUI-only
- **ffmpeg** — no SUP muxer

This project fills the gap: a **single CLI command** that uses libass for rendering and produces standard Blu-ray SUP output.

## Architecture

```
ASS file
  │
  ▼
libass (C library via FFI)
  │ ass_render_frame()
  ▼
ASS_Image linked list (shadow → outline → character)
  │ compose_frame()
  ▼
RGBA buffer (1920×1080)
  │ crop_to_tight_bbox
  │ color-quantizer → QuantizedFrame
  ▼
pgs-encoder → SUP binary
```

### Crate structure

| Crate | Responsibility |
|-------|---------------|
| `libass-sys` | Manual FFI bindings for libass 0.17 |
| `ass2sup-core` | Domain core — render, compose, quantize, encode |
| `ass2sup-cli` | CLI binary (clap) |

### DDD modules (ass2sup-core)

**Domain layer:**
- `renderer` — libass lifecycle wrapper
- `composer` — ASS_Image → RGBA compositing
- `timeline` — frame timestamp generation
- `pipeline` — orchestration orchestrator

**Infrastructure layer:**
- `vendor` — RGBA helper functions
- `pgs_adapter` — bridges domain types to pgs-encoder

## Building

### Prerequisites

- Rust 1.85+ (2021 edition)
- libass 0.17+ (runtime only; no -dev headers needed)

```bash
# Install libass (Debian/Ubuntu)
sudo apt-get install libass9

# Install libass (macOS)
brew install libass
```

### Build

```bash
cargo build --release -p ass2sup-cli
```

The binary at `target/release/ass2sup` is ready to use.

## Usage

```bash
# Basic conversion
ass2sup input.ass -o output.sup

# With custom resolution and framerate
ass2sup input.ass -o output.sup -r 1920x1080 -f 23.976

# Output BDN XML + PNG instead of SUP
ass2sup input.ass --to-bdn -d ./bdn_output/

# Verbose logging
ass2sup input.ass -o output.sup --verbose
```

### Options

| Flag | Description | Default |
|------|-------------|---------|
| `-o, --output` | Output SUP file | Input name with `.sup` |
| `--to-bdn` | BDN XML + PNG mode | SUP mode |
| `-d, --output-dir` | Output directory (BDN mode) | `bdn_output/` |
| `-r, --resolution` | Display resolution | ASS PlayRes or 1920×1080 |
| `-f, --fps` | Framerate | 23.976 |
| `--max-colors` | Max palette colours (1–255) | 255 |
| `--dither` | Dither method | floyd-steinberg |
| `--font` | Default font family | — |
| `--font-dir` | Additional font directory | — |
| `-v, --verbose` | Verbose logging | false |

## How libass integration works

1. `ass_library_init()` creates the library handle
2. `ass_renderer_init()` creates the renderer
3. `ass_set_frame_size()` and `ass_set_storage_size()` configure output dimensions
4. `ass_read_memory()` parses the ASS script
5. `ass_set_fonts_dir()` **must be called before** `ass_set_fonts()`
6. `ass_set_fonts()` configures font lookup via fontconfig
7. For each timestamp: `ass_render_frame()` returns `ASS_Image*` linked list
8. Each image has a 1-byte-per-pixel alpha mask + separate RGBA color
9. `compose_frame()` composites layers: `rgba = (color & 0xFFFFFF00) | alpha`
10. The RGBA frame is quantized and fed to pgs-encoder

## Reused crates

This project reuses three crates from the existing ass2sup workspace:

| Crate | Purpose |
|-------|---------|
| `color-quantizer` | RGBA → indexed palette with k-d tree, 3 dither methods |
| `pgs-encoder` | PGS segment encoding (PCS/WDS/PDS/ODS/END) |
| `bdn-xml` | BDN XML + PNG output |

## License

Apache-2.0