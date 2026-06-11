# Fix test_ocr_e2e: transparent_index=255 swap issue

## Problem

`test_ocr_e2e::test_ocr_roundtrip` fails with `RleDecodeFailed("invalid run length 2048")` when `transparent_index=255`.

The encoder's `swap()` function swaps colors 0 and 255 before RLE encoding, then uses `enc_transparent=0`. The decoder receives the SUP with:
- RLE data using swapped indices (0=transparent, 255=opaque after swap)
- PDS palette using original indices (0=transparent, 255=opaque)

The palette reconstruction in `decode_to_image.rs` swaps entries 0 and transparent_index, but the RLE decode still produces incorrect results.

## Root Cause Analysis

The `swap()` function in `rle.rs` changes which colors are treated as transparent vs opaque:
- Original transparent (index 0) → becomes 255 → treated as opaque
- Original opaque (index 255) → becomes 0 → treated as transparent

The encoder then encodes with `enc_transparent=0`, meaning:
- Color 0 (was opaque) → transparent format
- Color 255 (was transparent) → opaque format

The decoder's `rle_decode` always uses transparent_index=0 (hardcoded), which matches the encoder's `enc_transparent=0`. So RLE decoding should work.

But the palette in the SUP file uses original indices. The palette reconstruction swaps entries 0 and 255 to match the RLE's index space. This should also work.

The actual issue may be that the RLE data contains runs that exceed the frame dimensions after the swap changes which colors are transparent vs opaque.

## Files to Modify

- `crates/pgs-encoder/src/rle.rs` — `swap()` function, `encode_run()`
- `crates/pgs-encoder/src/decode_to_image.rs` — palette reconstruction logic
- `crates/ass2sup-cli/tests/test_ocr_e2e.rs` — test expectations

## Approach

1. Add debug logging to trace the exact RLE bytes produced with transparent_index=255
2. Compare with a working case (transparent_index=0) to identify the difference
3. Verify the palette reconstruction is correct
4. Ensure the RLE decoder handles all edge cases with the swapped palette

## Related Findings from Code Review

- **C1 (Fixed)**: Palette double-swap in multi-window compositing — moved swap before object loop
- **HIGH-1 (Fixed)**: rle_decode output overshoots total_pixels — added bounds checking
- **M4**: No tests for transparent_index != 0 through full pipeline — need to add integration test
- **M5**: No test for collision-range colors (0x40..0x7F) — need to add test

## Status

- [x] Root cause partially identified (palette swap logic)
- [x] C1 fixed (palette double-swap moved before loop)
- [x] HIGH-1 fixed (bounds checking in rle_decode)
- [ ] Full fix for transparent_index=255 pipeline
- [ ] test_ocr_e2e passes
- [ ] No regressions in other tests
