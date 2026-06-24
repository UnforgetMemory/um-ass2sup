use color_quantizer::{QuantizedFrame, Rgba};
use pgs_encoder::*;
use proptest::prelude::*;

/// Generate a random QuantizedFrame for testing.
fn arb_quantized_frame() -> impl Strategy<Value = QuantizedFrame> {
    let width = 1u32..=32;
    let height = 1u32..=32;
    (width, height)
        .prop_flat_map(|(w, h)| {
            let pixel_count = (w * h) as usize;
            let palette_size = 2usize..=8;
            let palette = proptest::collection::vec(
                (any::<u8>(), any::<u8>(), any::<u8>(), any::<u8>())
                    .prop_map(|(r, g, b, a)| Rgba::new(r, g, b, a)),
                palette_size,
            );
            let indices = proptest::collection::vec(any::<u8>(), pixel_count);
            (Just(w), Just(h), palette, indices, any::<u8>())
        })
        .prop_map(
            |(width, height, palette, indices, transparent_index)| QuantizedFrame {
                width,
                height,
                palette,
                indices,
                transparent_index,
                x: 0,
                y: 0,
                color_space: Default::default(),
            },
        )
}

// ============================================================
// Property: Encoded display set always ends with End segment
// ============================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn display_set_ends_with_end(
        frame in arb_quantized_frame(),
        pts_ms in 0u64..=60000u64,
        duration_ms in 0u64..=60000u64,
    ) {
        let mut encoder = PgsEncoder::new(1920, 1080, 24.0);
        let segments = encoder.encode_frame(&frame, pts_ms, duration_ms);

        assert!(segments.len() >= 5, "segments.len() = {}", segments.len());
        assert_eq!(
            segments.last().unwrap().segment_type,
            SegmentType::End,
            "Last segment must be End"
        );
    }
}

// ============================================================
// Property: Segment types appear in correct order
// ============================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn segment_order_is_valid(
        frame in arb_quantized_frame(),
        pts_ms in 0u64..=60000u64,
        duration_ms in 0u64..=60000u64,
    ) {
        let mut encoder = PgsEncoder::new(1920, 1080, 24.0);
        let segments = encoder.encode_frame(&frame, pts_ms, duration_ms);

        let non_end: Vec<_> = segments.iter()
            .take_while(|s| s.segment_type != SegmentType::End)
            .collect();

        assert!(!non_end.is_empty(), "Display set must have non-end segments");
        assert_eq!(non_end[0].segment_type, SegmentType::Pcs);
        assert_eq!(non_end[1].segment_type, SegmentType::Wds);
        assert_eq!(non_end[2].segment_type, SegmentType::Pds);

        for seg in &non_end[3..] {
            assert_eq!(seg.segment_type, SegmentType::Ods);
        }
    }
}

// ============================================================
// Property: Serialized segments start with "PG" magic bytes
// ============================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn serialized_segment_has_pg_magic(
        frame in arb_quantized_frame(),
        pts_ms in 0u64..=60000u64,
    ) {
        let mut encoder = PgsEncoder::new(1920, 1080, 24.0);
        let segments = encoder.encode_frame(&frame, pts_ms, 2000);

        for segment in &segments {
            let bytes = segment.to_bytes();
            assert!(bytes.len() >= 2, "Segment too short: {} bytes", bytes.len());
            assert_eq!(bytes[0], b'P', "Missing PG magic P");
            assert_eq!(bytes[1], b'G', "Missing PG magic G");
        }
    }
}

// ============================================================
// Property: Segment type field in serialized bytes matches enum
// ============================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn serialized_segment_type_matches(
        frame in arb_quantized_frame(),
        pts_ms in 0u64..=60000u64,
    ) {
        let mut encoder = PgsEncoder::new(1920, 1080, 24.0);
        let segments = encoder.encode_frame(&frame, pts_ms, 2000);

        for segment in &segments {
            let bytes = segment.to_bytes();
            assert_eq!(bytes[10], segment.segment_type as u8);
        }
    }
}

// ============================================================
// Property: Multiple frames maintain segment structure
// ============================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(50))]

    #[test]
    fn multiple_frames_maintain_structure(
        frames in proptest::collection::vec(arb_quantized_frame(), 1..=5),
    ) {
        let mut encoder = PgsEncoder::new(1920, 1080, 24.0);

        for (i, frame) in frames.iter().enumerate() {
            let pts_ms = (i as u64) * 2000;
            let segments = encoder.encode_frame(frame, pts_ms, 2000);

            assert!(!segments.is_empty(), "Frame {} produced empty segments", i);
            assert_eq!(
                segments.last().unwrap().segment_type,
                SegmentType::End,
                "Frame {} last segment must be End",
                i
            );

            if i > 0 {
                let expected_pts = pts_ms * 90;
                assert!(
                    segments[0].pts >= expected_pts,
                    "Frame {} PTS {} < expected {}",
                    i, segments[0].pts, expected_pts
                );
            }
        }
    }
}

// ============================================================
// Property: Payload serialization doesn't panic
// ============================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    #[test]
    fn payload_serialization_never_panics(
        frame in arb_quantized_frame(),
        pts_ms in 0u64..=60000u64,
    ) {
        let mut encoder = PgsEncoder::new(1920, 1080, 24.0);
        let segments = encoder.encode_frame(&frame, pts_ms, 2000);

        for segment in &segments {
            let bytes = segment.to_bytes();
            match &segment.payload {
                SegmentPayload::End => {
                    assert_eq!(bytes.len(), 13);
                }
                _ => {
                    assert!(bytes.len() > 13, "Segment {:?} payload too short", segment.segment_type);
                }
            }
        }
    }
}

// ============================================================
// Property: RLE roundtrip: rle_encode(rle_decode(x)) = x
// ============================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn rle_roundtrip(
        indices in proptest::collection::vec(any::<u8>(), 1..=200usize),
        width in 1u32..=20u32,
    ) {
        // transparent_index is always 0: the encoder swaps palette so index 0 = transparent
        // before calling rle_encode. rle_decode reverses this swap after decoding.
        let transparent_index = 0u8;
        // Round up to full rows
        let height = ((indices.len() as f64) / (width as f64)).ceil() as u32;
        let total = (width * height) as usize;
        let mut padded = indices.clone();
        padded.resize(total, transparent_index);

        let encoded = pgs_encoder::rle::rle_encode(&padded, width, height, transparent_index);
        let decoded = pgs_encoder::rle::rle_decode(&encoded, width, height, transparent_index)
            .expect("RLE decode must succeed");
        assert_eq!(decoded.len(), total, "Decoded length must match total pixels");
        assert_eq!(decoded, padded, "Roundtrip must preserve indices");
    }
}
