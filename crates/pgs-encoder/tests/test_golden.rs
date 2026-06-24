use color_quantizer::Rgba;
use pgs_encoder::color::{build_palette, color_space_for_height};
use pgs_encoder::rle::{rle_decode, rle_encode};
use pgs_encoder::{PgsEncoder, SegmentType};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn hash_bytes(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

#[test]
fn test_rle_golden_checkerboard() {
    let indices = vec![1u8, 0, 0, 1, 1, 0, 0, 1];
    let encoded = rle_encode(&indices, 4, 2, 0);
    let hash = hash_bytes(&encoded);
    assert_eq!(hash, 13418966361860685585, "RLE golden hash mismatch");
    let decoded = rle_decode(&encoded, 4, 2, 0).unwrap();
    assert_eq!(decoded, indices);
}

#[test]
fn test_palette_golden_bt709() {
    let palette = vec![
        Rgba::new(0, 0, 0, 0),
        Rgba::new(255, 0, 0, 255),
        Rgba::new(0, 255, 0, 255),
        Rgba::new(0, 0, 255, 255),
    ];
    let entries = build_palette(&palette, color_space_for_height(1080));
    assert_eq!(entries.len(), 4);
    assert_eq!(entries[0].alpha, 0);
    assert_eq!(entries[1].y, 54);
    assert_eq!(entries[1].cb, 99);
    assert_eq!(entries[1].cr, 255);
}

#[test]
fn test_encode_golden_small_frame() {
    let palette = vec![Rgba::new(0, 0, 0, 0), Rgba::new(255, 255, 255, 255)];
    let indices = vec![1u8; 16];
    let frame = color_quantizer::QuantizedFrame {
        width: 4,
        height: 4,
        palette,
        indices,
        transparent_index: 0,
        x: 0,
        y: 0,
        color_space: Default::default(),
    };

    let mut encoder = PgsEncoder::new(1920, 1080, 24.0);
    let segments = encoder.encode_frame(&frame, 0, 1000);

    assert!(segments.len() >= 5);
    assert_eq!(segments[0].segment_type, SegmentType::Pcs);
    assert_eq!(segments[1].segment_type, SegmentType::Wds);
    assert_eq!(segments[2].segment_type, SegmentType::Pds);
    assert!(matches!(segments[3].segment_type, SegmentType::Ods));

    let last = segments.len() - 1;
    assert_eq!(segments[last].segment_type, SegmentType::End);

    let mut totals = [0u64; 8];
    for seg in &segments {
        let bytes = seg.to_bytes();
        totals[0] += bytes.len() as u64;
    }
    assert!(totals[0] > 0, "Total encoded bytes must be > 0");
}
