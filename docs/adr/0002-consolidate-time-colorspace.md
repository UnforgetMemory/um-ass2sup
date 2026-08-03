# ADR 0002: Consolidate cross-crate time & colour-space logic

- **Status**: Accepted
- **Date**: 2026-08-03
- **Scope**: pgs-encoder, ass2sup-cli, subtitle-renderer-libass, ass-core

## Context

The audit (2026-08-03) found duplicated, drifting implementations of the same
domain logic across crates:

1. **`ms_to_90khz` in 4 places** — `ass-core::time::convert::ms_to_90khz`
   (pure `ms * 90`, public API), `pgs-encoder::domain::timing::ms_to_90khz`
   (NTSC-aware, **zero callers**), `PgsEncoder::ms_to_90khz` method (in use),
   free `pgs-encoder::encoding::encoder::ms_to_90khz` (zero callers).
2. **`frame_accurate_pts` byte-identical in 2 files** —
   `ass2sup-cli::pipeline::convert::ConversionPipeline::frame_accurate_pts`
   (private) and `subtitle-renderer-libass::infra::pgs_adapter::frame_accurate_pts`
   (pub). Both eliminate sub-frame NTSC drift.
3. **"height > 576 → BT.709" heuristic in 3 files** —
   `pgs-encoder::domain::palette::color_space_for_height` (pub fn, only used by
   tests), inline `if height > 576` in `pgs_adapter::create_pipeline`, and an
   inline guard in `ass2sup-cli::pipeline::convert`.

## Decision

Establish a single authoritative implementation for each piece of domain logic,
in `pgs-encoder` (the shared downstream of both rendering backends), and have
all consumers delegate to it:

- **`frame_accurate_pts(ms, fps)` → `pgs_encoder::domain::timing`** — the
  authoritative PTS mapping. `ass2sup-cli` and `subtitle-renderer-libass` now
  call it (the libass `pgs_adapter` version remains as a thin delegating
  wrapper to avoid breaking its public API).
- **Delete dead `ms_to_90khz` implementations** — `timing::ms_to_90khz(ms, fps)`
  (zero callers; superseded by frame-accurate conversion, as its own doc
  comments admitted drift) and the free `encoder::ms_to_90khz(ms)`.
  `ass-core::time::ms_to_90khz` is retained: it is a documented public API of
  the parser crate with doctests, and its integer form serves non-NTSC
  consumers.
- **`color_space_for_height(height)` → `pgs_encoder::domain::palette`** — the
  single HD/BT.709 heuristic. `pgs_adapter::create_pipeline` delegates to it;
  `ass2sup-cli` calls it only when the user left `--color-space` at its default
  (preserving the "user override wins" behaviour).

## Consequences

- **Positive**: one definition per concept; both backends produce
  byte-identical PTS values by construction; dead code removed (2 functions,
  unused `png` dependency).
- **Negative**: `pgs_adapter::frame_accurate_pts` is now a 1-line delegation
  (API kept for compatibility); the CLI's private method also delegates (one
  extra function call per frame — negligible vs. rendering cost).
- **Compatibility**: no public signatures removed; `color_space_for_height`
  behaviour unchanged; NTSC PTS output is provably identical (tests cover
  both backends' golden SUP output).

## Verification

- `cargo test --workspace --all-targets` — 767 passed, 0 failed.
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 errors.
- `cargo check --workspace --all-targets` — 0 errors/warnings.
- `cargo bench -p pgs-encoder` — no regression vs. BENCHMARKS.md baseline
  (rle_large 2.40 ms, pgs_encode_medium 98.7 µs).
