//! Windowed display sets: single-window (EpochStart) and
//! top/bottom-split multi-window builders plus the split-row search.
use super::{DisplaySetConfig, MAX_ODS_CHUNK};
use crate::domain::composition::{CompositionState, ObjectComposition, WindowDef};
use crate::domain::palette::PaletteEntry;
use crate::domain::rle::{chunk_rle_data, rle_encode};
use crate::domain::segment::{
    OdsPayload, PcsPayload, PdsPayload, Segment, SegmentPayload, SegmentType, WdsPayload,
};
#[allow(clippy::too_many_arguments)]
pub fn build_single_window_display_set(
    config: &DisplaySetConfig,
    frame: &color_quantizer::QuantizedFrame,
    pts: u64,
    dts: u64,
    palette_entries: &[PaletteEntry],
    rle: &[u8],
    composition_state: CompositionState,
    palette_update: bool,
    frame_count: u32,
    object_version: u8,
) -> Vec<Segment> {
    let mut segments = Vec::new();
    // Propagate cropped bitmap origin to PCS object position.
    // Without this, decoders that honor PCS x/y (not WDS) render at (0,0).
    let obj_x = frame.x;
    let obj_y = frame.y;

    segments.push(Segment {
        segment_type: SegmentType::Pcs,
        pts,
        dts,
        payload: SegmentPayload::Pcs(PcsPayload {
            width: config.display_width,
            height: config.display_height,
            frame_rate: config.frame_rate,
            composition_number: config.composition_number,
            composition_state,
            palette_update,
            palette_id: config.palette_id,
            num_objects: 1,
            compositions: vec![ObjectComposition {
                object_id: config.object_id,
                window_id: config.window_id,
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

    let win_x = obj_x.min(config.display_width.saturating_sub(1));
    let win_y = obj_y.min(config.display_height.saturating_sub(1));
    let win_w = (frame.width as u16).min(config.display_width.saturating_sub(win_x));
    let win_h = (frame.height as u16).min(config.display_height.saturating_sub(win_y));

    segments.push(Segment {
        segment_type: SegmentType::Wds,
        pts,
        dts,
        payload: SegmentPayload::Wds(WdsPayload {
            num_windows: 1,
            windows: vec![WindowDef {
                window_id: config.window_id,
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
            palette_id: config.palette_id,
            version: frame_count as u8,
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
                object_id: config.object_id,
                object_version,
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

/// Find the optimal transparent row to split the frame for multi-window display.
pub fn find_split_row(indices: &[u8], width: u32, height: u32, transparent_index: u8) -> u32 {
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

#[allow(clippy::too_many_arguments)]
/// Build an EpochStart display set with two windows (top/bottom split).
pub fn build_multi_window_display_set(
    config: &DisplaySetConfig,
    frame: &color_quantizer::QuantizedFrame,
    pts: u64,
    dts: u64,
    palette_entries: &[PaletteEntry],
    composition_state: CompositionState,
    palette_update: bool,
    frame_count: u32,
    object_version: u8,
) -> Vec<Segment> {
    let split_row = find_split_row(
        &frame.indices,
        frame.width,
        frame.height,
        frame.transparent_index,
    );
    let top_height = split_row as u16;
    let bottom_height = (frame.height as u16).saturating_sub(top_height);
    let mut segments = Vec::new();
    let obj1_y = frame.y;
    let obj2_y = obj1_y + top_height;
    let x_offset = frame
        .x
        .min(config.display_width.saturating_sub(frame.width as u16));

    segments.push(Segment {
        segment_type: SegmentType::Pcs,
        pts,
        dts,
        payload: SegmentPayload::Pcs(PcsPayload {
            width: config.display_width,
            height: config.display_height,
            frame_rate: config.frame_rate,
            composition_number: config.composition_number,
            composition_state,
            palette_update,
            palette_id: config.palette_id,
            num_objects: 2,
            compositions: vec![
                ObjectComposition {
                    object_id: config.object_id,
                    window_id: 0,
                    cropped: false,
                    forced: false,
                    x: x_offset,
                    y: obj1_y,
                    crop_x: 0,
                    crop_y: 0,
                    crop_w: 0,
                    crop_h: 0,
                },
                ObjectComposition {
                    object_id: config.object_id + 1,
                    window_id: 1,
                    cropped: false,
                    forced: false,
                    x: x_offset,
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
                    x: x_offset,
                    y: obj1_y,
                    width: frame.width as u16,
                    height: top_height,
                },
                WindowDef {
                    window_id: 1,
                    x: x_offset,
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
            palette_id: config.palette_id,
            version: frame_count as u8,
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

    for (obj_idx, (obj_rle, obj_id)) in [
        (rle_top, config.object_id),
        (rle_bottom, config.object_id + 1),
    ]
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
                    object_version,
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
