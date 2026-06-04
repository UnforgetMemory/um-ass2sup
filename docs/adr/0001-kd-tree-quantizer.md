# ADR 0001: kd-tree acceleration for color quantization

## Status
Accepted (Phase 24, May 2026).

## Context
The naive median-cut color quantizer maps each output pixel to its nearest palette color via a linear scan: O(N·P) where N = pixel count and P = palette size (up to 256). On a 1920x1080 frame (2,073,600 pixels) with a 64-color palette, that is ~133M comparisons per frame, and the conversion runs at ~24 frames/sec. Profiling identified this nearest-color search as the dominant cost (~75% of quantize time).

## Decision
Cache a 2D kd-tree (split on palette cell coordinates) alongside the palette. Look up nearest colors via kd-tree descent rather than linear scan.

## Consequences
- Quantize time: 908ms → 353ms (2.6×) on a 30-frame 1920x1080 stress asset. Validated in `BENCHMARKS.md`.
- The kd-tree is small (one node per palette entry, ~16 bytes each) and built once per quantize call. It lives only on the quantize stack — no global state, no synchronization.
- Output is byte-identical to the linear-scan baseline (asserted in proptest `ass_quantize_deterministic`).
- Code complexity added: ~60 lines in `color-quantizer/src/median_cut.rs` (`KdTree::new`, `KdTree::nearest`). Reviewed and approved in Phase 24.
- The kd-tree is internal to the quantizer — no public API surface change.

## Alternatives considered
- **SIMD-friendly palette indexing (precomputed LUT)**: rejected — the alpha channel and dithering make a flat LUT non-trivial; gain was estimated smaller than kd-tree's.
- **Wasm SIMD intrinsics**: rejected — adds a target-feature gate and compile-time complexity disproportionate to a 2.6× win already achieved.
- **GPU quantization**: rejected — adds a heavy dependency (wgpu) and changes deployment story; not worth the cost for a CLI tool.
