# 🛠️ Development Guide

> **Build commands, testing, quality gates, CI workflows, and contribution guidelines for um-ass2sup v3.0.0**

---

## 📋 Table of Contents

- [Prerequisites](#prerequisites)
- [Build Commands](#build-commands)
- [Testing](#testing)
- [Quality Gates](#quality-gates)
- [CI Workflows](#ci-workflows)
- [Contributing](#contributing)
- [Project Conventions](#project-conventions)
- [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Rust Toolchain

- **Rust 1.85+** (MSRV enforced in CI). Install via [rustup](https://rustup.rs/).

```bash
rustup toolchain install stable
rustup default stable
```

### System Dependencies

#### Linux (native-backend)

```bash
sudo apt-get install -y libfontconfig1-dev fonts-dejavu-core
```

- `libfontconfig1-dev` — font discovery for the native backend's `FontDiscovery`
- `fonts-dejavu-core` — test fonts used by SimpleShaper and GlyphRasterizer unit tests

#### Linux (libass-backend)

```bash
sudo apt-get install libass9
```

#### macOS

```bash
brew install libass
```

---

## Build Commands

### Standard Build (Native Backend)

```bash
cargo build --release
```

Binary: `target/release/ass2sup`

### Debug Build

```bash
cargo build
```

### Libass-Only Build

```bash
cargo build --release --no-default-features -F libass-backend
```

### Dual Backend Build

```bash
cargo build --release --no-default-features -F native-backend,libass-backend
```

### Check (Fast Compile-Only Verification)

```bash
cargo check --workspace --all-targets
```

### Single Crate Operations

```bash
# Build
cargo build -p pgs-encoder

# Test
cargo test -p ass-core

# Run single test by name
cargo test -p pgs-encoder -- test_rle

# Clippy
cargo clippy -p color-quantizer --all-targets -- -D warnings

# Run CLI
cargo run --release -p ass2sup-cli -- input.ass -o output.sup
```

### Install to PATH

```bash
cargo install --path crates/ass2sup-cli --locked
```

---

## Testing

### Full Workspace Test Suite

```bash
cargo test --workspace --all-targets
```

This runs **700+ unit/integration tests** across all 8 crates. All tests pass (2 ignored — known platform-specific exclusions).

### Documentation Tests

```bash
cargo test --workspace --doc
```

### Run Tests for a Specific Crate

```bash
cargo test -p ass-core
cargo test -p color-quantizer
cargo test -p pgs-encoder
```

### Run a Single Test by Name

```bash
cargo test -p pgs-encoder -- test_rle_small
```

### Property-Based Testing (proptest)

The following crates include proptest suites:

| Crate | What's Tested |
|---|---|
| **ass-core** | Parse determinism, SRT roundtrip, ASS lenient recovery |
| **color-quantizer** | Quantization invariants, palette constraints |
| **pgs-encoder** | Segment encoding roundtrips, RLE correctness |
| **bdn-xml** | XML serialization roundtrips |

Run them with:

```bash
cargo test -p ass-core     # includes proptest
cargo test -p color-quantizer
```

### Snapshot Tests (insta)

Snapshot files live in `crates/ass2sup-cli/tests/snapshots/`. To update:

```bash
cargo insta review
```

### Fuzz Testing

Fuzz targets are in `crates/*/fuzz/` (excluded from the main workspace):

| Crate | Fuzz Targets |
|---|---|
| **ass-core** | 3 targets (parser, events, styles) |
| **color-quantizer** | 1 target |
| **pgs-encoder** | 1 target |

Run with:

```bash
cd crates/ass-core/fuzz && cargo fuzz run parse
```

### Benchmarks

```bash
cargo bench --workspace
```

Uses criterion.rs with HTML report generation. Some representative benchmarks:

| Benchmark | Size | Median Time | Notes |
|---|---|---|---|
| `rle_small_64x32` | 64×32 | 2.84 µs | Single-segment RLE |
| `rle_large_1920x1080` | 1080p | 2.45 ms | Single-segment RLE |
| `quantizer_medium_320x180` | 320×180 | 13.1 ms | Quantize + dither + palette |
| `quantizer_large_1920x1080` | 1080p | 353 ms | After k-d tree (2.57×) |
| `pgs_encode_medium_320x180` | 320×180 | 90.3 µs | PGS encoding |

---

## Quality Gates

Every change must pass these checks (enforced in CI):

### 1. Compilation

```bash
cargo check --workspace --all-targets
```

### 2. Formatting

```bash
cargo fmt --all -- --check
```

Zero drift allowed. Auto-fix with:

```bash
cargo fmt --all
```

### 3. Clippy Lints

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

- **Zero warnings** enforced
- Prefer `#[expect(clippy::*)]` with justification over `#[allow(clippy::*)]`

### 4. Documentation

Four crates enforce `#![warn(missing_docs)]`:

- `subtitle-validator`
- `subtitle-renderer-libass`
- `color-quantizer`
- `ass2sup-cli`

Additionally, `ass-core` denies `unsafe_code`.

```bash
cargo doc --workspace --no-deps
```

### 5. Tests

```bash
cargo test --workspace --all-targets
cargo test --workspace --doc
```

### 6. Benchmarks (compile check)

```bash
cargo bench --workspace --no-run
```

### 7. Full CI Order (one-liner)

```bash
cargo check --workspace --all-targets && \
cargo fmt --all -- --check && \
cargo clippy --workspace --all-targets -- -D warnings && \
cargo test --workspace --all-targets && \
cargo test --workspace --doc && \
cargo bench --workspace --no-run && \
cargo doc --workspace --no-deps && \
cargo build --release
```

### Release Profile

| Setting | Value |
|---|---|
| `opt-level` | `3` |
| `lto` | `"thin"` |
| `codegen-units` | `1` |

---

## CI Workflows

The project runs three CI workflows on GitHub Actions:

### `ci.yml` — Full Check

Triggered on push/PR to `master`. Four jobs:

```
check (rustfmt) → clippy → test (+ bench compile) → MSRV 1.85
```

### `audit.yml` — Security Audit

Weekly Monday 06:00 UTC + push/PR.

- `cargo-audit` — dependency vulnerability scanning (`--deny warnings`)
- `cargo-deny` — license whitelist, source registry validation
- Known ignored advisory: `RUSTSEC-2025-0119` (`number_prefix` unmaintained, transitive via `indicatif`)

### `release.yml` — Release Build

Triggered on tag push. Cross-platform build matrix:

| Platform | Architecture |
|---|---|
| Linux | x86_64, aarch64 |
| macOS | ARM (Apple Silicon) |
| Windows | x86_64 |

Includes dry-run publish + GitHub Release.

---

## Contributing

### Before Submitting a PR

- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` — zero warnings
- [ ] `cargo doc --workspace --no-deps` — zero missing docs
- [ ] `cargo fmt --all -- --check` — no drift
- [ ] New public APIs have `///` rustdoc
- [ ] `CHANGELOG.md` updated with your change

### Code Style

- **No `unwrap()`/`expect()`** outside tests and CLI main
- **Workspace dependencies** managed in root `Cargo.toml` `[workspace.dependencies]`
- **Fuzz crates excluded** from workspace: `exclude = ["crates/*/fuzz"]`
- **`#[expect(clippy::*)]`** preferred over `#[allow(clippy::*)]` with justification

### PR Process

1. Fork the repository
2. Create a feature branch
3. Make your changes (keep commits atomic)
4. Run the full verification suite
5. Update `CHANGELOG.md`
6. Submit a PR with clear description of what and why

### Reporting Issues

- **Bug reports**: Open a GitHub issue with reproduction steps
- **Security vulnerabilities**: Report via **GitHub Security Advisories** — not public issues (see [SECURITY.md](../SECURITY.md))

---

## Project Conventions

### Crate Dependencies

Dependencies are versioned in `Cargo.toml` at workspace root:

```toml
[workspace.dependencies]
swash = "0.2"
tiny-skia = "0.11"
clap = { version = "4.5", features = ["derive"] }
```

Individual crate `Cargo.toml` files reference them without version:

```toml
[dependencies]
swash.workspace = true
tiny-skia.workspace = true
```

### Feature Gates

- **`native-backend`** (default): enables `subtitle-renderer`
- **`libass-backend`**: enables `libass-sys` + `subtitle-renderer-libass`

### No Makefile

There is no Makefile or task runner. Run all commands directly with `cargo`.

---

## Troubleshooting

### "Font not found" errors

Ensure DejaVu Sans is installed:

```bash
sudo apt install fonts-dejavu-core
```

### "libass.so not found"

Install libass:

```bash
# Debian/Ubuntu
sudo apt install libass9
# macOS
brew install libass
```

For CI, pre-built libass binaries are bundled in `links/`.

### "max_input_size_bytes" rejection

Input files exceeding 100 MiB are rejected to prevent accidental video ingestion. If your ASS file is genuinely that large, split it.

### Test failures on macOS

Some font path tests may fail due to different system font locations. Run with:

```bash
cargo test --workspace -- --skip font_path_tests
```

---

## Continue Reading

- [🏛️ Architecture](architecture.md) — Full pipeline and crate breakdown
- [🎨 Rendering Backends](rendering-backends.md) — Backend comparison
- [📦 PGS Encoder Design](pgs-encoder.md) — DDD architecture
- [🎯 Color Quantizer](color-quantizer.md) — Quantization pipeline
- [🔤 Font System](font-system.md) — Font subsystem internals
