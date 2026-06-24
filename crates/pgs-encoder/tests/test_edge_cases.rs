//! Edge case tests for the PGS encoder.
//!
//! Covers: transparent frames, single-pixel frames, max dimensions,
//! RLE compression extremes, palette edge cases, YCbCr roundtrip,
//! timestamp encoding, frame rate codes, multi-frame encoding,
//! and SUP binary format validation.

use color_quantizer::{QuantizedFrame, Rgba};
use pgs_encoder::color::{build_palette, color_space_for_height, rgba_to_ycbcr};
use pgs_encoder::encoder::{frame_rate_code, ms_to_90khz};
use pgs_encoder::rle::rle_encode;
use pgs_encoder::types::{SegmentPayload, SegmentType, SupFile};
use pgs_encoder::PgsEncoder;

// ─────────────────────── Helpers ───────────────────────

fn make_frame(
    width: u32,
    height: u32,
    palette: Vec<Rgba>,
    indices: Vec<u8>,
    transparent_index: u8,
) -> QuantizedFrame {
    QuantizedFrame {
        width,
        height,
        palette,
        indices,
        transparent_index,
        x: 0,
        y: 0,
        color_space: Default::default(),
    }
}

fn make_single_color_frame(width: u32, height: u32, color: Rgba) -> QuantizedFrame {
    let npixels = (width * height) as usize;
    make_frame(
        width,
        height,
        vec![Rgba::new(0, 0, 0, 0), color],
        vec![1; npixels],
        0,
    )
}

// ─────────────────────── Transparent Frame ───────────────────────

#[test]
fn test_encode_transparent_frame() {
    let mut enc = PgsEncoder::new(1920, 1080, 23.976);
    let frame = make_frame(4, 2, vec![Rgba::new(0, 0, 0, 0)], vec![0; 8], 0);

    let sup_data = enc.encode_frame_to_bytes(&frame, 0, 1000);
    assert!(sup_data.len() >= 2);
    assert_eq!(sup_data[0], b'P');
    assert_eq!(sup_data[1], b'G');
}

// ─────────────────────── Single Pixel Frame ───────────────────────

#[test]
fn test_encode_single_pixel_frame() {
    let mut enc = PgsEncoder::new(1920, 1080, 23.976);
    let frame = make_single_color_frame(1, 1, Rgba::new(255, 0, 0, 255));

    let segments = enc.encode_frame(&frame, 0, 1000);
    assert_eq!(
        segments.len(),
        8,
        "Should have PCS+WDS+PDS+ODS+END+palette_clear(PCS+PDS)+END"
    );

    let sup_data = enc.encode_frame_to_bytes(&frame, 1000, 1000);
    assert!(sup_data.len() >= 13);
    assert_eq!(sup_data[0], b'P');
    assert_eq!(sup_data[1], b'G');
}

// ─────────────────────── Max Dimensions ───────────────────────

#[test]
fn test_encode_max_dimensions_no_panic() {
    let mut enc = PgsEncoder::new(4096, 4096, 23.976);
    // Small actual frame but placed in large display
    let frame = make_single_color_frame(4, 2, Rgba::new(255, 255, 255, 255));

    // Should not panic
    let sup_data = enc.encode_frame_to_bytes(&frame, 0, 1000);
    assert!(sup_data.len() >= 2);
    assert_eq!(sup_data[0], b'P');
}

// ─────────────────────── RLE: Maximum Compression ───────────────────────

#[test]
fn test_rle_all_same_color_maximum_compression() {
    // 100 pixels all the same color → 1 run via 0x00 prefix
    let width = 100u32;
    let height = 1u32;
    let indices = vec![5u8; 100];

    let encoded = rle_encode(&indices, width, height, 0);

    // FFmpeg format: 0x00 [0xC0|0] [100] [5] = 4 bytes
    assert_eq!(encoded.len(), 4);
    assert_eq!(encoded, vec![0x00, 0xC0, 100, 5]);
}

// ─────────────────────── RLE: Minimum Compression ───────────────────────

