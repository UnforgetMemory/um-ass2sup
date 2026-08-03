# ADR 0001: Migrate workspace to Rust 2024 edition

- **Status**: Accepted
- **Date**: 2026-08-03
- **Scope**: Main workspace (8 crates) + `ass2sup-libass/` independent workspace

## Context

The workspace pinned `edition = "2021"` with MSRV 1.89. The user requested adopting
new language features and idioms as part of a comprehensive architecture/quality pass.
Rust 2024 (stable since 1.85) is fully supported by the MSRV toolchain (1.96 in use).

## Decision

Migrate all crates to `edition = "2024"`:

- Root `Cargo.toml` `[workspace.package] edition = "2024"` (inherited by crates using
  `edition.workspace = true`)
- `crates/color-quantizer` and `crates/subtitle-renderer` (independently declared) set
  directly
- `ass2sup-libass/` workspace (own `Cargo.toml` + 3 sub-crates) migrated separately

### Required mechanical changes

1. **Match ergonomics (2024 rule change)**: matching on a reference type with a
   non-reference pattern now implicitly borrows; explicit `ref` modifiers are rejected.
   Fixed: `crates/pgs-encoder/src/domain/segment.rs` — 8× `Variant(ref x)` → `Variant(x)`.
2. **`unsafe extern` blocks (2024 requirement)**: `extern "C" { ... }` blocks must be
   `unsafe extern "C"`. Fixed in `ass2sup-libass/crates/libass-sys/src/lib.rs`.
3. **rustfmt import ordering** (2024 style): `crate::` group ordering changed — applied
   `cargo fmt --all`.
4. **New clippy lints surfaced**: `collapsible_if` (edition-sensitive) auto-fixed by
   `cargo clippy --fix` across `ass2sup-cli`, `ass-core`, `subtitle-renderer-libass`.

### Incidental fixes (pre-existing breakage in `ass2sup-libass`)

`ass2sup-libass` did not compile before migration (3 errors at HEAD). Fixed alongside:
- `main.rs:89`: `ConversionConfig.fonts_dir` → `fonts_dirs` (API drift vs main workspace)
- `fonts_dirs: Option<String>` → `Vec<String>` collection

## Consequences

- **Positive**: 2024 idioms available (let-chains, `unsafe extern`, refined match
  ergonomics); uniform toolchain expectations across both workspaces.
- **Negative**: semantic differences from 2021 (match ergonomics, borrow checker
  refinements in 2024) — mitigated by full test suite (700+ tests) running green.
- **Compatibility**: `rust-version = "1.89"` retained; 2024 edition requires 1.85+,
  consistent with MSRV.
- `ass2sup-libass` README mentions "2021 edition" — doc update tracked in cleanup.

## Verification

```
cargo check --workspace --all-targets     # 0 errors
cargo fmt --all -- --check                # clean
cargo clippy --workspace --all-targets -- -D warnings   # 0 errors
cargo test --workspace --all-targets      # all green
cargo test --workspace --doc              # all green
cargo bench --workspace --no-run          # compiles
```
