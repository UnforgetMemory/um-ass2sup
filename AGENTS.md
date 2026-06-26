# AGENTS.md

## Project

ASS/SSA/SRT → Blu-ray SUP/PGS subtitle converter. Rust workspace, 7 crates.

## Workspace layout

```
crates/
  ass-parser/          # ASS/SSA/SRT parser → strong AST
  subtitle-validator/  # Syntax/overlap checks (depends on ass-parser)
  subtitle-renderer/   # RGBA bitmap rendering (fontdb + rustybuzz + tiny-skia)
  color-quantizer/     # RGBA → indexed color (k-d tree accelerated)
  pgs-encoder/         # Indexed frames → PGS/SUP binary segments
  bdn-xml/             # Blu-ray mastering XML + PNG output
  ass2sup-cli/         # CLI binary (clap), wires everything together
```

## System dependency

Linux requires `libfontconfig1-dev` and `fonts-dejavu-core` for tests:

```bash
sudo apt-get install -y libfontconfig1-dev fonts-dejavu-core
```

macOS/Windows: no extra setup needed.

## Build & verify commands

```bash
# Full verification (CI order)
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --all-targets
cargo test --workspace --doc
cargo doc --workspace --no-deps
```

There is no Makefile or task runner. Run commands directly.

## Single crate work

```bash
cargo test -p ass-parser
cargo test -p pgs-encoder -- test_rle   # single test by name
cargo clippy -p color-quantizer --all-targets -- -D warnings
```

## Quality gates

- **MSRV**: Rust 1.85 (enforced in CI, `Cargo.toml` `rust-version`)
- **clippy**: `-D warnings` (zero warnings enforced)
- **fmt**: `cargo fmt --all -- --check` (no drift allowed)
- **doc**: `#![warn(missing_docs)]` at crate level; public items must have `///` rustdoc
- **cargo-deny**: `deny.toml` enforces license whitelist, no unknown registries/git sources
- **Known ignored advisory**: `RUSTSEC-2025-0119` (transitive via `indicatif 0.17`, ignore in audit)

## Testing

- 350+ unit/integration tests across workspace
- **proptest** in: ass-parser, color-quantizer, pgs-encoder
- **insta snapshots** in: `crates/ass2sup-cli/tests/snapshots/` (update with `cargo insta review`)
- **fuzz targets**: `crates/ass-parser/fuzz/` (3 targets), `crates/color-quantizer/fuzz/` (1), `crates/pgs-encoder/fuzz/` (1)
- **Examples**: `cargo run --example parse_ass -p ass-parser` (and similar for color-quantizer, pgs-encoder)

## CI workflows

- `ci.yml`: fmt → clippy → test → MSRV check (on push/PR to master)
- `audit.yml`: cargo-audit + cargo-deny (weekly + push/PR)
- `release.yml`: cross-platform build matrix (Linux x86_64/aarch64, macOS ARM, Windows) on tag push

## Style conventions

- Dual license: Apache-2.0
- Workspace dependencies managed in root `Cargo.toml` `[workspace.dependencies]`
- Fuzz crates excluded from workspace: `exclude = ["crates/*/fuzz"]`
- Release profile: `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`
