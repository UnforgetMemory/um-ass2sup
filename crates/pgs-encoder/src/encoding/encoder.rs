use crate::domain::composition::CompositionState;
use crate::domain::epoch::{DisplaySetKind, EpochManager, hash_palette};
use crate::domain::palette::build_palette;
use crate::domain::segment::{Segment, SegmentPayload, SegmentType};
use crate::domain::timing::{frame_rate_code, is_ntsc_fps};
use crate::encoding::display_set as ds;
use color_quantizer::QuantizedFrame;

const MAX_DECODE_BUFFER: usize = 2 * 1024 * 1024;

/// PGS/SUP subtitle encoder. Converts quantized frames into PGS display sets.
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
            composition_number: 1,
            object_id: 0,
            palette_id: 0,
            window_id: 0,
            frame_rate: frame_rate_code(fps),
            display_width,
            display_height,
            fps,
            epoch: EpochManager::new().with_max_frames((60.0 * fps) as u32),
            potplayer_compat: true,
        }
    }

    /// Map milliseconds to 90 kHz PTS ticks, frame-accurate.
    ///
    /// Delegates to [`crate::domain::timing::frame_accurate_pts`] so this
    /// path is consistent with the CLI/libass production path (which calls
    /// `encode_frame_at_pts` with a pre-computed frame-accurate PTS).
    pub fn ms_to_90khz(&self, ms: u64) -> u64 {
        crate::domain::timing::frame_accurate_pts(ms, self.fps)
    }

    pub fn pts_at_frame(&self, first_pts: u64, frame_idx: u64) -> u64 {
        if is_ntsc_fps(self.fps) {
            // Round the 0.75-tick fraction instead of truncating, matching
            // `frame_accurate_pts`.
            first_pts + (frame_idx * 15015 + 2) / 4
        } else {
            let ticks = (90000.0 / self.fps) as u64;
            first_pts + frame_idx * ticks
        }
    }

    pub fn encode_frame_at_pts(
        &mut self,
        frame: &QuantizedFrame,
        pts: u64,
        _duration_ms: u64,
    ) -> Vec<Segment> {
        let dts = pts.saturating_sub(1);
        let mut segments = Vec::new();
        let (content_segments, _) = self.build_display_set(frame, pts, dts);
        segments.extend(content_segments);
        segments.push(Segment {
            segment_type: SegmentType::End,
            pts,
            dts,
            payload: SegmentPayload::End,
        });
        self.composition_number = self.composition_number.wrapping_add(1);
        segments
    }

    pub fn encode_frame(
        &mut self,
        frame: &QuantizedFrame,
        pts_ms: u64,
        duration_ms: u64,
    ) -> Vec<Segment> {
        let pts = self.ms_to_90khz(pts_ms);
        self.encode_frame_at_pts(frame, pts, duration_ms)
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
    ) -> (Vec<Segment>, DisplaySetKind) {
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
            DisplaySetKind::EpochContinue => ds::build_continue_display_set(
                cfg,
                frame,
                pts,
                dts,
                composition_state,
                &palette_entries,
                fc,
            ),
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

        let total_size: usize = segments.iter().map(|s| s.serialized_size()).sum();
        if total_size > MAX_DECODE_BUFFER * 3 / 4 {
            self.epoch.update(palette_hash, rle_hash);
            (
                ds::build_epoch_split_display_set(
                    cfg,
                    frame,
                    pts,
                    dts,
                    composition_state,
                    palette_update,
                    fc,
                    ov,
                ),
                kind,
            )
        } else {
            self.epoch.update(palette_hash, rle_hash);
            (segments, kind)
        }
    }

    pub fn emit_clear(&mut self, pts: u64) -> Vec<Segment> {
        let dts = pts.saturating_sub(1);
        let mut segs = ds::build_palette_clear_display_set(
            &self.make_config(),
            pts,
            dts,
            self.epoch.frame_count,
        );
        self.composition_number = self.composition_number.wrapping_add(1);
        segs.push(Segment {
            segment_type: SegmentType::End,
            pts,
            dts,
            payload: SegmentPayload::End,
        });
        segs
    }
}
