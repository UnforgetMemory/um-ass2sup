# Task 5.1 — Color pipeline (Sprint 4 foundation)

## Status: ✅ COMPLETED (2026-06-17)

## Goal

Introduce a typed colour-pipeline module: `ColorSpace`, `TransferFunction`,
`Tonemap`, and an end-to-end `convert_rgb` helper, plus the auto-detection
logic for HDR source markers.

## Files changed

| File | Change |
|------|--------|
| `crates/color-quantizer/src/color_pipeline.rs` (new) | `ColorSpace` (SdrBt709 / HdrBt2020Pq / HdrBt2020Hlg), `TransferFunction` (Linear / Srgb / Pq / Hlg), `Tonemap` (None / Hable / Reinhard / Aces), `ColorPipelineConfig`, `convert_rgb()`, `detect_source_color_space()` + 18 unit tests |
| `crates/color-quantizer/src/lib.rs` | `pub mod color_pipeline;` + re-exports |
| `crates/color-quantizer/Cargo.toml` | Adds `serde` workspace dep (for derive on enums) |

## Transfer functions

- **sRGB** (default): standard IEC 61966-2-1 gamma 2.4 with linear toe
- **PQ (SMPTE ST 2084)**: HDR10 transfer function
- **HLG (ARIB STD-B67)**: HLG transfer function with the diffuse-white
  reference at 0.5
- **Linear**: identity (no transfer function)

Each `TransferFunction` provides `to_linear()` (encoded → linear) and
`from_linear()` (linear → encoded). Both roundtrip losslessly within
the numerical precision of f64.

## Tonemapping operators

- **None**: clamp to `[0, 1]`
- **Hable**: Uncharted 2 filmic (asymmetric shoulder)
- **Reinhard**: simple `v / (1 + v)`
- **ACES**: Narkowicz fit (industry standard)

## Verification gates

- [x] 18 unit tests in `color_pipeline::tests`
- [x] `cargo test -p color-quantizer` — 18/18 pass
- [x] `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings
- [x] `cargo fmt --check` — clean
- [x] PQ + HLG + sRGB roundtrips within 1e-6
- [x] BT.709 XYZ matrix matches D65 reference
- [x] HDR auto-detection from `Output: HDR` and `YCbCr Matrix: BT.2020` markers

## Migration path

The existing v0.5.x colour path (BT.601/BT.709) is unchanged. New code
can opt into the colour pipeline by calling `convert_rgb()` per frame.
Full integration into the renderer is tracked in
`docs/superpowers/specs/2026-06-17-Sub-5-color-pipeline.md`.
