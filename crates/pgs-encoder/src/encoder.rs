use crate::color::build_palette;
use crate::domain::epoch::{hash_palette, DisplaySetKind, EpochManager};
use crate::domain::timing::is_ntsc_fps;
use crate::encoding::display_set as ds;
pub use crate::types::frame_rate_code;
use crate::types::*;
use color_quantizer::QuantizedFrame;

const MAX_DECODE_BUFFER: usize = 2 * 1024 * 1024;

pub struct PgsEncoder {
    pub composition_number: u16,
    pub object_id: u16,
    pub palette_id: u8,
    pub window_id: u8,
    pub frame_rate: u8,
    pub display_width: u16,
    pub display_height: u16,
    pub fps: f64,
    pub epoch: EpochManager,
    pub potplayer_compat: bool,
}

impl PgsEncoder {
    fn make_config(&self) -> ds::DisplaySetConfig {
        ds::DisplaySetConfig {
            display_width: self.display_width,
            display_height: self.display_height,
            frame_rate: self.frame_rate,
            composition_number: self.composition_number,
            object_id: self.object_id,
            palette_id: self.palette_id,
            window_id: self.window_id,
            potplayer_compat: self.potplayer_compat,
        }
    }

    pub fn new(display_width: u16, display_height: u16, fps: f64) -> Self {
        Self {
            composition_number: 0,
            object_id: 0,
            palette_id: 0,
            window_id: 0,
            frame_rate: frame_rate_code(fps),
            display_width,
            display_height,
            fps,
            epoch: EpochManager::new(),
            potplayer_compat: true,
        }
    }

    pub fn ms_to_90khz(&self, ms: u64) -> u64 {
        if is_ntsc_fps(self.fps) {
            (u128::from(ms) * 90000 * 1001 / 1000000) as u64
        } else {
            ms * 90
        }
    }

    pub fn encode_frame(
        &mut self,
        frame: &QuantizedFrame,
        pts_ms: u64,
        duration_ms: u64,
    ) -> Vec<Segment> {
        let pts = self.ms_to_90khz(pts_ms);
        let dts = pts;
        let pts_end = self.ms_to_90khz(pts_ms + duration_ms);
        let mut segments = Vec::new();
        segments.extend(self.build_display_set(frame, pts, dts));
        segments.push(Segment {
            segment_type: SegmentType::End,
            pts,
            dts,
            payload: SegmentPayload::End,
        });
        segments.extend(self.build_palette_clear_display_set(pts_end, pts_end));
        segments.push(Segment {
            segment_type: SegmentType::End,
            pts: pts_end,
            dts: pts_end,
            payload: SegmentPayload::End,
        });
        self.composition_number = self.composition_number.wrapping_add(1);
        self.epoch.frame_count += 1;
        segments
    }

    pub fn encode_frame_to_bytes(
        &mut self,
        frame: &QuantizedFrame,
        pts_ms: u64,
        duration_ms: u64,
    ) -> Vec<u8> {
        let segments = self.encode_frame(frame, pts_ms, duration_ms);
        let mut output = Vec::new();
        for seg in &segments {
            output.extend(seg.to_bytes());
        }
        output
    }

    pub fn build_display_set(
        &mut self,
        frame: &QuantizedFrame,
        pts: u64,
        dts: u64,
    ) -> Vec<Segment> {
        let config = self.make_config();
        let mut palette_entries = build_palette(&frame.palette, frame.color_space);
        let palette_hash = hash_palette(&palette_entries);
        let (rle, rle_hash) = ds::prepare_rle_and_hash(
            &mut palette_entries,
            &frame.indices,
            frame.width,
            frame.height,
            frame.transparent_index,
        );

        let kind = self.epoch.decide_kind(palette_hash, rle_hash);
        let (composition_state, palette_update) = match kind {
            DisplaySetKind::EpochStart => (CompositionState::EpochStart, true),
            DisplaySetKind::NormalCase => {
                let palette_changed = self.epoch.prev_palette_hash != Some(palette_hash);
                (CompositionState::NormalCase, palette_changed)
            }
            DisplaySetKind::EpochContinue => (CompositionState::EpochContinue, false),
            DisplaySetKind::PaletteOnly => (CompositionState::NormalCase, true),
        };

        let cfg = &config;
        let fc = self.epoch.frame_count;
        let ov = self.epoch.object_version;
        let segments = match kind {
            DisplaySetKind::EpochContinue => {
                ds::build_continue_display_set(cfg, frame, pts, dts, composition_state)
            }
            DisplaySetKind::PaletteOnly => ds::build_palette_only_display_set(
                cfg,
                frame,
                pts,
                dts,
                palette_update,
                &palette_entries,
                fc,
            ),
            DisplaySetKind::EpochStart | DisplaySetKind::NormalCase => {
                let rle_size_est = 13 + 4 + rle.len();
                let use_multi_window = rle_size_est > MAX_DECODE_BUFFER / 2 && frame.height > 100;
                if use_multi_window {
                    ds::build_multi_window_display_set(
                        cfg,
                        frame,
                        pts,
                        dts,
                        &palette_entries,
                        composition_state,
                        palette_update,
                        fc,
                        ov,
                    )
                } else {
                    ds::build_single_window_display_set(
                        cfg,
                        frame,
                        pts,
                        dts,
                        &palette_entries,
                        &rle,
                        composition_state,
                        palette_update,
                        fc,
                        ov,
                    )
                }
            }
        };

        let total_size: usize = segments.iter().map(|s| s.to_bytes().len()).sum();
        if total_size > MAX_DECODE_BUFFER * 3 / 4 {
            ds::build_epoch_split_display_set(
                cfg,
                frame,
                pts,
                dts,
                composition_state,
                palette_update,
                fc,
                ov,
            )
        } else {
            self.epoch.update(palette_hash, rle_hash);
            segments
        }
    }

