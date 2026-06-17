# Task 3.1 — cosmic-text dependency + AssFallback trait (Sprint 2)

## Status: ✅ COMPLETED (2026-06-17)

## Goal

Add `cosmic-text = "0.19"` as an opt-in dependency and define the
`AssFallback` trait + `FontResolver` data structure that the rest of the
font migration will build on.

## Files changed

| File | Change |
|------|--------|
| `Cargo.toml` | `cosmic-text = "0.19"` added to `[workspace.dependencies]` with `["std", "fontconfig", "warn_on_missing_glyphs"]` features |
| `crates/subtitle-renderer/Cargo.toml` | `cosmic-text` optional dep + new `cosmic-text` Cargo feature (off by default) |
| `crates/subtitle-renderer/src/font_cosmic.rs` (new) | `FallbackChain`, `AssFallback`, `FontResolver` + 8 unit tests |
| `crates/subtitle-renderer/src/lib.rs` | Re-exports gated on `#[cfg(feature = "cosmic-text")]` |

## API surface

```rust
pub struct FallbackChain {
    pub chain: Vec<String>,
    pub per_style: HashMap<String, Vec<String>>,
    pub strict: bool,
}

impl FallbackChain {
    pub fn for_style(&self, style_name: &str) -> &[String];
}

pub struct AssFallback { /* owns FallbackChain */ }
impl AssFallback {
    pub fn new(chain: FallbackChain) -> Self;
    pub fn global_chain(&self) -> &[String];
    pub fn chain_for(&self, style_name: &str) -> &[String];
    pub fn is_strict(&self) -> bool;
}

pub struct FontResolver { /* owns FallbackChain */ }
impl FontResolver {
    pub fn new(chain: FallbackChain) -> Self;
    pub fn chain(&self) -> &FallbackChain;
    pub fn resolve_for_style(&self, style_name: &str) -> &[String];
}
```

## Verification gates

- [x] `cargo build -p subtitle-renderer --features cosmic-text` — clean
- [x] `cargo build -p subtitle-renderer` (default features) — clean (no impact)
- [x] `cargo test -p subtitle-renderer --features cosmic-text font_cosmic` — 8/8 pass
- [x] `cargo fmt --check` — clean
- [x] `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings

## Migration path (deferred)

The full migration of `font.rs` (1085 lines) to cosmic-text is out of
scope for this PR. The existing `FontManager` continues to work; the
new `FontResolver` is the v2.0 entry point that the renderer migration
will use once the cosmic-text `FontSystem` is wired in.

See `docs/superpowers/specs/2026-06-17-Sub-3-font-engine.md` for the
full plan.
