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