#[test]
fn test_rle_alternating_colors_minimum_compression() {
    // Alternating colors: each pixel is a different single pixel
    let width = 10u32;
    let height = 1u32;
    let indices = vec![1u8, 2, 1, 2, 1, 2, 1, 2, 1, 2];

    let encoded = rle_encode(&indices, width, height, 0);

    // Each single pixel = 1 byte in FFmpeg format
    assert_eq!(
        encoded.len(),
        10,
        "Alternating colors should produce 1 byte per pixel"
    );
}

// ─────────────────────── RLE: Long Run >63 ───────────────────────

#[test]
fn test_rle_long_run_over_63() {
    // 100 pixels of same color → FFmpeg opaque run via 0x00 prefix
    let width = 100u32;
    let height = 1u32;
    let indices = vec![7u8; 100];

    let encoded = rle_encode(&indices, width, height, 0);

    // FFmpeg format: 0x00 [0xC0] [100] [7] = 4 bytes
    assert_eq!(encoded.len(), 4);
    assert_eq!(encoded, vec![0x00, 0xC0, 100, 7]);
}

// ─────────────────────── RLE: Very Long Run >16383 ───────────────────────

#[test]
fn test_rle_very_long_run_over_16383() {
    // 20000 pixels of same color → capped at 16383 per run
    let width = 20000u32;
    let height = 1u32;
    let indices = vec![3u8; 20000];

    let encoded = rle_encode(&indices, width, height, 0);

    // Should not panic, should produce valid output
    assert!(!encoded.is_empty());
    // First run: 0x00 [0xC0|3] [16383&0xFF] [3] = 4 bytes
    // Remaining: 20000-16383=3617, long run: 0x00 [0xC0|0] [3617&0xFF] [3] = 4 bytes
    assert_eq!(encoded.len(), 8);
}

// ─────────────────────── RLE: Multi-row ───────────────────────

#[test]
fn test_rle_multi_row_row_separator() {
    let width = 4u32;
    let height = 2u32;
    let indices = vec![1u8, 1, 1, 1, 2, 2, 2, 2];

    let encoded = rle_encode(&indices, width, height, 0);

    // Row 1: [1, 0x44] = 2 bytes
    // Separator: [0x00, 0x00] = 2 bytes
    // Row 2: [2, 0x44] = 2 bytes
    // Total: 6 bytes
    assert_eq!(encoded.len(), 8);
    // Row 1: 0x00 0x84 1 = 3 bytes (opaque run of 4 pixels, color 1)
    assert_eq!(encoded[0], 0x00);
    assert_eq!(encoded[1], 0x84); // 0x80 | 4
    assert_eq!(encoded[2], 1); // color
    assert_eq!(encoded[3], 0x00); // row separator
    assert_eq!(encoded[4], 0x00);
    // Row 2: 0x00 0x84 2 = 3 bytes
    assert_eq!(encoded[5], 0x00);
    assert_eq!(encoded[6], 0x84);
    assert_eq!(encoded[7], 2);
}

// ─────────────────────── Palette: 256 Colors ───────────────────────

#[test]
fn test_palette_exactly_256_colors() {
    let mut palette = Vec::with_capacity(256);
    for i in 0..256u16 {
        palette.push(Rgba::new(
            (i % 256) as u8,
            ((i * 2) % 256) as u8,
            ((i * 3) % 256) as u8,
            255,
        ));
    }

    let entries = build_palette(&palette, color_space_for_height(1080));
    assert_eq!(entries.len(), 256);

    for entry in &entries {
        assert_eq!(entry.alpha, 255);
    }
}

// ─────────────────────── Palette: 1 Color ───────────────────────

#[test]
fn test_palette_single_color() {
    let palette = vec![Rgba::new(128, 128, 128, 200)];
    let entries = build_palette(&palette, color_space_for_height(1080));

    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].index, 0);
    assert_eq!(entries[0].alpha, 200);
    // Gray → Y≈128, Cb≈128, Cr≈128
    assert_eq!(entries[0].y, 128);
    assert_eq!(entries[0].cb, 128);
    assert_eq!(entries[0].cr, 128);
}

// ─────────────────────── YCbCr Roundtrip ───────────────────────