    fn build_palette_clear_display_set(&self, pts: u64, dts: u64) -> Vec<Segment> {
        ds::build_palette_clear_display_set(&self.make_config(), pts, dts, self.epoch.frame_count)
    }
}

/// Convert milliseconds to 90kHz PTS ticks.
pub fn ms_to_90khz(ms: u64) -> u64 {
    ms * 90
}

/// Parse an ASS-style timecode string into milliseconds.
pub fn timecode_to_ms(timecode: &str) -> Option<u64> {
    let parts: Vec<&str> = timecode.split(':').collect();
    if parts.len() != 3 {
        return None;
    }
    let h: u64 = parts[0].parse().ok()?;
    let m: u64 = parts[1].parse().ok()?;
    let sec_parts: Vec<&str> = parts[2].split('.').collect();
    if sec_parts.len() != 2 {
        return None;
    }
    let s: u64 = sec_parts[0].parse().ok()?;
    let cs: u64 = sec_parts[1].parse().ok()?;
    Some(h * 3600000 + m * 60000 + s * 1000 + cs * 10)
}

#[cfg(test)]
mod tests {
    use super::*;

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
            x: 0,
            y: 0,
            color_space: Default::default(),
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
        assert_eq!(segments.len(), 8);
        assert_eq!(segments[0].segment_type, SegmentType::Pcs);
        assert_eq!(segments[4].segment_type, SegmentType::End);
        assert_eq!(segments[7].segment_type, SegmentType::End);
    }

    #[test]
    fn test_encode_frame_pts() {
        let mut enc = PgsEncoder::new(1920, 1080, 24.0);
        let frame = make_test_frame();
        let segments = enc.encode_frame(&frame, 1000, 2000);
        assert_eq!(segments[0].pts, 90000);
        assert_eq!(segments[7].pts, 270000);
    }

    #[test]
    fn test_encode_frame_increments_ids() {
        let mut enc = PgsEncoder::new(1920, 1080, 24.0);
        let frame = make_test_frame();
        enc.encode_frame(&frame, 0, 1000);
        assert_eq!(enc.composition_number, 1);
        assert_eq!(enc.object_id, 0);
        enc.encode_frame(&frame, 1000, 1000);
        assert_eq!(enc.composition_number, 2);
        assert_eq!(enc.object_id, 0);
    }

    #[test]
    fn test_build_display_set() {
        let mut enc = PgsEncoder::new(1920, 1080, 24.0);
        let frame = make_test_frame();
        let segments = enc.encode_frame(&frame, 1000, 2000);
        assert!(segments.len() >= 4);
    }

    #[test]
    fn test_ms_to_90khz() {
        let enc = PgsEncoder::new(1920, 1080, 24.0);
        assert_eq!(enc.ms_to_90khz(0), 0);
        assert_eq!(enc.ms_to_90khz(1000), 90000);
        assert_eq!(enc.ms_to_90khz(1), 90);
    }

    #[test]
    fn test_ms_to_90khz_ntsc() {
        let enc = PgsEncoder::new(1920, 1080, 23.976);
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
        assert!(!pcs_segments.is_empty(), "Need at least one PCS");
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
            assert!(p.palette_update, "palette changed");
        }
    }

    #[test]
    fn test_palette_update_false_when_unchanged() {
        let mut enc = PgsEncoder::new(1920, 1080, 24.0);
        let frame = make_test_frame();
        enc.encode_frame(&frame, 0, 1000);
        let segments = enc.encode_frame(&frame, 1000, 1000);
        let display_pcs = segments
            .iter()
            .find(|s| matches!(s.payload, SegmentPayload::Pcs(_)))
            .unwrap();
        if let SegmentPayload::Pcs(ref p) = display_pcs.payload {
            assert!(!p.palette_update, "palette unchanged => false");
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
        assert!(
            pcs_count >= 1,
            "EpochContinue needs at least 1 PCS in display set"
        );
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
}
