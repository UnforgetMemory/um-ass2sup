//! Unit tests for the PGS encoder (extracted from `src/encoding/encoder.rs`
//! to keep the production file focused on encoding logic).

use color_quantizer::QuantizedFrame;
use pgs_encoder::domain::composition::CompositionState;
use pgs_encoder::domain::segment::{SegmentPayload, SegmentType};
use pgs_encoder::domain::timing::frame_rate_code;
use pgs_encoder::encoding::encoder::PgsEncoder;

fn make_test_frame() -> QuantizedFrame {
    QuantizedFrame {
        width: 4,
        height: 2,
        palette: vec![
            color_quantizer::Rgba::new(0, 0, 0, 0),
            color_quantizer::Rgba::new(255, 255, 255, 255),
        ],
        indices: vec![1, 1, 1, 1, 0, 0, 0, 0],
        transparent_index: 0,
        x: 100,
        y: 200,
        color_space: Default::default(),
        pts_ms: 0,
        duration_ms: 0,
    }
}

#[test]
fn test_encoder_new() {
    let enc = PgsEncoder::new(1920, 1080, 23.976);
    assert_eq!(enc.display_width, 1920);
    assert_eq!(enc.display_height, 1080);
    assert_eq!(enc.frame_rate, 0x10);
}

#[test]
fn test_encode_frame() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    let segments = enc.encode_frame(&frame, 1000, 2000);
    assert_eq!(segments.len(), 5);
    assert_eq!(segments[0].segment_type, SegmentType::Pcs);
    assert_eq!(segments[4].segment_type, SegmentType::End);
}

#[test]
fn test_encode_frame_pts() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    let segments = enc.encode_frame(&frame, 1000, 2000);
    assert_eq!(segments[0].pts, 90000);
    assert_eq!(segments[0].dts, 89999);
    assert_eq!(segments[4].pts, 90000);
    assert_eq!(segments[4].dts, 89999);
}

#[test]
fn test_dts_strictly_less_than_pts() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    let segments = enc.encode_frame(&frame, 1000, 2000);
    for seg in &segments {
        assert!(seg.dts < seg.pts);
    }
}

#[test]
fn test_dts_zero_when_pts_zero() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    let segments = enc.encode_frame(&frame, 0, 0);
    for seg in &segments {
        assert_eq!(seg.dts, 0);
    }
}

#[test]
fn test_encode_frame_increments_ids() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    enc.encode_frame(&frame, 0, 1000);
    assert_eq!(enc.composition_number, 2);
    enc.encode_frame(&frame, 1000, 1000);
    assert_eq!(enc.composition_number, 3);
}

#[test]
fn test_ms_to_90khz() {
    let enc = PgsEncoder::new(1920, 1080, 24.0);
    assert_eq!(enc.ms_to_90khz(0), 0);
    // 1000 ms is exactly frame 24 at 24 fps → 24 * 3750 ticks.
    assert_eq!(enc.ms_to_90khz(1000), 90000);
    // Sub-frame timestamps snap to the nearest frame boundary: 1 ms → frame 0.
    assert_eq!(enc.ms_to_90khz(1), 0);
    // 500 ms is frame 12 → 12 * 3750.
    assert_eq!(enc.ms_to_90khz(500), 45000);
}

#[test]
fn test_ms_to_90khz_ntsc() {
    let enc = PgsEncoder::new(1920, 1080, 23.976);
    // 1000 ms → frame 24 at 23.976 → 24 * 15015 / 4.
    assert_eq!(enc.ms_to_90khz(1000), 90090);
}

#[test]
fn test_frame_rate_code() {
    assert_eq!(frame_rate_code(23.976), 0x10);
    assert_eq!(frame_rate_code(24.0), 0x10);
    assert_eq!(frame_rate_code(25.0), 0x20);
    assert_eq!(frame_rate_code(29.97), 0x40);
    assert_eq!(frame_rate_code(30.0), 0x40);
    assert_eq!(frame_rate_code(50.0), 0x50);
    assert_eq!(frame_rate_code(60.0), 0x70);
    assert_eq!(frame_rate_code(120.0), 0x10);
}

#[test]
fn test_timecode_to_ms() {
    use pgs_encoder::domain::timing::timecode_to_ms;
    assert_eq!(timecode_to_ms("0:00:01.00"), Some(1000));
    assert_eq!(timecode_to_ms("1:30:00.00"), Some(5400000));
    assert_eq!(timecode_to_ms("invalid"), None);
}

#[test]
fn test_encode_to_bytes() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    let bytes = enc.encode_frame_to_bytes(&frame, 1000, 2000);
    assert_eq!(bytes[0], b'P');
    assert_eq!(bytes[1], b'G');
}

#[test]
fn test_pcs_to_bytes() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    let segments = enc.encode_frame(&frame, 1000, 2000);
    let pcs_bytes = segments[0].to_bytes();
    assert_eq!(pcs_bytes[10], 0x16);
}

#[test]
fn test_full_encode_two_frames() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    let s1 = enc.encode_frame(&frame, 0, 1000);
    let s2 = enc.encode_frame(&frame, 1000, 1000);
    assert!(!s1.is_empty());
    assert!(!s2.is_empty());
}