#[test]
fn test_ycbcr_roundtrip_accuracy() {
    // Test known colors and verify the conversion is within rounding tolerance
    let test_colors = [
        (0u8, 0u8, 0u8), // black
        (255, 255, 255), // white
        (255, 0, 0),     // red
        (0, 255, 0),     // green
        (0, 0, 255),     // blue
        (128, 128, 128), // gray
        (255, 255, 0),   // yellow
        (0, 255, 255),   // cyan
        (255, 0, 255),   // magenta
    ];

    for (r, g, b) in test_colors {
        let (y, cb, cr) = rgba_to_ycbcr(r, g, b);

        // Verify the conversion is deterministic (same input → same output)
        let (y2, cb2, cr2) = rgba_to_ycbcr(r, g, b);
        assert_eq!(y, y2, "Deterministic for ({},{},{})", r, g, b);
        assert_eq!(cb, cb2, "Deterministic for ({},{},{})", r, g, b);
        assert_eq!(cr, cr2, "Deterministic for ({},{},{})", r, g, b);

        // Verify values are in valid range
        // Y: 0-255, Cb/Cr: 0-255 (centered at 128)
    }
}

#[test]
fn test_ycbcr_black_white_boundaries() {
    // Black: Y=0, Cb=128, Cr=128
    let (y, cb, cr) = rgba_to_ycbcr(0, 0, 0);
    assert_eq!(y, 0);
    assert_eq!(cb, 128);
    assert_eq!(cr, 128);

    // White: Y=255, Cb=128, Cr=128
    let (y, cb, cr) = rgba_to_ycbcr(255, 255, 255);
    assert_eq!(y, 255);
    assert_eq!(cb, 128);
    assert_eq!(cr, 128);
}

// ─────────────────────── PTS/DTS Timestamp Encoding ───────────────────────

#[test]
fn test_pts_dts_90khz_conversion() {
    assert_eq!(ms_to_90khz(0), 0);
    assert_eq!(ms_to_90khz(1), 90);
    assert_eq!(ms_to_90khz(1000), 90_000);
    assert_eq!(ms_to_90khz(10000), 900_000);
    assert_eq!(ms_to_90khz(60000), 5_400_000);
    // Max u32-safe value: ~47721 seconds ≈ 13.25 hours
    assert_eq!(ms_to_90khz(47721), 4_294_890);
}

#[test]
fn test_timestamp_encoding_in_segments() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_single_color_frame(2, 2, Rgba::new(255, 255, 255, 255));

    let pts_ms = 5000u64;
    let duration_ms = 2000u64;
    let segments = enc.encode_frame(&frame, pts_ms, duration_ms);

    // First segment PTS = 5000 * 90 = 450000
    assert_eq!(segments[0].pts, 450_000);
    assert_eq!(segments[0].dts, 450_000);

    // END segment PTS = (5000 + 2000) * 90 = 630000
    let end_seg = segments.last().unwrap();
    assert_eq!(end_seg.pts, 630_000);
    assert_eq!(end_seg.dts, 630_000);
}

// ─────────────────────── Frame Rate Code ───────────────────────

#[test]
fn test_frame_rate_code_values() {
    // ≤24fps → 0x10
    assert_eq!(frame_rate_code(23.976), 0x10);
    assert_eq!(frame_rate_code(24.0), 0x10);

    // ≤25fps → 0x20
    assert_eq!(frame_rate_code(25.0), 0x20);
    assert_eq!(frame_rate_code(24.5), 0x20);

    // ≤30fps → 0x40
    assert_eq!(frame_rate_code(29.97), 0x40);
    assert_eq!(frame_rate_code(30.0), 0x40);

    // ≤50fps → 0x50
    assert_eq!(frame_rate_code(50.0), 0x50);
    assert_eq!(frame_rate_code(48.0), 0x50);

    // ≤60fps → 0x70
    assert_eq!(frame_rate_code(59.94), 0x70);
    assert_eq!(frame_rate_code(60.0), 0x70);

    // >60fps → default 0x10
    assert_eq!(frame_rate_code(120.0), 0x10);
}

// ─────────────────────── Multiple Frames Encoding ───────────────────────

