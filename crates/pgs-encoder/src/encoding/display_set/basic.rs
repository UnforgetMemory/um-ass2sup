//! Basic display sets: palette_clear, EpochContinue, and palette-only
//! (fade) sets — the common kinds emitted for most frames.
use super::DisplaySetConfig;
use crate::domain::composition::{CompositionState, ObjectComposition};
use crate::domain::palette::PaletteEntry;
use crate::domain::segment::{PcsPayload, PdsPayload, Segment, SegmentPayload, SegmentType};
pub fn build_palette_clear_display_set(
    config: &DisplaySetConfig,
    pts: u64,
    dts: u64,
    frame_count: u32,
) -> Vec<Segment> {
    let num_objects = config.palette_clear_num_objects();
    let pcs = PcsPayload {
        width: config.display_width,
        height: config.display_height,
        frame_rate: config.frame_rate,
        composition_number: config.composition_number,
        composition_state: CompositionState::NormalCase,
        palette_update: true,
        palette_id: config.palette_id,
        num_objects,
        compositions: vec![ObjectComposition {
            object_id: config.object_id,
            window_id: config.window_id,
            cropped: false,
            forced: false,
            x: 0,
            y: 0,
            crop_x: 0,
            crop_y: 0,
            crop_w: config.display_width,
            crop_h: config.display_height,
        }],
    };
    let transparent_entries: Vec<PaletteEntry> = (0..=255u8)
        .map(|i| PaletteEntry {
            index: i,
            y: 0,
            cb: 128,
            cr: 128,
            alpha: 0,
        })
        .collect();
    let pds = PdsPayload {
        palette_id: config.palette_id,
        version: frame_count as u8,
        entries: transparent_entries,
    };
    vec![
        Segment {
            segment_type: SegmentType::Pcs,
            pts,
            dts,
            payload: SegmentPayload::Pcs(pcs),
        },
        Segment {
            segment_type: SegmentType::Pds,
            pts,
            dts,
            payload: SegmentPayload::Pds(pds),
        },
    ]
}

/// Build an EpochContinue display set when palette is unchanged.
pub fn build_continue_display_set(
    config: &DisplaySetConfig,
    frame: &color_quantizer::QuantizedFrame,
    pts: u64,
    dts: u64,
    composition_state: CompositionState,
    palette_entries: &[PaletteEntry],
    frame_count: u32,
) -> Vec<Segment> {
    vec![
        Segment {
            segment_type: SegmentType::Pcs,
            pts,
            dts,
            payload: SegmentPayload::Pcs(PcsPayload {
                width: config.display_width,
                height: config.display_height,
                frame_rate: config.frame_rate,
                composition_number: config.composition_number,
                composition_state,
                palette_update: true, // PotPlayer requires this on all PCS
                palette_id: config.palette_id,
                num_objects: 1,
                compositions: vec![ObjectComposition {
                    object_id: config.object_id,
                    window_id: config.window_id,
                    cropped: false,
                    forced: false,
                    x: frame.x,
                    y: frame.y,
                    crop_x: 0,
                    crop_y: 0,
                    crop_w: 0,
                    crop_h: 0,
                }],
            }),
        },
        // PotPlayer requires PDS to follow when palette_update=true.
        Segment {
            segment_type: SegmentType::Pds,
            pts,
            dts,
            payload: SegmentPayload::Pds(PdsPayload {
                palette_id: config.palette_id,
                version: frame_count as u8,
                entries: palette_entries.to_vec(),
            }),
        },
    ]
}

/// Build a palette-only display set for fade animation (no ODS).
pub fn build_palette_only_display_set(
    config: &DisplaySetConfig,
    frame: &color_quantizer::QuantizedFrame,
    pts: u64,
    dts: u64,
    palette_update: bool,
    palette_entries: &[PaletteEntry],
    frame_count: u32,
) -> Vec<Segment> {
    vec![
        Segment {
            segment_type: SegmentType::Pcs,
            pts,
            dts,
            payload: SegmentPayload::Pcs(PcsPayload {
                width: config.display_width,
                height: config.display_height,
                frame_rate: config.frame_rate,
                composition_number: config.composition_number,
                composition_state: CompositionState::NormalCase,
                palette_update,
                palette_id: config.palette_id,
                num_objects: 1,
                compositions: vec![ObjectComposition {
                    object_id: config.object_id,
                    window_id: config.window_id,
                    cropped: false,
                    forced: false,
                    x: frame.x,
                    y: frame.y,
                    crop_x: 0,
                    crop_y: 0,
                    crop_w: 0,
                    crop_h: 0,
                }],
            }),
        },
        Segment {
            segment_type: SegmentType::Pds,
            pts,
            dts,
            payload: SegmentPayload::Pds(PdsPayload {
                palette_id: config.palette_id,
                version: frame_count as u8,
                entries: palette_entries.to_vec(),
            }),
        },
    ]
}