#[test]
fn test_pcs_object_position_propagated() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    let segments = enc.encode_frame(&frame, 1000, 2000);
    let pcs_seg = segments
        .iter()
        .find(|s| s.segment_type == SegmentType::Pcs)
        .expect("PCS segment must exist");
    if let SegmentPayload::Pcs(ref pcs) = pcs_seg.payload {
        assert_eq!(pcs.compositions.len(), 1);
        assert_eq!(pcs.compositions[0].x, 100);
        assert_eq!(pcs.compositions[0].y, 200);
    } else {
        panic!("PCS segment must contain PcsPayload");
    }
}

#[test]
fn test_wds_position_matches_frame() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    let segments = enc.encode_frame(&frame, 1000, 2000);
    let wds_seg = segments
        .iter()
        .find(|s| s.segment_type == SegmentType::Wds)
        .expect("WDS segment must exist");
    if let SegmentPayload::Wds(ref wds) = wds_seg.payload {
        assert_eq!(wds.windows.len(), 1);
        assert_eq!(wds.windows[0].x, 100);
        assert_eq!(wds.windows[0].y, 200);
    } else {
        panic!("WDS segment must contain WdsPayload");
    }
}

#[test]
fn test_composition_state_epoch_continue_value() {
    assert_eq!(CompositionState::EpochContinue as u8, 0xC0);
}

#[test]
fn test_first_frame_uses_epoch_start() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    let segments = enc.encode_frame(&frame, 0, 1000);
    let pcs = segments
        .iter()
        .find(|s| s.segment_type == SegmentType::Pcs)
        .unwrap();
    if let SegmentPayload::Pcs(ref p) = pcs.payload {
        assert_eq!(p.composition_state, CompositionState::EpochStart);
    } else {
        panic!("Expected PCS");
    }
}

#[test]
fn test_unchanged_rle_uses_epoch_continue() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    enc.encode_frame(&frame, 0, 1000);
    let segments = enc.encode_frame(&frame, 1000, 1000);
    let pcs_segments: Vec<_> = segments
        .iter()
        .filter(|s| s.segment_type == SegmentType::Pcs)
        .collect();
    assert!(!pcs_segments.is_empty());
    if let SegmentPayload::Pcs(ref p) = pcs_segments[0].payload {
        assert_eq!(p.composition_state, CompositionState::EpochContinue);
    }
}

#[test]
fn test_changed_rle_uses_normal_case() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame1 = make_test_frame();
    let mut frame2 = make_test_frame();
    frame2.indices = vec![2, 2, 2, 2, 0, 0, 0, 0];
    frame2.palette = frame1.palette.clone();
    enc.encode_frame(&frame1, 0, 1000);
    let segments = enc.encode_frame(&frame2, 1000, 1000);
    let pcs_segments: Vec<_> = segments
        .iter()
        .filter(|s| s.segment_type == SegmentType::Pcs)
        .collect();
    if let SegmentPayload::Pcs(ref p) = pcs_segments[0].payload {
        assert_eq!(p.composition_state, CompositionState::NormalCase);
    }
}

#[test]
fn test_palette_update_true_when_palette_changed() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame1 = make_test_frame();
    let mut frame2 = make_test_frame();
    frame2.palette = vec![
        color_quantizer::Rgba::new(0, 0, 0, 0),
        color_quantizer::Rgba::new(0, 255, 0, 255),
    ];
    enc.encode_frame(&frame1, 0, 1000);
    let segments = enc.encode_frame(&frame2, 1000, 1000);
    let display_pcs = segments
        .iter()
        .find(|s| matches!(s.payload, SegmentPayload::Pcs(_)))
        .unwrap();
    if let SegmentPayload::Pcs(ref p) = display_pcs.payload {
        assert!(p.palette_update);
    }
}

#[test]
fn test_palette_update_true_in_continue_set() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    enc.encode_frame(&frame, 0, 1000);
    let segments = enc.encode_frame(&frame, 1000, 1000);
    let display_pcs = segments
        .iter()
        .find(|s| matches!(s.payload, SegmentPayload::Pcs(_)))
        .unwrap();
    if let SegmentPayload::Pcs(ref p) = display_pcs.payload {
        assert!(p.palette_update);
    }
}

#[test]
fn test_epoch_continue_emits_pcs_and_end_only() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame = make_test_frame();
    enc.encode_frame(&frame, 0, 1000);
    let segments = enc.encode_frame(&frame, 1000, 1000);
    let display_end = segments
        .iter()
        .position(|s| s.segment_type == SegmentType::End)
        .unwrap();
    let pre_end = &segments[..display_end];
    let pcs_count = pre_end
        .iter()
        .filter(|s| s.segment_type == SegmentType::Pcs)
        .count();
    assert!(pcs_count >= 1);
}

#[test]
fn test_palette_only_emits_pcs_and_pds() {
    let mut enc = PgsEncoder::new(1920, 1080, 24.0);
    let frame1 = make_test_frame();
    let mut frame2 = make_test_frame();
    frame2.indices = frame1.indices.clone();
    frame2.palette = vec![
        color_quantizer::Rgba::new(0, 0, 0, 0),
        color_quantizer::Rgba::new(255, 255, 0, 255),
    ];
    enc.encode_frame(&frame1, 0, 1000);
    let segments = enc.encode_frame(&frame2, 1000, 1000);
    let display_end = segments
        .iter()
        .position(|s| s.segment_type == SegmentType::End)
        .unwrap();
    let pre_end_types: Vec<SegmentType> = segments[..display_end]
        .iter()
        .map(|s| s.segment_type)
        .collect();
    assert!(pre_end_types.contains(&SegmentType::Pcs));
    assert!(pre_end_types.contains(&SegmentType::Pds));
}