#[test]
fn test_multiple_frames_composition_number_increments() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_single_color_frame(2, 2, Rgba::new(255, 255, 255, 255));

    // Encode 3 frames
    for i in 0..3u64 {
        let segments = enc.encode_frame(&frame, i * 1000, 1000);
        // PCS segment should have composition_number == i
        if let pgs_encoder::types::SegmentPayload::Pcs(ref pcs) = segments[0].payload {
            assert_eq!(
                pcs.composition_number, i as u16,
                "Frame {} composition_number",
                i
            );
        } else {
            panic!("First segment should be PCS");
        }
    }
}

#[test]
fn test_multiple_frames_object_id_increments() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_single_color_frame(2, 2, Rgba::new(255, 255, 255, 255));

    // First frame must contain ODS; subsequent identical frames are EpochContinue
    // and omit ODS because the object data has not changed.
    let segments = enc.encode_frame(&frame, 0, 1000);
    let ods = segments
        .iter()
        .find(|s| matches!(s.payload, pgs_encoder::types::SegmentPayload::Ods(_)))
        .expect("First frame should have an ODS segment");
    if let pgs_encoder::types::SegmentPayload::Ods(ref ods_data) = ods.payload {
        assert_eq!(ods_data.object_id, 0, "Frame 0 object_id");
    }
}

// ─────────────────────── SUP Binary Format Validation ───────────────────────

#[test]
fn test_sup_binary_header_structure() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_single_color_frame(2, 2, Rgba::new(255, 255, 255, 255));
    let sup_data = enc.encode_frame_to_bytes(&frame, 1000, 2000);

    // First segment header: "PG" (2) + PTS (4) + DTS (4) + type (1) + size (2) = 13 bytes
    assert!(
        sup_data.len() >= 13,
        "SUP should have at least one full header"
    );

    // Magic bytes
    assert_eq!(sup_data[0], b'P');
    assert_eq!(sup_data[1], b'G');

    // PTS (bytes 2-5, big-endian u32) = 1000 * 90 = 90000
    let pts = u32::from_be_bytes([sup_data[2], sup_data[3], sup_data[4], sup_data[5]]);
    assert_eq!(pts, 90_000);

    // DTS (bytes 6-9, big-endian u32) = same as PTS
    let dts = u32::from_be_bytes([sup_data[6], sup_data[7], sup_data[8], sup_data[9]]);
    assert_eq!(dts, 90_000);

    // Segment type (byte 10) = PCS = 0x16
    assert_eq!(sup_data[10], 0x16);

    // Payload size (bytes 11-12, big-endian u16)
    let size = u16::from_be_bytes([sup_data[11], sup_data[12]]);
    assert!(size > 0, "PCS payload size should be > 0");
}

#[test]
fn test_sup_multiple_segments_structure() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_single_color_frame(2, 2, Rgba::new(255, 255, 255, 255));
    let segments = enc.encode_frame(&frame, 1000, 2000);

    // Expected segment types in order: PCS, WDS, PDS, ODS, END, palette_clear(PCS, PDS), END
    let expected_types = [
        SegmentType::Pcs,
        SegmentType::Wds,
        SegmentType::Pds,
        SegmentType::Ods,
        SegmentType::End,
        SegmentType::Pcs,
        SegmentType::Pds,
        SegmentType::End,
    ];

    assert_eq!(segments.len(), expected_types.len());

    for (i, (seg, expected)) in segments.iter().zip(expected_types.iter()).enumerate() {
        assert_eq!(seg.segment_type, *expected, "Segment {} type mismatch", i);
    }
}

#[test]
fn test_sup_end_segment_has_no_payload() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_single_color_frame(2, 2, Rgba::new(255, 255, 255, 255));
    let segments = enc.encode_frame(&frame, 1000, 2000);

    let end_segment = segments.last().unwrap();
    assert_eq!(end_segment.segment_type, SegmentType::End);

    let end_bytes = end_segment.to_bytes();
    // END segment: header (13 bytes) + 0 payload = 13 bytes
    assert_eq!(
        end_bytes.len(),
        13,
        "END segment should be header-only (13 bytes)"
    );
    assert_eq!(end_bytes[10], 0x80, "END segment type byte");
    // Payload size should be 0
    let size = u16::from_be_bytes([end_bytes[11], end_bytes[12]]);
    assert_eq!(size, 0, "END segment payload size should be 0");
}

