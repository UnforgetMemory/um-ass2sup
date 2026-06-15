use crate::color::build_palette;
use crate::rle::{chunk_rle_data, rle_encode};
use crate::types::*;
use color_quantizer::QuantizedFrame;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

const MAX_ODS_CHUNK: usize = 0xFFE0;
const MAX_DECODE_BUFFER: usize = 2 * 1024 * 1024; // ~2MB PGS decoder buffer limit

/// PGS/SUP binary encoder for Blu-ray subtitle streams.
///
/// Encodes [`QuantizedFrame`]s into PGS segments (PCS/WDS/PDS/ODS/END) following
/// the Blu-ray Disc Presentation Graphics specification. Supports:
///
/// - NTSC-aware PTS timing (23.976/29.97/59.94 fps)
/// - Multi-window mode for large frames
/// - Epoch splitting for decoder buffer safety (~2MB limit)
/// - Palette reuse detection between frames
pub struct PgsEncoder {
    composition_number: u16,
    object_id: u16,
    palette_id: u8,
    window_id: u8,
    frame_rate: u8,
    display_width: u16,
    display_height: u16,
    fps: f64,
    prev_palette_hash: Option<u64>,
    prev_object_rle_hash: Option<u64>,
    frame_count: u32,
    /// ODS object version counter — incremented when object content changes.
    object_version: u8,
}

