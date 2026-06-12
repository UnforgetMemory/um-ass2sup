# ass2sup

[![CI](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml/badge.svg)](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/ci.yml)
[![Audit](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/audit.yml/badge.svg)](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/audit.yml)
[![Release](https://github.com/UnforgetMemory/um-ass2sup/actions/workflows/release.yml/badge.svg)](https://github.com/UnforgetMemory/um-ass2sup/releases)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.85%2B-orange.svg)](https://www.rust-lang.org)
[![Version](https://img.shields.io/badge/version-0.5.0-blue.svg)](https://github.com/UnforgetMemory/um-ass2sup/releases)
[![Coverage](https://img.shields.io/badge/coverage-88.13%25-brightgreen.svg)](COVERAGE.md)

**[简体中文](README.md) | English**

> A Rust-based subtitle converter that transforms **ASS / SSA / SRT** subtitle files into Blu-ray **SUP / PGS** bitmap subtitle streams, with first-class **BDN XML** mastering output.

---

## Table of Contents

- [Overview](#overview)
- [Highlights](#highlights)
- [Quick Start](#quick-start)
- [Architecture](#architecture)
- [Workspace Layout](#workspace-layout)
- [Installation](#installation)
- [Usage](#usage)
  - [Single File](#single-file)
  - [Batch](#batch)
  - [Validation & Downgrade](#validation--downgrade)
  - [BDN XML Mastering](#bdn-xml-mastering)
  - [Multi-core Acceleration](#multi-core-acceleration)
- [CLI Reference](#cli-reference)
- [Library Usage](#library-usage)
- [Performance & Benchmarks](#performance--benchmarks)
- [Testing & Quality](#testing--quality)
- [Security](#security)
- [Contributing](#contributing)
- [License](#license)
- [Acknowledgments](#acknowledgments)

---

## Overview

`ass2sup` is a **modular Rust workspace** that converts open subtitle formats (ASS / SSA / SRT) into the bitmap subtitle streams (PGS / SUP) and Blu-ray mastering metadata (BDN XML) required for Blu-ray playback.

**Typical use cases:**

- Authoring BDMV (Blu-ray Disc Movie) with custom-rendered multi-language subtitle tracks
- Automating pipelines that process hundreds of TV-series subtitle files
- Faithful temporal reproduction of ASS effects (karaoke, `\move`, `\fad`, `\t`, etc.)
- Precise per-frame PTS calibration for non-integer framerates (23.976 / 29.97)

**What sets it apart:**

- **Truly native Rust**: no Python / Node dependencies, ships as a **single static binary**
- **Modular workspace**: 6 focused crates — render, quantize, encode are independently reusable
- **k-d tree acceleration**: 1080p quantization 908 ms → 353 ms (**2.57×**)
- **Blu-ray compliant**: precise NTSC 1001/1000 factor handling, multi-window splitting, EPG display-set segmentation
- **Tested & fuzzed**: 350+ unit/integration tests, proptest, insta snapshots, cargo-fuzz

---

## Highlights

### Input & Parsing

- **Multi-format**: ASS v4+, SSA v4, SubRip (`.srt`) with auto-detection via `SubtitleFormat::detect`
- **Full AST**: preserves Style, Dialogue, Font, Embedded Font information
- **SRT self-check**: `ass2sup in.srt --to-srt -o out.srt && diff in.srt out.srt` validates parser+serializer roundtrip

### Rendering

- **Glyph shaping**: `fontdb` + `rustybuzz` (HarfBuzz Rust bindings), full support for complex scripts (CJK, Arabic, Indic, etc.)
- **6-level font fallback chain**: user-specified → ASS `[Fonts]` embedded → system fontconfig
- **ASS effects**: karaoke (`\k` / `\kf` / `\ko` / `\kt`), motion (`\move`), fade (`\fad` / `\fade`), transform (`\t`), 3D rotation (`\frx` / `\fry`), anisotropic borders, vector clip (`\clip` / `\iclip`), scroll banners
- **Small palette dedup**: `HashSet<u32>` optimization, O(n²) → O(n)

### Quantization & Encoding

- **Median-Cut quantizer**: built-in k-d tree nearest-color lookup
- **Three dithering modes**: None / Floyd-Steinberg / Ordered
- **Palette reuse**: adjacent frames share palettes, reducing PGS segment header overhead
- **PGS encoding**: complete PCS / WDS / PDS / ODS display sets, precise NTSC 1001/1000 factor handling
- **Multi-window mode**: automatically splits large display sets at transparent row boundaries
- **Parallel quantization** (opt-in): rayon, **1.36×** speedup (30-event 1080p stress: 366 ms → 270 ms)

### Output & Distribution

- **SUP (`.sup`)**: Blu-ray Disc subtitle stream
- **BDN XML + PNG**: Blu-ray mastering metadata with per-frame `<Event InTC="..." />` references
- **SRT downgrade**: ASS → SRT for debugging and non-Blu-ray preview

### Engineering

- **Three CI workflows** (`ci.yml` / `audit.yml` / `release.yml`)
- **cargo-deny**: dependency advisory / ban / license / source audits
- **`#![warn(missing_docs)]`** enforced in public APIs
- **clippy `cast_lossless`** lint enforced workspace-wide
- **88.13% line coverage** (tarpaulin xml, lower bound)

---

## Quick Start

```bash
# 1. Convert a single subtitle file
ass2sup input.ass -o output.sup

# 2. Convert with validation
ass2sup input.ass -o output.sup --validate --overlap-warn

# 3. Batch-convert an entire season
ass2sup s01/*.ass -d ./sup_output/ --parallel
```

> See [Usage](#usage) for more.

---

## Architecture

```
            ┌────────────┐
            │  Input File│  ASS / SSA / SRT
            └─────┬──────┘
                  │ SubtitleFormat::detect
                  ▼
        ┌────────────────────┐
        │     ass-parser     │  → typed AST (events, styles, fonts)
        └─────────┬──────────┘
                  │ (optional)
                  ▼
        ┌──────────────────────┐
        │  subtitle-validator  │  syntax check / event overlap detection
        └─────────┬────────────┘
                  │
                  ▼
        ┌──────────────────────┐
        │   subtitle-renderer  │  fontdb + rustybuzz → per-frame RGBA bitmap
        └─────────┬────────────┘
                  │ (optional parallel via rayon)
                  ▼
        ┌──────────────────────┐
        │    color-quantizer   │  RGBA → indexed palette (≤255 colors + alpha)
        └─────────┬────────────┘
                  │ palette reuse
                  ▼
        ┌──────────────────────┐
        │     pgs-encoder      │  PGS / SUP segments (PCS / WDS / PDS / ODS)
        └─────────┬────────────┘
                  │
        ┌─────────┴──────────┐
        ▼                    ▼
  ┌──────────┐        ┌────────────┐
  │  .sup    │        │   BDN XML  │  + 0001.png, 0002.png, …
  └──────────┘        └────────────┘
```

---

## Workspace Layout

| Crate                    | Responsibility                                        | Key dependencies                  |
| ------------------------ | ----------------------------------------------------- | --------------------------------- |
| **`ass-parser`**         | Parse ASS / SSA / SRT, produce typed AST              | —                                 |
| **`subtitle-validator`** | Syntax validation, style checks, overlap detection    | `ass-parser`                      |
| **`subtitle-renderer`**  | Render subtitles to RGBA bitmaps (shaping, effects)   | `fontdb`, `rustybuzz`, `tiny-skia`|
| **`color-quantizer`**    | RGBA → indexed color (k-d tree accelerated)           | `tiny-skia`                       |
| **`pgs-encoder`**        | Quantized frame → PGS / SUP binary segments           | —                                 |
| **`bdn-xml`**            | Blu-ray mastering XML + PNG assets                    | `png`, `quick-xml`                |
| **`ass2sup-cli`**        | CLI binary wiring (`ass2sup`)                         | all of the above + `clap` + `rayon` |

All crates share versions through `[workspace.dependencies]` and use the unified `MIT OR Apache-2.0` license.

---

## Installation

### Prerequisites

- **Rust 1.75+** ([rustup](https://rustup.rs/))
- **fontconfig** (system library; built-in on macOS / Windows)
  - Debian / Ubuntu: `sudo apt install libfontconfig1-dev`
  - Fedora: `sudo dnf install fontconfig-devel`
  - macOS: `brew install fontconfig` (Homebrew usually has it)

### Build from source

```bash
git clone https://github.com/UnforgetMemory/um-ass2sup.git
cd um-ass2sup
cargo build --release
```

Binary: `target/release/ass2sup` (≈ 3.5 MB; smaller when stripped).

### Install to `$PATH`

```bash
cargo install --path crates/ass2sup-cli --locked
```

---

## Usage

### Single File

```bash
ass2sup input.ass -o output.sup
```

Default: 1920×1080 @ 23.976 fps. Customizable:

```bash
ass2sup input.ass -o output.sup -r 1280x720 -f 25.0
```

### Batch

```bash
# Explicit shell glob
ass2sup *.srt -d ./out/

# --glob (safer, cross-platform)
ass2sup --glob "subs/**/*.ass" --recursive -d ./out/

# Multi-core parallel
ass2sup --glob "subs/**/*.ass" --recursive --parallel -d ./out/
```

### Validation & Downgrade

```bash
# Validate only, no file written (CI-friendly: exit 0 OK / 1 errors)
ass2sup input.ass --check

# Validate with overlap warnings
ass2sup input.ass --check --validate --overlap-warn --overlap-mode strict

# ASS → SRT downgrade
ass2sup input.ass --to-srt -o output.srt

# SRT self-check: diff should be empty
ass2sup input.srt --to-srt -o out.srt && diff input.srt out.srt
```

### BDN XML Mastering

```bash
ass2sup input.srt --to-bdn -d ./bdn_out/
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

`BDN.xml` looks like:

```xml
<?xml version="1.0" encoding="utf-8"?>
<BDN Version="0.93">
  <Description>
    <Name>input</Name>
    <Language>eng</Language>
    <Format VideoFormat="NTSC">
      <Events>
        <Event InTC="00:00:01:01" OutTC="00:00:03:03" Forced="false">
          <Graphic File="0001.png" Area="0,0,1920,1080" />
        </Event>
        …
      </Events>
    </Format>
  </Description>
</BDN>
```

### Multi-core Acceleration

```bash
# Single file: parallel quantization (opt-in)
ass2sup input.ass -o output.sup --parallel-frames

# Batch: parallel files
ass2sup --glob "subs/**/*.srt" --parallel -d ./out/
```

The two are independent and stack: batch parallel × per-file parallel quantization.

---

## CLI Reference

| Option                              | Description                                       | Default          |
| ----------------------------------- | ------------------------------------------------- | ---------------- |
| `-o, --output <OUTPUT>`             | Output SUP path (single file)                     | —                |
| `-d, --output-dir <DIR>`            | Output directory (batch)                          | —                |
| `-r, --resolution <WxH>`            | Display resolution                                | `1920x1080`      |
| `-f, --fps <FLOAT>`                 | Framerate                                         | `23.976`         |
| `--validate`                        | Run validation before conversion                  | off              |
| `--overlap-warn`                    | Enable event overlap detection                    | off              |
| `--overlap-mode <MODE>`             | Overlap mode `strict` / `lenient`                 | `lenient`        |
| `--quantizer <ALGO>`                | Quantizer algorithm (current: `median-cut`)      | `median-cut`     |
| `--max-colors <1-255>`              | Max palette colors                                | `255`            |
| `--dither <METHOD>`                 | Dithering `none` / `floyd-steinberg` / `ordered`  | `floyd-steinberg`|
| `--check`                           | Parse and validate only, no write (exit 0/1)      | off              |
| `--to-srt`                          | Output SRT (ASS→SRT downgrade / SRT self-check)   | off              |
| `--to-bdn`                          | Output BDN XML + PNG (Blu-ray mastering)          | off              |
| `--parallel-frames`                 | Parallel quantization (single file, rayon)        | off              |
| `--parallel`                        | Parallel file processing (batch)                  | off              |
| `--dry-run`                         | Parse and validate only, no write                 | off              |
| `--force`                           | Convert even if validation fails                  | off              |
| `--font <NAME>`                     | Default font for SRT input                        | `Arial`          |
| `--font-size <PT>`                  | Default font size for SRT input                   | `48.0`           |
| `--glob <PATTERN>`                  | Glob pattern for input files                      | —                |
| `--recursive`                       | Recursive directory traversal with `--glob`       | off              |
| `--max-files <N>`                   | Max files processed in glob mode                  | unlimited        |
| `--quiet`                           | Suppress progress bar                             | off              |
| `--color <MODE>`                    | Color output `auto` / `always` / `never`           | `auto`           |
| `-v, --verbose`                     | Enable verbose logging                            | off              |
| `-h, --help`                        | Print help                                        | —                |
| `-V, --version`                     | Print version                                     | —                |

Input files **larger than 100 MiB are rejected** (`MAX_INPUT_SIZE_BYTES`) to prevent accidental video ingestion. Adjust the constant in source if you really need more.

---

## Library Usage

Each crate is an **independently reusable library**. `Cargo.toml`:

```toml
[dependencies]
ass-parser          = "0.3"
subtitle-validator  = "0.3"
subtitle-renderer   = { version = "0.3", features = ["..."] }
color-quantizer     = "0.3"
pgs-encoder         = "0.3"
bdn-xml             = "0.3"
```

Or path-dependency:

```toml
[dependencies]
ass-parser = { path = "../ass2sup/crates/ass-parser" }
```

Minimal example — parse + validate:

```rust
use ass_parser::AssFile;
use subtitle_validator::{validate, ValidationStage};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text  = std::fs::read_to_string("input.ass")?;
    let ass   = AssFile::parse(&text)?;
    let report = validate(&ass, ValidationStage::Full);
    if report.has_errors() {
        eprintln!("validation failed: {}", report);
        std::process::exit(1);
    }
    println!("OK: {} events", ass.events.len());
    Ok(())
}
```

More in `crates/*/examples/`:

```bash
cargo run --example parse_ass       -p ass-parser
cargo run --example quantize_image  -p color-quantizer
cargo run --example encode_sup      -p pgs-encoder
```

---

## Performance & Benchmarks

Full data in [BENCHMARKS.md](BENCHMARKS.md). Representative numbers (Linux WSL2 / Rust 1.77):

| Benchmark                          | Size     | Median     | Notes                        |
| ---------------------------------- | -------- | ---------- | ---------------------------- |
| `rle_small_64x32`                  | 64×32    | 2.84 µs    | single-segment RLE           |
| `rle_large_1920x1080`              | 1080p    | 2.45 ms    | single-segment RLE           |
| `quantizer_medium_320x180`         | 320×180  | 13.1 ms    | quantize + dither + palette  |
| `quantizer_large_1920x1080`        | 1080p    | 908 ms     | 353 ms after k-d tree (**2.57×**)|
| `pgs_encode_medium_320x180`        | 320×180  | 90.3 µs    | PGS encoding                 |
| `pgs_encode_ntsc_320x180`          | 320×180  | 91.1 µs    | NTSC 1001/1000 factor        |

Reproduce:

```bash
cargo bench --workspace
```

---

## Testing & Quality

- **350+ unit / integration tests** (`cargo test --workspace`)
- **proptest** (ass-parser determinism, SRT roundtrips, ASS lenient recovery, etc.)
- **insta snapshots** (`crates/ass2sup-cli/tests/snapshots/`) for CLI output stability
- **cargo-fuzz** targets (`decode_pgs`, `quantize_rgba`) — P26 found 2 PGS decoder OOB bugs, both fixed
- **88.13% line coverage** (cargo-tarpaulin)

Run everything:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo fmt --all -- --check
```

> See [COVERAGE.md](COVERAGE.md) for details. Architecture decisions in [`docs/adr/`](docs/adr/).

---

## Security

- **`SECURITY.md`**: vulnerability reporting flow (use GitHub Security Advisories — **not** public issues)
- **`deny.toml`**: cargo-deny audit (advisories / bans / licenses / sources)
- **`.github/workflows/audit.yml`**: weekly Monday 06:00 UTC + push/PR automatic audit

Known accepted warning: `RUSTSEC-2025-0119` (`number_prefix` unmaintained, transitive via `indicatif 0.17.11`) — waiting on upstream.

See [SECURITY.md](SECURITY.md).

---

## Contributing

Issues and PRs welcome. Suggested workflow:

```bash
# 1. Clone and build
git clone https://github.com/UnforgetMemory/um-ass2sup.git
cd um-ass2sup
cargo build

# 2. Run all gates
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo fmt --all -- --check

# 3. If adding a crate / public file, update root README and CHANGELOG
```

Before submitting a PR:

- [ ] All tests pass
- [ ] `clippy` zero warnings (`-- -D warnings`)
- [ ] `cargo doc` zero missing-doc warnings
- [ ] `cargo fmt` no drift
- [ ] New public items have `///` rustdoc
- [ ] `CHANGELOG.md` updated

---

## License

**Dual-licensed** under either of:

- [`MIT`](LICENSE-MIT)
- [`Apache-2.0`](LICENSE-APACHE)

at your option.

```
Copyright (c) 2024-2026 The um-ass2sup authors
```

See each `LICENSE-*` file for full terms.

---

## Acknowledgments

Built on the shoulders of:

- [`rustybuzz`](https://github.com/RazrFalcon/rustybuzz) — HarfBuzz Rust bindings
- [`tiny-skia`](https://github.com/RazrFalcon/tiny-skia) — pure-Rust Skia bindings
- [`fontdb`](https://github.com/RazrFalcon/fontdb) — font database
- [`clap`](https://github.com/clap-rs/clap) — CLI argument parsing
- [`rayon`](https://github.com/rayon-rs/rayon) — data parallelism
- Every crate in the [dependency list](Cargo.toml)

Thanks to all [contributors](https://github.com/UnforgetMemory/um-ass2sup/graphs/contributors).

---

<p align="center">
  <sub>Built with <code>cargo</code> · tracked on <code>master</code></sub>
</p>