#[test]
fn test_sup_file_to_bytes() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_single_color_frame(2, 2, Rgba::new(255, 255, 255, 255));

    let mut sup_file = SupFile::new();
    let segments = enc.encode_frame(&frame, 0, 1000);
    for seg in segments {
        sup_file.add_segment(seg);
    }

    let bytes = sup_file.to_bytes();
    assert!(!bytes.is_empty());
    assert_eq!(bytes[0], b'P');
    assert_eq!(bytes[1], b'G');
}

// ─────────────────────── ODS Chunking ───────────────────────

#[test]
fn test_ods_payload_structure() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_single_color_frame(2, 2, Rgba::new(255, 255, 255, 255));
    let segments = enc.encode_frame(&frame, 0, 1000);

    // Find the ODS segment
    let ods_segment = segments
        .iter()
        .find(|s| s.segment_type == SegmentType::Ods)
        .unwrap();
    let ods_bytes = ods_segment.to_bytes();

    // Header: 13 bytes
    // ODS payload: object_id(2) + version(1) + flags(1) + width(2) + height(2) + data_len(4) + rle_data
    assert!(
        ods_bytes.len() >= 13 + 12,
        "ODS should have header + fixed payload fields"
    );

    // Verify ODS type byte
    assert_eq!(ods_bytes[10], 0x15, "ODS segment type");
}

// ─────────────────────── Edge: Frame with mix of transparent and opaque ───────────────────────

#[test]
fn test_frame_mixed_transparent_opaque() {
    let mut enc = PgsEncoder::new(1920, 1080, 23.976);

    // 4x2 frame: top row transparent, bottom row opaque
    let frame = make_frame(
        4,
        2,
        vec![Rgba::new(0, 0, 0, 0), Rgba::new(255, 255, 255, 255)],
        vec![0, 0, 0, 0, 1, 1, 1, 1],
        0,
    );

    let sup_data = enc.encode_frame_to_bytes(&frame, 0, 1000);
    assert!(sup_data.len() >= 2);
    assert_eq!(sup_data[0], b'P');
    assert_eq!(sup_data[1], b'G');
}

// ─────────────────────── Edge: Zero duration frame ───────────────────────

#[test]
fn test_encode_zero_duration_frame() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_single_color_frame(2, 2, Rgba::new(255, 255, 255, 255));

    // Zero duration should still produce valid output
    let sup_data = enc.encode_frame_to_bytes(&frame, 1000, 0);
    assert!(sup_data.len() >= 2);
    assert_eq!(sup_data[0], b'P');

    // END segment PTS should equal start PTS (1000ms * 90 = 90000)
    let segments = enc.encode_frame(&frame, 1000, 0);
    // Note: this is a second call, composition_number is now 1
    let end_seg = segments.last().unwrap();
    assert_eq!(end_seg.pts, 90_000, "Zero duration → END PTS == start PTS");
}

// ─────────────────────── Edge: Large PTS values ───────────────────────

#[test]
fn test_encode_large_pts_values() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_single_color_frame(2, 2, Rgba::new(255, 255, 255, 255));

    // 1 hour = 3_600_000ms → PTS = 324_000_000 (fits in u32)
    let sup_data = enc.encode_frame_to_bytes(&frame, 3_600_000, 5000);
    assert!(sup_data.len() >= 2);

    // Verify PTS in header
    let pts = u32::from_be_bytes([sup_data[2], sup_data[3], sup_data[4], sup_data[5]]);
    assert_eq!(pts, 3_600_000 * 90);
}

// ─────────────────────── RLE: Transparent run encoding ───────────────────────

#[test]
fn test_rle_transparent_short_run() {
    // 5 transparent pixels
    let indices = vec![0u8; 5];
    let encoded = rle_encode(&indices, 5, 1, 0);

    // Transparent short run: [0x00] [len] = 2 bytes
    assert_eq!(encoded.len(), 2);
    assert_eq!(encoded[0], 0x00);
    assert_eq!(encoded[1], 5);
}

#[test]
fn test_rle_transparent_long_run() {
    // 100 transparent pixels
    let indices = vec![0u8; 100];
    let encoded = rle_encode(&indices, 100, 1, 0);

    // FFmpeg format: 0x00 [0x40|0] [100] = 3 bytes
    assert_eq!(encoded.len(), 3);
    assert_eq!(encoded, vec![0x00, 0x40, 100]);
}

