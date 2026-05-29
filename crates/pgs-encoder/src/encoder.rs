use color_quantizer::QuantizedFrame;
use crate::types::*;
use crate::rle::{rle_encode, chunk_rle_data};
use crate::color::build_palette;

const MAX_ODS_CHUNK: usize = 0xFFE0;

pub struct PgsEncoder {
    composition_number: u16,
    object_id: u16,
    palette_id: u8,
    window_id: u8,
    frame_rate: u8,
    display_width: u16,
    display_height: u16,
}

impl PgsEncoder {
    pub fn new(display_width: u16, display_height: u16, fps: f64) -> Self {
        Self {
            composition_number: 0,
            object_id: 0,
            palette_id: 0,
            window_id: 0,
            frame_rate: frame_rate_code(fps),
            display_width,
            display_height,
        }
    }

    pub fn encode_frame(&mut self, frame: &QuantizedFrame, pts_ms: u64, duration_ms: u64) -> Vec<Segment> {
        let pts = ms_to_90khz(pts_ms);
        let dts = pts;
        let pts_end = ms_to_90khz(pts_ms + duration_ms);

        let mut segments = Vec::new();

        // Start Display Set: PCS + WDS + PDS + ODS(s) + END
        segments.extend(self.build_display_set(frame, pts, dts));

        // End Display Set at end time
        segments.push(Segment {
            segment_type: SegmentType::End,
            pts: pts_end,
            dts: pts_end,
            payload: SegmentPayload::End,
        });

        self.composition_number = self.composition_number.wrapping_add(1);
        self.object_id = self.object_id.wrapping_add(1);

        segments
    }

    pub fn encode_frame_to_bytes(&mut self, frame: &QuantizedFrame, pts_ms: u64, duration_ms: u64) -> Vec<u8> {
        let segments = self.encode_frame(frame, pts_ms, duration_ms);
        let mut output = Vec::new();
        for seg in &segments {
            output.extend(seg.to_bytes());
        }
        output
    }

    pub fn build_display_set(&self, frame: &QuantizedFrame, pts: u64, dts: u64) -> Vec<Segment> {
        let mut segments = Vec::new();

        let obj_x = ((self.display_width as i32 - frame.width as i32) / 2).max(0) as u16;
        let obj_y = (self.display_height as i32 - frame.height as i32 - 20).max(0) as u16;

        // PCS
        segments.push(Segment {
            segment_type: SegmentType::Pcs,
            pts, dts,
            payload: SegmentPayload::Pcs(PcsPayload {
                width: self.display_width,
                height: self.display_height,
                frame_rate: self.frame_rate,
                composition_number: self.composition_number,
                composition_state: CompositionState::EpochStart,
                palette_update: false,
                palette_id: self.palette_id,
                num_objects: 1,
                compositions: vec![ObjectComposition {
                    object_id: self.object_id,
                    window_id: self.window_id,
                    cropped: false,
                    forced: false,
                    x: obj_x, y: obj_y,
                    crop_x: 0, crop_y: 0,
                    crop_w: 0, crop_h: 0,
                }],
            }),
        });

        // WDS
        segments.push(Segment {
            segment_type: SegmentType::Wds,
            pts, dts,
            payload: SegmentPayload::Wds(WdsPayload {
                num_windows: 1,
                windows: vec![WindowDef {
                    window_id: self.window_id,
                    x: obj_x, y: obj_y,
                    width: frame.width as u16,
                    height: frame.height as u16,
                }],
            }),
        });

        // PDS
        let palette_entries = build_palette(&frame.palette);
        segments.push(Segment {
            segment_type: SegmentType::Pds,
            pts, dts,
            payload: SegmentPayload::Pds(PdsPayload {
                palette_id: self.palette_id,
                version: 0,
                entries: palette_entries,
            }),
        });

        // ODS(s)
        let rle = rle_encode(&frame.indices, frame.width, frame.height);
        let chunks = chunk_rle_data(&rle, MAX_ODS_CHUNK);
        for (i, chunk) in chunks.iter().enumerate() {
            segments.push(Segment {
                segment_type: SegmentType::Ods,
                pts, dts,
                payload: SegmentPayload::Ods(OdsPayload {
                    object_id: self.object_id,
                    object_version: 0,
                    last_in_sequence: i == chunks.len() - 1,
                    width: frame.width as u16,
                    height: frame.height as u16,
                    rle_data: chunk.clone(),
                }),
            });
        }

        segments
    }
}

pub fn ms_to_90khz(ms: u64) -> u64 {
    ms * 90
}

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