impl PgsEncoder {
    /// Create a new PGS encoder with display parameters.
    ///
    /// # Arguments
    /// * `display_width` - Display width in pixels (e.g. 1920)
    /// * `display_height` - Display height in pixels (e.g. 1080)
    /// * `fps` - Frame rate (e.g. 23.976, 25.0, 29.97)
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
            prev_palette_hash: None,
            prev_object_rle_hash: None,
            frame_count: 0,
            object_version: 0,
        }
    }

    /// Convert milliseconds to PTS ticks at 90kHz.
    ///
    /// Uses NTSC-correct formula (`ms * 90000 * 1001 / 1000000`) for
    /// 23.976/29.97/59.94 fps, simple `ms * 90` otherwise.
    pub fn ms_to_90khz(&self, ms: u64) -> u64 {
        if is_ntsc_fps(self.fps) {
            (u128::from(ms) * 90000 * 1001 / 1000000) as u64
        } else {
            ms * 90
        }
    }

    /// Encode a quantized frame into PGS segments.
    ///
    /// Produces a full display set (PCS+WDS+PDS+ODS+END) for the given frame.
    /// Returns a list of segments that can be serialized with [`Segment::to_bytes`].
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

        // NOTE: We do NOT emit an empty display set (num_objects=0) to clear subtitles.
        // Some players (including PotPlayer) crash on empty display sets.
        // PGS standard behavior: subtitle stays visible until the next display set
        // replaces it. This means subtitles "extend" until the next event starts.

        segments.push(Segment {
            segment_type: SegmentType::End,
            pts: pts_end,
            dts: pts_end,
            payload: SegmentPayload::End,
        });

                self.composition_number = self.composition_number.wrapping_add(1);
        // Object ID: keep at 0 for BDSup2Sub compatibility within each display set.
        // BDSup2Sub uses object_id as an index into imageObjectList per display set.
        // Each display set is independent, so object_id=0 is correct for all frames.
        // self.object_id = self.object_id.wrapping_add(1);
        self.object_version = self.object_version.wrapping_add(1);
        self.frame_count += 1;

        segments
    }

    /// Encode a quantized frame directly to SUP binary bytes.
    ///
    /// Convenience wrapper around [`encode_frame`](Self::encode_frame) that serializes all segments
    /// to bytes in one call.
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
        // The new RLE encoder uses FFmpeg-compatible format where non-zero bytes
        let mut palette_entries = build_palette(&frame.palette, self.display_height);

        // Transparent color is at palette index 0 (from quantizer fix).
        // The RLE encoder uses index 0 for transparent pixels.
        // Swap palette entries if needed to ensure index 0 = transparent color.
        let ti = frame.transparent_index;
        if ti != 0 && (ti as usize) < palette_entries.len() {
            palette_entries.swap(0, ti as usize);
        }
        let palette_hash = hash_palette(&palette_entries);

        let rle = rle_encode(
            &frame.indices,
            frame.width,
            frame.height,
            frame.transparent_index,
        );
        let rle_hash = hash_bytes(&rle);

        let palette_changed = self.prev_palette_hash != Some(palette_hash);
        let object_changed = self.prev_object_rle_hash != Some(rle_hash);

        // PGS spec: first display set uses EpochStart, subsequent use NormalCase.
        // EpochStart clears the screen and starts a new epoch; NormalCase continues it.
        // PotPlayer requires EpochStart on every frame to properly process ODS data.
        let composition_state = CompositionState::EpochStart;

        // palette_update=true: tells the player that the PDS palette is present.
        // PotPlayer requires this flag to load the PDS palette. Without it,
        // PotPlayer skips PDS processing and renders garbage or crashes.
        // BDSup2Sub ignores this flag for ODS processing.
        let palette_update = true;

        let rle_size_est = 13 + 4 + rle.len();
        let use_multi_window = rle_size_est > MAX_DECODE_BUFFER / 2 && frame.height > 100;

        let segments = if use_multi_window {
            self.build_multi_window_display_set(
                frame,
                pts,
                dts,
                pts,
                &palette_entries,
                &rle,
                composition_state,
                palette_changed,
                palette_update,
            )
        } else {
            self.build_single_window_display_set(
                frame,
                pts,
                dts,
                &palette_entries,
                &rle,
                composition_state,
                palette_changed,
                palette_update,
            )
        };

        let total_size: usize = segments.iter().map(|s| s.to_bytes().len()).sum();
        let result = if total_size > MAX_DECODE_BUFFER * 3 / 4 {
            self.build_epoch_split_display_set(frame, pts, dts, composition_state, palette_changed, palette_update)
        } else {
            segments
        };
        self.prev_palette_hash = Some(palette_hash);
        self.prev_object_rle_hash = Some(rle_hash);
        result
    }

    fn build_epoch_split_display_set(
        &self,
        frame: &QuantizedFrame,
        pts: u64,
        dts: u64,
        composition_state: CompositionState,
        palette_changed: bool,
        palette_update: bool,
    ) -> Vec<Segment> {
        let palette_entries = build_palette(&frame.palette, self.display_height);
        let band_height = (frame.height / 3).max(64);
        let mut all_segments = Vec::new();

        for band_idx in 0..3u32 {
            let y_start = band_idx * band_height;
            let y_end = ((band_idx + 1) * band_height).min(frame.height);
            if y_start >= frame.height {
                break;
            }
            let band_h = y_end - y_start;
            let start_offset = (y_start * frame.width) as usize;
            let end_offset = (y_end * frame.width) as usize;
            let band_indices = &frame.indices[start_offset..end_offset];

            let band_frame = QuantizedFrame {
                width: frame.width,
                height: band_h,
                palette: frame.palette.clone(),
                indices: band_indices.to_vec(),
                transparent_index: frame.transparent_index,
            };

            let band_rle = rle_encode(
                &band_frame.indices,
                band_frame.width,
                band_frame.height,
                band_frame.transparent_index,
            );
            let band_state = if band_idx == 0 {
                composition_state
            } else {
                CompositionState::NormalCase
            };

            let band_segments = self.build_single_window_display_set(
                &band_frame,
                pts,
                dts,
                &palette_entries,
                &band_rle,
                band_state,
                palette_changed,
                palette_update,
            );
            all_segments.extend(band_segments);
        }

        all_segments
    }

    /// Build a display set with zero objects to clear the subtitle from screen.
    ///
    /// PGS has no explicit "end time" — a subtitle persists until replaced.
    /// An empty PCS with `num_objects=0` tells the player to clear the display.
    fn build_clear_display_set(
        &self,
        pts: u64,
        dts: u64,
    ) -> Vec<Segment> {
        let pcs = PcsPayload {
            width: self.display_width,
            height: self.display_height,
            frame_rate: self.frame_rate,
            composition_number: self.composition_number.wrapping_add(1),
            composition_state: CompositionState::NormalCase,
            palette_update: false,
            palette_id: self.palette_id,
            num_objects: 0,
            compositions: vec![],
        };

        vec![Segment {
            segment_type: SegmentType::Pcs,
            pts,
            dts,
            payload: SegmentPayload::Pcs(pcs),
        }]
    }

    #[allow(clippy::too_many_arguments)]
    fn build_single_window_display_set(
        &self,
        frame: &QuantizedFrame,
        pts: u64,
        dts: u64,
        palette_entries: &[PaletteEntry],
        rle: &[u8],
        composition_state: CompositionState,
        palette_changed: bool,
        palette_update: bool,
    ) -> Vec<Segment> {
        let mut segments = Vec::new();

        let obj_x = ((i32::from(self.display_width) - frame.width as i32) / 2).max(0) as u16;
        let obj_y = (i32::from(self.display_height) - frame.height as i32 - 20).max(0) as u16;

        segments.push(Segment {
            segment_type: SegmentType::Pcs,
            pts,
            dts,
            payload: SegmentPayload::Pcs(PcsPayload {
                width: self.display_width,
                height: self.display_height,
                frame_rate: self.frame_rate,
                composition_number: self.composition_number,
                composition_state,
                palette_update,
                palette_id: self.palette_id,
                num_objects: 1,
                compositions: vec![ObjectComposition {
                    object_id: self.object_id,
                    window_id: self.window_id,
                    cropped: false,
                    forced: false,
                    x: obj_x,
                    y: obj_y,
                    crop_x: 0,
                    crop_y: 0,
                    crop_w: 0,
                    crop_h: 0,
                }],
            }),
        });

        // Clamp window dimensions to display bounds
        let win_x = obj_x.min(self.display_width.saturating_sub(1));
        let win_y = obj_y.min(self.display_height.saturating_sub(1));
        let win_w = (frame.width as u16).min(self.display_width.saturating_sub(win_x));
        let win_h = (frame.height as u16).min(self.display_height.saturating_sub(win_y));

        segments.push(Segment {
            segment_type: SegmentType::Wds,
            pts,
            dts,
            payload: SegmentPayload::Wds(WdsPayload {
                num_windows: 1,
                windows: vec![WindowDef {
                    window_id: self.window_id,
                    x: win_x,
                    y: win_y,
                    width: win_w,
                    height: win_h,
                }],
            }),
        });

        segments.push(Segment {
            segment_type: SegmentType::Pds,
            pts,
            dts,
            payload: SegmentPayload::Pds(PdsPayload {
                palette_id: self.palette_id,
                version: self.frame_count as u8,
                entries: palette_entries.to_vec(),
            }),
        });

        let chunks = chunk_rle_data(rle, MAX_ODS_CHUNK);
        let total_rle_size = rle.len();
        for (i, chunk) in chunks.iter().enumerate() {
            segments.push(Segment {
                segment_type: SegmentType::Ods,
                pts,
                dts,
                payload: SegmentPayload::Ods(OdsPayload {
                    object_id: self.object_id,
                    object_version: self.object_version,
                    first_in_sequence: i == 0,
                    last_in_sequence: i == chunks.len() - 1,
                    width: frame.width as u16,
                    height: frame.height as u16,
                    rle_data: chunk.clone(),
                    total_rle_size,
                }),
            });
        }

        segments
    }

    #[allow(clippy::too_many_arguments)]
    fn build_multi_window_display_set(
        &self,
        frame: &QuantizedFrame,
        pts: u64,
        dts: u64,
        _pts_end: u64,
        palette_entries: &[PaletteEntry],
        _rle: &[u8],
        composition_state: CompositionState,
        palette_changed: bool,
        palette_update: bool,
    ) -> Vec<Segment> {
        let split_row = self.find_split_row(
            &frame.indices,
            frame.width,
            frame.height,
            frame.transparent_index,
        );
        let top_height = split_row as u16;
        let bottom_height = (frame.height as u16).saturating_sub(top_height);

        let mut segments = Vec::new();

        let obj1_y = (self.display_height - top_height) / 2;
        let obj2_y = obj1_y + top_height;

        segments.push(Segment {
            segment_type: SegmentType::Pcs,
            pts,
            dts,
            payload: SegmentPayload::Pcs(PcsPayload {
                width: self.display_width,
                height: self.display_height,
                frame_rate: self.frame_rate,
                composition_number: self.composition_number,
                composition_state,
                palette_update,
                palette_id: self.palette_id,
                num_objects: 2,
                compositions: vec![
                    ObjectComposition {
                        object_id: self.object_id,
                        window_id: 0,
                        cropped: false,
                        forced: false,
                        x: ((i32::from(self.display_width) - frame.width as i32) / 2).max(0) as u16,
                        y: obj1_y,
                        crop_x: 0,
                        crop_y: 0,
                        crop_w: 0,
                        crop_h: 0,
                    },
                    ObjectComposition {
                        object_id: self.object_id + 1,
                        window_id: 1,
                        cropped: false,
                        forced: false,
                        x: ((i32::from(self.display_width) - frame.width as i32) / 2).max(0) as u16,
                        y: obj2_y,
                        crop_x: 0,
                        crop_y: 0,
                        crop_w: 0,
                        crop_h: 0,
                    },
                ],
            }),
        });

        segments.push(Segment {
            segment_type: SegmentType::Wds,
            pts,
            dts,
            payload: SegmentPayload::Wds(WdsPayload {
                num_windows: 2,
                windows: vec![
                    WindowDef {
                        window_id: 0,
                        x: ((i32::from(self.display_width) - frame.width as i32) / 2).max(0) as u16,
                        y: obj1_y,
                        width: frame.width as u16,
                        height: top_height,
                    },
                    WindowDef {
                        window_id: 1,
                        x: ((i32::from(self.display_width) - frame.width as i32) / 2).max(0) as u16,
                        y: obj2_y,
                        width: frame.width as u16,
                        height: bottom_height,
                    },
                ],
            }),
        });

        segments.push(Segment {
            segment_type: SegmentType::Pds,
            pts,
            dts,
            payload: SegmentPayload::Pds(PdsPayload {
                palette_id: self.palette_id,
                version: self.frame_count as u8,
                entries: palette_entries.to_vec(),
            }),
        });

        let rle_top = rle_encode(
            &frame.indices[..(frame.width * split_row) as usize],
            frame.width,
            u32::from(top_height),
            frame.transparent_index,
        );
        let rle_bottom = rle_encode(
            &frame.indices[(frame.width * split_row) as usize..],
            frame.width,
            u32::from(bottom_height),
            frame.transparent_index,
        );

        for (obj_idx, (obj_rle, obj_id)) in
            [(rle_top, self.object_id), (rle_bottom, self.object_id + 1)]
                .iter()
                .enumerate()
        {
            let chunks = chunk_rle_data(obj_rle, MAX_ODS_CHUNK);
            let total_obj_rle = obj_rle.len();
            for (i, chunk) in chunks.iter().enumerate() {
                segments.push(Segment {
                    segment_type: SegmentType::Ods,
                    pts,
                    dts,
                    payload: SegmentPayload::Ods(OdsPayload {
                        object_id: *obj_id,
                        object_version: self.object_version,
                        first_in_sequence: i == 0,
                        last_in_sequence: i == chunks.len() - 1,
                        width: frame.width as u16,
                        height: if obj_idx == 0 {
                            top_height
                        } else {
                            bottom_height
                        },
                        rle_data: chunk.clone(),
                        total_rle_size: total_obj_rle,
                    }),
                });
            }
        }

        segments
    }

    fn find_split_row(
        &self,
        indices: &[u8],
        width: u32,
        height: u32,
        transparent_index: u8,
    ) -> u32 {
        let mid = height / 2;
        let mut best_row = mid;
        let mut best_score = 0u32;

        let search_start = (mid / 2).max(1);
        let search_end = height - (height / 4).max(1);

        for row in search_start..search_end {
            let offset = (row * width) as usize;
            let end = (offset + width as usize).min(indices.len());
            if end > indices.len() || offset >= indices.len() {
                continue;
            }
            let transparent_count = indices[offset..end]
                .iter()
                .filter(|&&c| c == transparent_index)
                .count() as u32;
            if transparent_count > best_score {
                best_score = transparent_count;
                best_row = row;
            }
        }

        best_row
    }
}