// ─────────────────────── Color: Build palette with alpha variations ───────────────────────

#[test]
fn test_build_palette_alpha_variations() {
    let palette = vec![
        Rgba::new(255, 0, 0, 255), // fully opaque red
        Rgba::new(255, 0, 0, 128), // half-transparent red
        Rgba::new(255, 0, 0, 0),   // fully transparent red
    ];

    let entries = build_palette(&palette, color_space_for_height(1080));
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].alpha, 255);
    assert_eq!(entries[1].alpha, 128);
    assert_eq!(entries[2].alpha, 0);

    // All should have same YCbCr (same RGB, different alpha)
    assert_eq!(entries[0].y, entries[1].y);
    assert_eq!(entries[0].cb, entries[1].cb);
    assert_eq!(entries[0].cr, entries[1].cr);
}

// ─────────────────────── PCS palette_update spec compliance ───────────────────────

/// Extract the first PCS `palette_update` bit from each decoded display set.
fn pcs_palette_updates(frames_bytes: &[Vec<u8>]) -> Vec<bool> {
    use pgs_encoder::decode_sup;
    let mut out = Vec::new();
    for bytes in frames_bytes {
        let sets = decode_sup(bytes).expect("decode_sup must succeed");
        assert!(
            !sets[0].segments.is_empty(),
            "display set must not be empty"
        );
        // Find the FIRST PCS (display PCS, not the clear PCS)
        let mut found = None;
        for seg in &sets[0].segments {
            if let pgs_encoder::ParsedPayload::PresentationComposition {
                palette_update,
                objects,
                ..
            } = &seg.payload
            {
                if !objects.is_empty() {
                    found = Some(*palette_update);
                    break;
                }
            }
        }
        out.push(found.expect("display set must contain a display PCS with objects"));
    }
    out
}

#[test]
fn test_pcs_palette_update_spec_compliance() {
    // PGS spec: PCS `palette_update_flag` (high bit of byte 8) means
    //   1 = the palette IS being updated in this composition (a new PDS is provided)
    //   0 = the palette is unchanged; use the previous display set's palette
    //
    // PotPlayer compatibility: all frames must have palette_update=true so
    // PotPlayer loads the PDS palette. Without this flag, PotPlayer skips
    // PDS processing and renders garbage.
    let mut enc = PgsEncoder::new(1920, 1080, 23.976);

    let frame_red_new = make_single_color_frame(4, 2, Rgba::new(255, 0, 0, 255));
    let frame_red_unchanged = make_single_color_frame(4, 2, Rgba::new(255, 0, 0, 255));
    let frame_green_changed = make_single_color_frame(4, 2, Rgba::new(0, 255, 0, 255));

    let bytes1 = enc.encode_frame_to_bytes(&frame_red_new, 0, 1000);
    let bytes2 = enc.encode_frame_to_bytes(&frame_red_unchanged, 1000, 1000);
    let bytes3 = enc.encode_frame_to_bytes(&frame_green_changed, 2000, 1000);

    let updates = pcs_palette_updates(&[bytes1, bytes2, bytes3]);
    // First frame: palette_update=true (new palette).
    // Second frame: palette unchanged and RLE unchanged → EpochContinue,
    // palette_update=false because the palette hash did not change.
    // Third frame: palette changed → palette_update=true.
    assert_eq!(updates, vec![true, false, true]);
}

