# Baseline Benchmark Results

Hardware: Linux WSL2 | Rust: 1.77 | Date: 2026-06-02

## pgs-encoder (6 benchmarks)

| Benchmark                      | Median    | Std Dev   |
| ------------------------------ | --------- | --------- |
| rle_small_64x32                | 2.8448 µs | ±57.8 ns  |
| rle_medium_320x180             | 73.625 µs | ±1.4 µs   |
| rle_large_1920x1080            | 2.4450 ms | ±47.6 µs  |
| pgs_encode_small_64x32         | 4.2357 µs | ±46.1 ns  |
| pgs_encode_medium_320x180      | 90.343 µs | ±1.5 µs   |
| pgs_encode_ntsc_320x180        | 91.138 µs | ±1.4 µs   |

## color-quantizer (5 benchmarks)

| Benchmark                           | Median     | Std Dev    |
| ----------------------------------- | ---------- | ---------- |
| quantizer_small_64x32               | 112.73 µs  | ±3.9 µs    |
| quantizer_medium_320x180            | 13.147 ms  | ±395 µs    |
| quantizer_large_1920x1080           | 907.88 ms  | ±9.3 ms    |
| quantizer_no_dither_320x180         | 11.408 ms  | ±127 µs    |
| quantizer_ordered_dither_320x180    | 11.678 ms  | ±45.8 µs   |

## subtitle-renderer (24 benchmarks)

See `/tmp/bench_renderer_baseline.txt` for full results.

## Notes

- `quantizer_large_1920x1080` takes ~908ms — potential optimization target
- `rle_large_1920x1080` at 2.4ms is acceptable for per-frame encoding
- Encoder benchmarks include end-to-end PGS display set construction

---

# Phase 24 Optimizations (2026-06-04)

After the Phase 24 audit identified `find_nearest_index` as the dominant
hot loop in color quantization, two commits landed:

| Commit  | Type   | Change                                                 | Effect                                 |
| ------- | ------ | ------------------------------------------------------ | -------------------------------------- |
| 7140ea4 | perf   | HashSet dedup in small-palette path (`O(n²)→O(n)`)     | Helps when `opaque_pixels.len() ≤ max_colors` |
| 8f981d5 | perf   | k-d tree accelerator for `find_nearest_index`          | Dominant path at 1080p: 908ms → 353ms  |

## Measured impact — 1080p × 255 colors

| Metric                    | Before (linear) | After (k-d tree) | Speedup |
| ------------------------- | --------------- | ---------------- | ------- |
| `quantize` wall time      | 907.88 ms       | **353 ms**       | **2.57x** |
| Per-pixel cost            | ~440 ns         | ~170 ns          | **2.59x** |
| `find_nearest_index` calls per frame | 2,073,600 | 2,073,600 | (unchanged) |
| Avg comparisons per call  | 128 (mid-palette) | ~log₂(255) ≈ 8  | **~16x fewer** |
| Parity hash (`quantize()` output) | `8847b5d7b81ba7fa` | `8847b5d7b81ba7fa` | **identical** |

## Implementation notes

### P1 — HashSet dedup
- File: `crates/color-quantizer/src/lib.rs:128-138`
- Replaced `Vec::contains` (O(n) per check) with `HashSet::insert`
  (O(1) per check) for the small-palette dedup branch.
- First-occurrence order preserved (Vec push, HashSet only for "seen" check).
- 3 regression tests added: `dedup_preserves_first_occurrence_order`,
  `dedup_handles_all_same_color`, `dedup_handles_empty`.
- Parity bench `parity_bench` hash `5ace83442d49fa2` confirmed identical
  pre/post.

### P2 — k-d tree
- File: `crates/color-quantizer/src/median_cut.rs:136-247`
- In-tree k-d tree, no external crate. `KdNode` enum:
  `Leaf(Vec<usize>)` for small batches (≤8 indices), `Split { axis,
  threshold, left, right }` otherwise.
- Build: O(n log n) via sort + `partition_point` on longest axis.
- Query: branch-and-bound with plane-distance pruning.
- Linear fallback for `palette.len() < 32` guarantees exact tie-breaking
  parity with the original `min_by_key` (first-minimum preference).
- 2 parity tests added: `kdtree_parity_against_linear` (5 cases incl.
  255-color max palette + 1024 random queries), `kdtree_e2e_parity_hash`
  (end-to-end `quantize()` output hash).
- E2E parity hash confirmed identical pre/post: `8847b5d7b81ba7fa`.

## Verification commands

```bash
# Reproduce parity check
cargo test -p color-quantizer --lib kdtree_parity_against_linear
cargo test -p color-quantizer --lib kdtree_e2e_parity_hash -- --nocapture

# Reproduce perf number (requires release build)
cargo bench -p color-quantizer quantizer_large_1920x1080
```

## Targets met

- 1080p ≤ 500ms target: **PASS** (353ms)
- Byte-for-byte parity with linear baseline: **PASS** (hash unchanged)
- No external dependencies added: **PASS** (in-tree k-d tree)
- No `unsafe` code: **PASS**