/// Convert milliseconds to 90kHz PTS ticks (simple, non-NTSC).
///
/// This is the standard conversion for integer frame rates (24, 25, 30, 50, 60).
/// For NTSC rates (23.976, 29.97, 59.94), use [`PgsEncoder::ms_to_90khz`] instead.
pub fn ms_to_90khz(ms: u64) -> u64 {
    ms * 90
}

/// Parse an ASS-style timecode string into milliseconds.
///
/// Expected format: `H:MM:SS.CS` (hours:minutes:seconds.centiseconds)
///
/// # Examples
/// ```
/// use pgs_encoder::timecode_to_ms;
/// assert_eq!(timecode_to_ms("0:00:01.00"), Some(1000));
/// assert_eq!(timecode_to_ms("1:30:00.00"), Some(5400000));
/// assert_eq!(timecode_to_ms("invalid"), None);
/// ```
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

/// Map a numeric FPS value to the PGS frame rate code byte.
///
/// PGS supports a discrete set of frame rates via a single code byte:
///
/// | Code  | FPS   |
/// |-------|-------|
/// | 0x10  | 24p   |
/// | 0x20  | 25p   |
/// | 0x40  | 30p   |
/// | 0x50  | 50p   |
/// | 0x70  | 60p   |
///
/// Values above 60 default to 24p (0x10).
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

