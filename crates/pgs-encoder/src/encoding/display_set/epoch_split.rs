//! Epoch-split display set (3-band) for frames exceeding the decoder
//! buffer limit, keeping PotPlayer compatibility.
use super::DisplaySetConfig;
use crate::domain::composition::CompositionState;
use crate::domain::segment::Segment;
#[allow(clippy::too_many_arguments)]
pub fn build_epoch_split_display_set(
    config: &DisplaySetConfig,
    frame: &color_quantizer::QuantizedFrame,
    pts: u64,
    dts: u64,
    composition_state: CompositionState,
    palette_update: bool,
    frame_count: u32,
    object_version: u8,
) -> Vec<Segment> {
    use crate::domain::palette::build_palette;
    use crate::domain::rle::rle_encode;
    let palette_entries = build_palette(&frame.palette, frame.color_space);
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
        // Propagate original frame origin + band vertical offset.
        // Without this, all bands render at (0,0) — losing subtitle position.
        let band_frame = color_quantizer::QuantizedFrame {
            width: frame.width,
            height: band_h,
            palette: frame.palette.clone(),
            indices: band_indices.to_vec(),
            transparent_index: frame.transparent_index,
            x: frame.x,
            y: frame.y.saturating_add(y_start as u16),
            color_space: frame.color_space,
            pts_ms: frame.pts_ms,
            duration_ms: frame.duration_ms,
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
        let band_segments = super::window::build_single_window_display_set(
            config,
            &band_frame,
            pts,
            dts,
            &palette_entries,
            &band_rle,
            band_state,
            palette_update,
            frame_count,
            object_version,
        );
        all_segments.extend(band_segments);
    }
    all_segments
}