pub fn frame_rate_code(fps: f64) -> u8 {
    if fps <= 24.0 {
        0x10
    } else if fps <= 25.0 {
        0x20
    } else if fps <= 30.0 {
        0x40
    } else if fps <= 50.0 {
        0x50
    } else if fps <= 60.0 {
        0x70
    } else {
        0x10
    }
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
        // PCS + WDS + PDS + ODS + END = 5 segments
        assert_eq!(segments.len(), 5);
        assert_eq!(segments[0].segment_type, SegmentType::Pcs);
        assert_eq!(segments[1].segment_type, SegmentType::Wds);
        assert_eq!(segments[2].segment_type, SegmentType::Pds);
        assert_eq!(segments[3].segment_type, SegmentType::Ods);
        assert_eq!(segments[4].segment_type, SegmentType::End);
    }

    #[test]
    fn test_encode_frame_pts() {
        let mut enc = PgsEncoder::new(1920, 1080, 24.0);
        let frame = make_test_frame();
        let segments = enc.encode_frame(&frame, 1000, 2000);
        assert_eq!(segments[0].pts, 90000);
        assert_eq!(segments[4].pts, 270000);
    }

    #[test]
    fn test_encode_frame_increments_ids() {
        let mut enc = PgsEncoder::new(1920, 1080, 24.0);
        let frame = make_test_frame();
        enc.encode_frame(&frame, 0, 1000);
        assert_eq!(enc.composition_number, 1);
        assert_eq!(enc.object_id, 1);
        enc.encode_frame(&frame, 1000, 1000);
        assert_eq!(enc.composition_number, 2);
        assert_eq!(enc.object_id, 2);
    }

    #[test]
    fn test_build_display_set() {
        let enc = PgsEncoder::new(1920, 1080, 24.0);
        let frame = make_test_frame();
        let segments = enc.build_display_set(&frame, 90000, 90000);
        assert_eq!(segments.len(), 4);
    }

    #[test]
    fn test_ms_to_90khz() {
        assert_eq!(ms_to_90khz(0), 0);
        assert_eq!(ms_to_90khz(1000), 90000);
        assert_eq!(ms_to_90khz(1), 90);
    }

    #[test]
    fn test_frame_rate_code() {
        assert_eq!(frame_rate_code(23.976), 0x10);
        assert_eq!(frame_rate_code(24.0), 0x10);
        assert_eq!(frame_rate_code(25.0), 0x20);
        assert_eq!(frame_rate_code(29.97), 0x40);
        assert_eq!(frame_rate_code(30.0), 0x40);
        assert_eq!(frame_rate_code(50.0), 0x50);
        assert_eq!(frame_rate_code(59.94), 0x70);
        assert_eq!(frame_rate_code(60.0), 0x70);
    }

    #[test]
    fn test_timecode_to_ms() {
        assert_eq!(timecode_to_ms("0:00:01.00"), Some(1000));
        assert_eq!(timecode_to_ms("1:30:00.00"), Some(5400000));
        assert_eq!(timecode_to_ms("0:00:00.50"), Some(500));
        assert_eq!(timecode_to_ms("invalid"), None);
    }

    #[test]
    fn test_encode_to_bytes() {
        let mut enc = PgsEncoder::new(1920, 1080, 24.0);
        let frame = make_test_frame();
        let bytes = enc.encode_frame_to_bytes(&frame, 1000, 2000);
        assert!(!bytes.is_empty());
        // First two bytes should be "PG" magic
        assert_eq!(bytes[0], b'P');
        assert_eq!(bytes[1], b'G');
    }

    #[test]
    fn test_pcs_to_bytes() {
        let enc = PgsEncoder::new(1920, 1080, 24.0);
        let frame = make_test_frame();
        let segments = enc.build_display_set(&frame, 90000, 90000);
        let pcs_bytes = segments[0].to_bytes();
        assert_eq!(pcs_bytes[0], b'P');
        assert_eq!(pcs_bytes[1], b'G');
        assert_eq!(pcs_bytes[10], SegmentType::Pcs as u8);
    }

    #[test]
    fn test_full_encode_two_frames() {
        let mut enc = PgsEncoder::new(1920, 1080, 24.0);
        let frame = make_test_frame();
        let bytes1 = enc.encode_frame_to_bytes(&frame, 0, 2000);
        let bytes2 = enc.encode_frame_to_bytes(&frame, 2000, 2000);
        assert!(!bytes1.is_empty());
        assert!(!bytes2.is_empty());
    }
}