/// Check if an FPS value requires NTSC-aware PTS calculation.
///
/// NTSC frame rates (23.976, 29.97, 59.94) use the exact formula
/// `ms * 90000 * 1001 / 1000000` instead of `ms * 90` to avoid
/// long-term PTS drift (~337ms/hour).
fn is_ntsc_fps(fps: f64) -> bool {
    (fps - 23.976).abs() < 0.01 || (fps - 29.97).abs() < 0.01 || (fps - 59.94).abs() < 0.01
}

/// Compute a hash of a palette for change detection.
///
/// Used internally to determine whether a palette update segment (PDS)
/// is needed between consecutive frames.
fn hash_palette(palette: &[PaletteEntry]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for entry in palette {
        entry.index.hash(&mut hasher);
        entry.y.hash(&mut hasher);
        entry.cb.hash(&mut hasher);
        entry.cr.hash(&mut hasher);
        entry.alpha.hash(&mut hasher);
    }
    hasher.finish()
}

/// Compute a hash of RLE data for object change detection.
///
/// Used internally to determine whether the object data has changed
/// between consecutive frames, enabling NormalCase composition.
fn hash_bytes(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

/// Remap indices in the collision range (0x40-0x7F) to safe values (0x80+).
///
/// The PGS RLE format uses transparent long-run headers `[0x40|len_hi, len_lo]`.
/// When an opaque color happens to be in 0x40-0x7F and the next byte has bit 7
/// set, the decoder's ambiguous-case handler tries transparent interpretation first,
/// misinterpreting opaque runs as transparent runs.
///
/// This function remaps any used collision-range index to an unused slot in 0x80-0xBF,
/// updating both the index array and the palette to match.
fn remap_collision_range(frame: &QuantizedFrame) -> (Vec<u8>, Vec<color_quantizer::Rgba>) {
    let mut indices = frame.indices.clone();
    let mut palette = frame.palette.clone();
    while palette.len() < 256 {
        palette.push(color_quantizer::Rgba::new(0, 0, 0, 0));
    }

    // Find which indices are actually used
    let mut used = [false; 256];
    for &idx in &indices {
        used[idx as usize] = true;
    }

    let mut remap = [0u8; 256];
    for (i, item) in remap.iter_mut().enumerate() {
        *item = i as u8;
    }

    let mut next_target: u8 = 0x80;
    for i in 0x40..0x80u8 {
        let i_usize = i as usize;
        if used[i_usize] && i != frame.transparent_index && i != 0 {
            while next_target < 0xC0 && used[next_target as usize] {
                next_target += 1;
            }
            if next_target < 0xC0 {
                remap[i_usize] = next_target;
                used[next_target as usize] = true;
                next_target += 1;
            }
        }
    }

    for idx in indices.iter_mut() {
        *idx = remap[*idx as usize];
    }

    (indices, palette)
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
        assert_eq!(segments[0].pts, 90000); // PCS at 1s
        assert_eq!(segments[4].pts, 270000); // END at 3s
    }

    #[test]
    fn test_encode_frame_increments_ids() {
        let mut enc = PgsEncoder::new(1920, 1080, 24.0);
        let frame = make_test_frame();
        enc.encode_frame(&frame, 0, 1000);
        assert_eq!(enc.composition_number, 1);
        // object_id stays at 0 for BDSup2Sub compatibility
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
        let pts_1s = enc.ms_to_90khz(1000);
        let expected_1s = (1000u128 * 90000 * 1001 / 1000000) as u64;
        assert_eq!(pts_1s, expected_1s);
        assert_eq!(pts_1s, 90090);
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
        let mut enc = PgsEncoder::new(1920, 1080, 24.0);
        let frame = make_test_frame();
        let segments = enc.encode_frame(&frame, 1000, 2000);
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
