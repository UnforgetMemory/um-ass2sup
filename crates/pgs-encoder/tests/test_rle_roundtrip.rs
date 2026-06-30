//! RLE encode/decode round-trip tests with proptest.

use pgs_encoder::domain::rle::{chunk_rle_data, rle_decode, rle_encode};
use proptest::prelude::*;

/// Swap palette index 0 ↔ pivot (mirrors `domain::palette::swap`).
fn swap(val: u8, pivot: u8) -> u8 {
    if val == 0 {
        pivot
    } else if val == pivot {
        0
    } else {
        val
    }
}

fn swap_slice(data: &[u8], pivot: u8) -> Vec<u8> {
    data.iter().map(|&v| swap(v, pivot)).collect()
}

/// Test RLE encode/decode round-trip.
///
/// When `ti != 0`, the RLE pipeline requires pre-encode swap (0 ↔ ti) so that
/// the transparent index in the encoded byte stream is always 0.  Callers
/// (`prepare_rle_and_hash`) perform this swap before `rle_encode`; the
/// complementary swap happens inside `rle_decode` after decompression.
fn roundtrip(indices: &[u8], w: usize, h: usize, ti: u8) {
    let w32 = w as u32;
    let h32 = h as u32;
    // Pre-swap so transparent index becomes 0 in the RLE stream
    let swapped = if ti != 0 {
        swap_slice(indices, ti)
    } else {
        indices.to_vec()
    };
    let encoded = rle_encode(&swapped, w32, h32, 0);
    assert!(!encoded.is_empty(), "RLE should produce non-empty output");
    let decoded = rle_decode(&encoded, w32, h32, ti)
        .unwrap_or_else(|e| panic!("RLE decode failed for {}x{} ti={ti}: {e}", w, h));
    assert_eq!(
        decoded, indices,
        "RLE round-trip mismatch for {}x{} ti={ti}",
        w, h
    );
}

// ── proptest: random small images ──────────────────────────────────

proptest! {
    #[test]
    fn proptest_roundtrip_small(
        indices in proptest::collection::vec(0u8..=255, 8*8..=16*16),
        ti in 0u8..=255u8,
    ) {
        // Find dimensions that divide exactly: try width=8, then derive height
        let total = indices.len();
        let w = 8usize;
        let h = total / w;
        if total % w != 0 || h == 0 { return Ok(()); }
        if h > 64 { return Ok(()); }
        roundtrip(&indices, w, h, ti);
    }
}

// ── Edge cases ────────────────────────────────────────────────────

#[test]
fn all_transparent_ti0() {
    let w = 16;
    let h = 16;
    let indices = vec![0u8; w * h];
    roundtrip(&indices, w, h, 0);
}

#[test]
fn all_transparent_ti_nonzero() {
    let w = 16;
    let h = 16;
    let ti = 128;
    let indices = vec![ti; w * h];
    roundtrip(&indices, w, h, ti);
}

#[test]
fn all_opaque() {
    let w = 16;
    let h = 16;
    let indices = vec![1u8; w * h];
    roundtrip(&indices, w, h, 0);
}

#[test]
fn single_row() {
    let w = 64;
    let h = 1;
    let indices: Vec<u8> = (0..w as u8).collect();
    roundtrip(&indices, w, h, 0);
}

#[test]
fn single_column() {
    let w = 1;
    let h = 64;
    let indices: Vec<u8> = (0..h as u8).collect();
    roundtrip(&indices, w, h, 0);
}

#[test]
fn single_pixel_opaque() {
    roundtrip(&[42], 1, 1, 0);
}

#[test]
fn single_pixel_transparent() {
    roundtrip(&[0], 1, 1, 0);
}

#[test]
fn single_pixel_transparent_ti_nonzero() {
    roundtrip(&[128], 1, 1, 128);
}

#[test]
fn alternating_pattern() {
    let w = 8;
    let h = 8;
    let indices: Vec<u8> = (0..(w * h))
        .map(|i| if i % 2 == 0 { 0 } else { 255 })
        .collect();
    roundtrip(&indices, w, h, 0);
}

#[test]
fn alternating_pattern_ti_nonzero() {
    let w = 8;
    let h = 8;
    let ti = 128;
    let indices: Vec<u8> = (0..(w * h))
        .map(|i| if i % 2 == 0 { ti } else { 1 })
        .collect();
    roundtrip(&indices, w, h, ti);
}

#[test]
fn checkerboard_2x2() {
    // 2x2 repeating: 0, 1, 1, 0
    let w = 4;
    let h = 4;
    let indices: Vec<u8> = (0..(w * h))
        .map(|i| {
            let x = i % w;
            let y = i / w;
            if (x / 2 + y / 2) % 2 == 0 {
                0
            } else {
                255
            }
        })
        .collect();
    roundtrip(&indices, w, h, 0);
}

#[test]
fn color_0x40_to_0xbf() {
    // Colors in the range 0x40..=0xBF must use 3-byte long-run format
    let w = 4;
    let h = 4;
    let indices: Vec<u8> = (0x40u8..=0x4Fu8).cycle().take(w * h).collect();
    roundtrip(&indices, w, h, 0);
}

#[test]
fn transparent_index_swap_roundtrip() {
    // transparent_index != 0: color 0 ↔ transparent_index swap before encode
    let w = 8;
    let h = 8;
    let ti = 7;
    let indices: Vec<u8> = (0..(w * h))
        .map(|i| match i % 5 {
            0 => 0,   // transparent (becomes ti after encode swap)
            1 => ti,  // opaque (becomes 0 after encode swap)
            2 => 128, // opaque
            3 => 64,  // opaque
            _ => 255, // opaque
        })
        .collect();
    roundtrip(&indices, w, h, ti);
}

#[test]
fn chunk_rle_correctness() {
    // Verify chunk_rle_data splits correctly and all chunks are valid
    let w = 64;
    let h = 64;
    let indices: Vec<u8> = (0..(w * h)).map(|i| (i % 256) as u8).collect();
    let rle = rle_encode(&indices, w, h, 0);
    let chunks = chunk_rle_data(&rle, 100);
    assert!(chunks.len() > 1, "Should produce multiple chunks");

    // Reassembled chunks should equal original RLE
    let mut reassembled = Vec::new();
    for data in &chunks {
        reassembled.extend_from_slice(data);
    }
    assert_eq!(
        reassembled, rle,
        "Reassembled chunks should equal original RLE"
    );
}

#[test]
fn zero_sized_frame() {
    let rle = rle_encode(&[], 0, 0, 0);
    let decoded = rle_decode(&rle, 0, 0, 0).unwrap();
    assert!(decoded.is_empty());
}