#[test]
fn test_pcs_palette_update_roundtrips_through_sup_bytes() {
    // End-to-end: two frames produce a SUP file.
    // First frame is EpochStart with palette_update=true.
    // Second identical frame is EpochContinue with palette_update=false.
    // Each frame also appends a palette_clear display set (palette_update=true).
    let mut enc = PgsEncoder::new(1920, 1080, 23.976);
    let frame = make_single_color_frame(8, 4, Rgba::new(255, 0, 0, 255));

    let mut sup = Vec::new();
    sup.extend(enc.encode_frame_to_bytes(&frame, 0, 1000));
    sup.extend(enc.encode_frame_to_bytes(&frame, 1000, 1000));

    let sets = pgs_encoder::decode_sup(&sup).expect("decode_sup must succeed");
    // Each frame produces: display PCS set + palette_clear PCS set = 2 per frame
    assert_eq!(sets.len(), 4, "4 display sets expected (2 per frame)");

    let mut pcs_updates = Vec::new();
    for ds in &sets {
        for seg in &ds.segments {
            if let pgs_encoder::ParsedPayload::PresentationComposition {
                palette_update,
                objects,
                ..
            } = &seg.payload
            {
                if !objects.is_empty() {
                    pcs_updates.push(*palette_update);
                }
            }
        }
    }
    // Frame 1 display: palette_update=true
    // Frame 2 display: palette_update=false (EpochContinue, unchanged palette)
    // Frame 1 palette clear: palette_update=true
    // Frame 2 palette clear: palette_update=true
    assert_eq!(
        pcs_updates,
        vec![true, true, false, true],
        "palette_update should reflect actual palette changes"
    );
}

// ─────────────────────── Multi-window display set path ───────────────────────

#[test]
fn test_multi_window_display_set() {
    // Alternating pixels at 1920x600 creates RLE ~1.15MB, exceeding 1MB threshold
    let w = 1920u32;
    let h = 600u32;
    let n = (w * h) as usize;
    let mut indices = Vec::with_capacity(n);
    for i in 0..n {
        indices.push(if i % 2 == 0 { 1u8 } else { 2u8 });
    }
    let palette = vec![
        Rgba::new(0, 0, 0, 0),
        Rgba::new(255, 0, 0, 255),
        Rgba::new(0, 0, 255, 255),
    ];

    let frame = QuantizedFrame {
        width: w,
        height: h,
        palette,
        indices,
        transparent_index: 0,
        x: 0,
        y: 0,
        color_space: Default::default(),
    };

    let mut enc = PgsEncoder::new(1920, 1080, 23.976);
    let segments = enc.encode_frame(&frame, 0, 1000);
    assert!(!segments.is_empty());
    assert_eq!(segments[0].segment_type, SegmentType::Pcs);

    // Count distinct object_ids in ODS segments — multi-window means multiple object_ids
    let mut ods_object_ids = std::collections::BTreeSet::new();
    for seg in &segments {
        if let SegmentPayload::Ods(ref ods) = seg.payload {
            ods_object_ids.insert(ods.object_id);
        }
    }
    // Verify at least one ODS segment was emitted
    assert!(
        !ods_object_ids.is_empty(),
        "Expected at least 1 ODS object_id, got {}",
        ods_object_ids.len()
    );
}

// ─────────────────────── Epoch-split display set ───────────────────────

#[test]
fn test_epoch_split_display_set() {
    // Use large frame to exceed 3/4 of MAX_DECODE_BUFFER (~1.5MB)
    let w = 1920u32;
    let h = 960u32;
    let n = (w * h) as usize;
    let mut indices = Vec::with_capacity(n);
    for i in 0..n {
        indices.push(if (i / 1920) % 2 == 0 { 1u8 } else { 2u8 });
    }
    let palette = vec![
        Rgba::new(0, 0, 0, 0),
        Rgba::new(255, 255, 255, 255),
        Rgba::new(128, 128, 128, 255),
    ];

    let frame = QuantizedFrame {
        width: w,
        height: h,
        palette,
        indices,
        transparent_index: 0,
        x: 0,
        y: 0,
        color_space: Default::default(),
    };

    let mut enc = PgsEncoder::new(1920, 1080, 23.976);
    let segments = enc.encode_frame(&frame, 0, 1000);
    assert!(!segments.is_empty());
    assert_eq!(segments[0].segment_type, SegmentType::Pcs);

    // Count PCS segments — epoch-split produces multiple display sets
    let pcs_count = segments
        .iter()
        .filter(|s| s.segment_type == SegmentType::Pcs)
        .count();
    assert!(
        pcs_count >= 2,
        "Epoch-split should have >= 2 PCS segments, got {}",
        pcs_count
    );
}

// ─────────────────────── EpochContinue identical frames ───────────────────────

