# AGENTS.md

## Project

ASS/SSA/SRT → Blu-ray SUP/PGS subtitle converter. Rust workspace, 7 crates.
**Zero external font/shaper dependencies** — self-built `FontRegistry` + `SimpleShaper` + `GlyphRasterizer` on swash.

## Workspace layout

```
crates/
  ass-core/            # ASS/SSA/SRT parser → strong AST (hand-written, 0 external deps)
  subtitle-validator/  # Syntax/overlap checks (depends on ass-core)
  subtitle-renderer/   # RGBA bitmap rendering — FontRegistry + swash + tiny-skia
  color-quantizer/     # RGBA → indexed color (k-d tree accelerated, Floyd-Steinberg dither)
  pgs-encoder/         # Indexed frames → PGS/SUP binary segments
  bdn-xml/             # Blu-ray mastering XML + PNG output
  ass2sup-cli/         # CLI binary (clap), wires everything together
```

## Rendering stack (NO fontdb / NO cosmic-text / NO rustybuzz)

```
Trace: ass-core parse → RenderContext (build_context) → shape_horizontal/vertical (SimpleShaper/swash)
  → glyph rasterization (GlyphRasterizer/swash) → composite_glyph → effects (blur/shadow/outline)
  → transform_layer (AffineTransform for scale/rotate/shear/perspective) → composite_subregion
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
# Generate (not check) release binary
cargo build --release
```

There is no Makefile or task runner. Run commands directly.

## Single crate work

```bash
cargo test -p ass-core
cargo test -p pgs-encoder -- test_rle   # single test by name
cargo clippy -p color-quantizer --all-targets -- -D warnings
cargo run --release -p ass2sup-cli -- input.ass -o output.sup
```

## Quality gates

- **MSRV**: Rust 1.85 (enforced in CI, `Cargo.toml` `rust-version`)
- **Edition**: 2021
- **clippy**: `-D warnings` (zero warnings enforced)
- **fmt**: `cargo fmt --all -- --check` (no drift allowed)
- **doc**: `#![warn(missing_docs)]` at crate level; public items must have `///` rustdoc
- **Profile**: `opt-level = 3`, `lto = "thin"`, `codegen-units = 1`
- **cargo-deny**: `deny.toml` enforces license whitelist, no unknown registries/git sources
- **Known ignored advisory**: `RUSTSEC-2025-0119` (transitive via `indicatif 0.17`, ignore in audit)

## Testing

- 350+ unit/integration tests across workspace
- **proptest** in: ass-core, color-quantizer, pgs-encoder
- **insta snapshots** in: `crates/ass2sup-cli/tests/snapshots/` (update with `cargo insta review`)
- **fuzz targets**: `crates/ass-core/fuzz/` (3 targets), `crates/color-quantizer/fuzz/` (1), `crates/pgs-encoder/fuzz/` (1)
- **Examples**: `cargo run --release --example parse_ass -p ass-core` (and similar for color-quantizer, pgs-encoder)

## CI workflows

- `ci.yml`: fmt → clippy → test → MSRV check (on push/PR to master)
- `audit.yml`: cargo-audit + cargo-deny (weekly + push/PR)
- `release.yml`: cross-platform build matrix (Linux x86_64/aarch64, macOS ARM, Windows) on tag push

## Style conventions

- Dual license: Apache-2.0
- Workspace dependencies managed in root `Cargo.toml` `[workspace.dependencies]`
- Fuzz crates excluded from workspace: `exclude = ["crates/*/fuzz"]`
- No `unwrap()`/`expect()` outside tests and CLI main
- `#[expect(clippy::*)]` over `#[allow(clippy::*)]` with justification

## Font subsystem (v3, swash-native)

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

Cross-platform font fallback: 8-level chain (exact match → suffix-strip → alias → hardcoded CJK → cross-platform CJK scan → generic → SansSerif → any).

## Surgical fix protocol

Every non-trivial fix MUST follow:

```
1. FULL-CHAIN INVESTIGATION
   - Trace the exact code path from ASS parse → RenderContext → shape → rasterize → composite
   - Identify ROOT CAUSE with file:line evidence — never treat symptoms
   - Verify with pixel-level ground truth (reference SUP comparison) where applicable

2. PLAN THEN CUT
   - Define the surgical boundary: what changes, what must NOT change
   - Single root cause per operation (multiple independent bugs = parallel ops)
   - Zero collateral damage — fix ONLY the broken path

3. VERIFY
   - cargo fmt + clippy + test (full workspace)
   - Generate .output/ artifacts from .localref/ for end-to-end verification
```

## Post-fix verification artifacts

After every completed fix:

```bash
# Generate SUP from .localref/ ASS files to .output/
timestamp=$(date +%Y%m%d-%H%M%S)
for ass in .localref/*.ass; do
  base=$(basename "$ass" .ass)
  cargo run --release -p ass2sup-cli -- "$ass" -o ".output/${base}-${timestamp}.sup"
  # BDN XML + PNG sequence for pixel-level inspection
  cargo run --release -p ass2sup-cli -- "$ass" --to-bdn -d ".output/${base}-${timestamp}/"
done
```

Only run when `.localref/` contains `.ass` files and after a fix that affects rendering output.
Output naming: `{original-name}-{YYYYMMDD-HHMMSS}.sup` + `{original-name}-{YYYYMMDD-HHMMSS}/` (BDN XML + PNG seq).
Run in foreground (not background) — completion reminder will deliver the result.

## Performance constraints

- **No heap allocation in hot render paths** (glyph loop, composite, transform)
- **PixmapPool**: reuse Pixmap buffers via pool_get/pool_put (8 cached entries)
- **AffineTransform**: SIMD (wide::f32x4) bilinear interpolation in `apply_to_pixmap`
- **composite_over**: SIMD (wide::u32x4) Porter-Duff over for 4-pixel chunks
- **Parallel rendering**: rayon-based `par_iter()` in `build_display_set` — each worker holds 1 frame at a time (~8.3 MB at 1080p), no intermediate `Vec<RenderedFrame>`
- **Small palette dedup**: `HashSet<u32>` in quantizer, O(n²) → O(n)
- **k-d tree quantizer**: `find_nearest_index` for palette mapping acceleration (2.57×)

## Memory model

- Renderer owns: `PixmapPool` (8 cached Pixmaps), `FontRegistryRenderResources` (registry + pool + font_map)
- `build_context` produces one `RenderContext` per event per timestamp
- `render_event_font_registry` allocates one `layer: Pixmap` per event (pool_get → fill → composite → pool_put)
- `transform_layer` allocates output buffer (the transform is approx 1:1 or smaller)
- Peak memory: `max_events_per_timestamp × layer_size + output_buffer`, typically < 50 MB at 1080p