use crate::domain::composition::{CompositionState, ObjectComposition, WindowDef};
use crate::domain::palette::PaletteEntry;
use crate::domain::rle::{chunk_rle_data, rle_encode};
use crate::domain::segment::{
    OdsPayload, PcsPayload, PdsPayload, Segment, SegmentPayload, SegmentType, WdsPayload,
};

const MAX_ODS_CHUNK: usize = 0xFFE0;

pub struct DisplaySetConfig {
    pub display_width: u16,
    pub display_height: u16,
    pub frame_rate: u8,
    pub composition_number: u16,
    pub object_id: u16,
    pub palette_id: u8,
    pub window_id: u8,
    pub potplayer_compat: bool,
}

impl DisplaySetConfig {
    pub fn palette_clear_num_objects(&self) -> u8 {
        1
    }
}

pub fn prepare_rle_and_hash(
    palette_entries: &mut [PaletteEntry],
    indices: &[u8],
    width: u32,
    height: u32,
    transparent_index: u8,
) -> (Vec<u8>, u64) {
    let ti = transparent_index;
    if ti != 0 && (ti as usize) < palette_entries.len() {
        palette_entries.swap(0, ti as usize);
        let mut swapped_indices = indices.to_vec();
        for idx in swapped_indices.iter_mut() {
            if *idx == 0 {
                *idx = ti;
            } else if *idx == ti {
                *idx = 0;
            }
        }
        let rle = rle_encode(&swapped_indices, width, height, 0);
        let rle_hash = crate::domain::epoch::hash_bytes(&rle);
        (rle, rle_hash)
    } else {
        let rle = rle_encode(indices, width, height, transparent_index);
        let rle_hash = crate::domain::epoch::hash_bytes(&rle);
        (rle, rle_hash)
    }
}

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
        let band_segments = build_single_window_display_set(
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