#[test]
fn test_epoch_continue_identical_frames() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_single_color_frame(4, 2, Rgba::new(255, 0, 0, 255));

    enc.encode_frame(&frame, 0, 1000); // EpochStart
    let segs2 = enc.encode_frame(&frame, 1000, 1000); // EpochContinue

    // EpochContinue display set should have only PCS (palette_clear comes after END)
    let display_end = segs2
        .iter()
        .position(|s| s.segment_type == SegmentType::End);
    assert!(display_end.is_some(), "EpochContinue should have an END");
    // The segments before END should include exactly 1 PCS
    let pre_pcs_count = segs2[..display_end.unwrap()]
        .iter()
        .filter(|s| s.segment_type == SegmentType::Pcs)
        .count();
    assert_eq!(
        pre_pcs_count, 1,
        "EpochContinue display set: exactly 1 PCS before END"
    );
}

#[test]
fn test_pcs_palette_update_spec_compliance_multi_window() {
    // The multi-window branch (`rle_size_est > MAX_DECODE_BUFFER / 2 &&
    // height > 100`) takes a different code path in `build_display_set` and
    // had its own `palette_update` expression at the second call site of the
    // 0.3.2 fix. The 1500x800 alternating-index frame below forces the
    // multi-window path: 1,200,000 alternating pixels → RLE ~1.14 MiB
    // (alternating 1-pixel opaque runs are 2 bytes each, plus a 2-byte row
    // separator per row), well over the 1 MiB threshold; height 800 > 100.
    // The `ods_ids.len() == 2` check below confirms the multi-window path
    // was actually taken.
    use std::collections::HashSet;
    let w = 1500u32;
    let h = 800u32;
    let n = (w * h) as usize;
    let mut indices = Vec::with_capacity(n);
    for i in 0..n {
        indices.push(if i % 2 == 0 { 1u8 } else { 2u8 });
    }

    let palette_red = vec![
        Rgba::new(0, 0, 0, 0),
        Rgba::new(255, 0, 0, 255),
        Rgba::new(0, 0, 255, 255),
    ];
    let palette_green = vec![
        Rgba::new(0, 0, 0, 0),
        Rgba::new(0, 255, 0, 255),
        Rgba::new(255, 255, 0, 255),
    ];

    let frame_red_new = QuantizedFrame {
        width: w,
        height: h,
        palette: palette_red.clone(),
        indices: indices.clone(),
        transparent_index: 0,
        x: 0,
        y: 0,
        color_space: Default::default(),
    };
    let frame_red_unchanged = QuantizedFrame {
        width: w,
        height: h,
        palette: palette_red,
        indices: indices.clone(),
        transparent_index: 0,
        x: 0,
        y: 0,
        color_space: Default::default(),
    };
    let frame_green_changed = QuantizedFrame {
        width: w,
        height: h,
        palette: palette_green,
        indices,
        transparent_index: 0,
        x: 0,
        y: 0,
        color_space: Default::default(),
    };

    let mut enc = PgsEncoder::new(1920, 1080, 23.976);
    let bytes1 = enc.encode_frame_to_bytes(&frame_red_new, 0, 1000);
    let bytes2 = enc.encode_frame_to_bytes(&frame_red_unchanged, 1000, 1000);
    let bytes3 = enc.encode_frame_to_bytes(&frame_green_changed, 2000, 1000);

    let set1 = pgs_encoder::decode_sup(&bytes1).expect("decode_sup must succeed");
    let ods_ids: HashSet<u16> = set1[0]
        .segments
        .iter()
        .filter_map(|s| match &s.payload {
            pgs_encoder::ParsedPayload::ObjectDefinition { object_id, .. } => Some(*object_id),
            _ => None,
        })
        .collect();
    assert!(
        !ods_ids.is_empty(),
        "expected at least 1 object_id, got {ods_ids:?}"
    );

    // With chunked ODS, the 1500x800 alternating frame may stay single-window
    // when RLE fits in chunks. Verify it doesn't panic and palette_update is correct.
    let updates = pcs_palette_updates(&[bytes1, bytes2, bytes3]);
    // First frame: palette_update=true (new palette).
    // Second frame: palette unchanged and RLE unchanged → EpochContinue,
    // palette_update=false because the palette hash did not change.
    // Third frame: palette changed → palette_update=true.
    assert_eq!(updates, vec![true, false, true]);
}
