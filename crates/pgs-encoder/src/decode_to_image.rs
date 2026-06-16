use std::collections::HashMap;

use crate::color::ycbcr_to_rgba;
use crate::decoder::{DisplaySet, ParsedObjectComposition, ParsedPayload, ParsedSegment};
use crate::rle::rle_decode;
use crate::types::{CompositionState, PaletteEntry, WindowDef};

const MAX_ODS_BYTES: usize = 1920 * 1080;

pub struct FramePixels {
    pub width: u32,
    pub height: u32,
    pub data: Vec<u8>,
}

pub struct RenderContext {
    pub width: u32,
    pub height: u32,
    windows: HashMap<u8, WindowDef>,
    palette: HashMap<u8, PaletteEntry>,
    objects: HashMap<u16, ObjectData>,
    palette_id: u8,
    has_palette: bool,
    last_pcs_objects: Vec<ParsedObjectComposition>,
}

struct ObjectData {
    width: u16,
    height: u16,
    rle_data: Vec<u8>,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            windows: HashMap::new(),
            palette: HashMap::new(),
            objects: HashMap::new(),
            palette_id: 0,
            has_palette: false,
            last_pcs_objects: Vec::new(),
        }
    }
}

impl RenderContext {
    fn reset(&mut self) {
        self.windows.clear();
        self.palette.clear();
        self.objects.clear();
        self.has_palette = false;
        self.last_pcs_objects.clear();
    }
}

pub fn decode_frame_to_rgba(
    display_set: &DisplaySet,
    ctx: &mut RenderContext,
    transparent_index: u8,
) -> Result<FramePixels, DecodeImageError> {
    for segment in &display_set.segments {
        process_segment(segment, ctx)?;
    }

    let (width, height) = (ctx.width, ctx.height);
    let total = width
        .checked_mul(height)
        .and_then(|v| v.checked_mul(4))
        .ok_or_else(|| DecodeImageError::InvalidDimensions(format!("{width}x{height} overflow")))?;
    let mut rgba = vec![0u8; total as usize];
    composite_objects(ctx, &mut rgba, width, height, transparent_index)?;

    Ok(FramePixels {
        width,
        height,
        data: rgba,
    })
}

fn process_segment(
    segment: &ParsedSegment,
    ctx: &mut RenderContext,
) -> Result<(), DecodeImageError> {
    match &segment.payload {
        ParsedPayload::WindowDefinition { windows } => {
            for w in windows {
                ctx.windows.insert(w.window_id, w.clone());
            }
        }

        ParsedPayload::PaletteDefinition {
            palette_id,
            entries,
            ..
        } => {
            ctx.palette_id = *palette_id;
            ctx.has_palette = true;
            for entry in entries {
                ctx.palette.insert(entry.index, *entry);
            }
        }

        ParsedPayload::ObjectDefinition {
            object_id,
            width,
            height,
            first_in_sequence,
            data,
            ..
        } => {
            let obj = ctx.objects.entry(*object_id).or_insert_with(|| ObjectData {
                width: *width,
                height: *height,
                rle_data: Vec::new(),
            });
            // Only update stored dimensions from first-in-sequence ODS segments;
            // continuation segments have width=0, height=0.
            if *first_in_sequence && *width > 0 {
                obj.width = *width;
                obj.height = *height;
            }
            let new_len = obj.rle_data.len() + data.len();
            if new_len > MAX_ODS_BYTES {
                return Err(DecodeImageError::InvalidDimensions(format!(
                    "ODS data exceeds maximum: {new_len} > {MAX_ODS_BYTES}"
                )));
            }
            obj.rle_data.extend_from_slice(data);
        }

        ParsedPayload::PresentationComposition {
            width,
            height,
            state,
            palette_update,
            palette_id,
            objects,
            ..
        } => {
            ctx.width = *width as u32;
            ctx.height = *height as u32;

            match state {
                CompositionState::EpochStart => {
                    ctx.reset();
                }
                CompositionState::AcquirePoint => {
                    ctx.windows.clear();
                    ctx.objects.clear();
                    ctx.last_pcs_objects.clear();
                }
                CompositionState::NormalCase => {}
            }

            ctx.palette_id = *palette_id;

            if !*palette_update {
                ctx.palette.retain(|_, _| false);
                ctx.has_palette = false;
            }

            ctx.last_pcs_objects = objects.clone();
        }

        ParsedPayload::End => {}
    }
    Ok(())
}

fn composite_objects(
    ctx: &mut RenderContext,
    rgba: &mut [u8],
    canvas_w: u32,
    canvas_h: u32,
    transparent_index: u8,
) -> Result<(), DecodeImageError> {
    if ctx.last_pcs_objects.is_empty() {
        return Ok(());
    }

    if !ctx.has_palette {
        return Err(DecodeImageError::NoPalette);
    }

    // When transparent_index != 0, the encoder swaps colors (0 ↔ transparent_index)
    // before RLE encoding with enc_transparent=0. The RLE decode produces:
    //   - Old-opaque pixels → index 0
    //   - Old-transparent pixels → index transparent_index
    // But the SUP palette uses ORIGINAL indices. We must swap palette entries
    // 0 and transparent_index to match the RLE's index space.
    if transparent_index != 0 {
        if let (Some(zero_entry), Some(ti_entry)) = (
            ctx.palette.get(&0).cloned(),
            ctx.palette.get(&transparent_index).cloned(),
        ) {
            ctx.palette.insert(0, ti_entry);
            ctx.palette.insert(transparent_index, zero_entry);
        }
    }

    for obj_comp in &ctx.last_pcs_objects {
        let Some(obj_data) = ctx.objects.get(&obj_comp.object_id) else {
            continue;
        };

        let window = ctx
            .windows
            .get(&obj_comp.window_id)
            .cloned()
            .unwrap_or(WindowDef {
                window_id: 0,
                x: 0,
                y: 0,
                width: canvas_w as u16,
                height: canvas_h as u16,
            });

        let rle_data = &obj_data.rle_data;

        // The encoder always uses enc_transparent=0 (swapping colors when transparent_index != 0).
        // So the RLE data always treats index 0 as the transparent format marker.
        // We must pass 0 to rle_decode regardless of the actual transparent_index.
        let palette_indices = rle_decode(
            rle_data,
            u32::from(obj_data.width),
            u32::from(obj_data.height),
            0,
        )
        .map_err(DecodeImageError::RleDecodeFailed)?;

        let obj_w = u32::from(obj_data.width);
        let obj_h = u32::from(obj_data.height);
        let obj_total = obj_w
            .checked_mul(obj_h)
            .and_then(|v| v.checked_mul(4))
            .ok_or_else(|| {
                DecodeImageError::InvalidDimensions(format!("object {obj_w}x{obj_h} overflow"))
            })?;
        let mut obj_rgba = vec![0u8; obj_total as usize];

        for (i, &idx) in palette_indices.iter().enumerate() {
            let rgba_color = if let Some(entry) = ctx.palette.get(&idx) {
                ycbcr_to_rgba(entry.y, entry.cb, entry.cr, entry.alpha)
            } else {
                [0, 0, 0, 0]
            };
            let offset = i * 4;
            obj_rgba[offset..offset + 4].copy_from_slice(&rgba_color);
        }

        let abs_x = u32::from(window.x).saturating_add(u32::from(obj_comp.x));
        let abs_y = u32::from(window.y).saturating_add(u32::from(obj_comp.y));

        let x0 = abs_x.min(canvas_w);
        let y0 = abs_y.min(canvas_h);
        let x1 = (abs_x + obj_w).min(canvas_w);
        let y1 = (abs_y + obj_h).min(canvas_h);

        let blit_w = x1.saturating_sub(x0);
        let blit_h = y1.saturating_sub(y0);

        if blit_w == 0 || blit_h == 0 {
            continue;
        }

        let src_x = if abs_x > canvas_w { obj_w } else { 0 };
        let src_y = if abs_y > canvas_h { obj_h } else { 0 };

        for row in 0..blit_h {
            let dst_start = ((y0 + row) * canvas_w + x0) as usize * 4;
            let src_row = src_y + row;
            let src_start = (src_row * obj_w + src_x) as usize * 4;

            let dst_pixels = &mut rgba[dst_start..dst_start + (blit_w as usize) * 4];
            let src_pixels = &obj_rgba[src_start..src_start + (blit_w as usize) * 4];

            for (dp, sp) in dst_pixels
                .chunks_exact_mut(4)
                .zip(src_pixels.chunks_exact(4))
            {
                let src_a = sp[3];
                if src_a == 255 {
                    dp.copy_from_slice(sp);
                } else if src_a > 0 {
                    let src_a_f = src_a as f32 / 255.0;
                    let dst_a_f = 1.0 - src_a_f;
                    dp[0] = ((sp[0] as f32 * src_a_f) + (dp[0] as f32 * dst_a_f)) as u8;
                    dp[1] = ((sp[1] as f32 * src_a_f) + (dp[1] as f32 * dst_a_f)) as u8;
                    dp[2] = ((sp[2] as f32 * src_a_f) + (dp[2] as f32 * dst_a_f)) as u8;
                    dp[3] = (255.0 * src_a_f + dp[3] as f32 * dst_a_f) as u8;
                }
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
pub enum DecodeImageError {
    NoPcs,
    ObjectNotFound(u16),
    RleDecodeFailed(String),
    NoPalette,
    OutOfBounds,
    InvalidDimensions(String),
}

impl std::fmt::Display for DecodeImageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoPcs => write!(f, "no PCS segment in display set"),
            Self::ObjectNotFound(id) => write!(f, "object {id} not found in ODS data"),
            Self::RleDecodeFailed(msg) => write!(f, "RLE decode failed: {msg}"),
            Self::NoPalette => write!(f, "no palette available"),
            Self::OutOfBounds => write!(f, "object position out of bounds"),
            Self::InvalidDimensions(msg) => write!(f, "invalid dimensions: {msg}"),
        }
    }
}

impl std::error::Error for DecodeImageError {}

pub fn frame_to_png(frame: &FramePixels) -> Result<Vec<u8>, PngEncodeError> {
    let mut buf = std::io::Cursor::new(Vec::new());
    let mut encoder = png::Encoder::new(&mut buf, frame.width, frame.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(|e| PngEncodeError::Header(e.to_string()))?
        .write_image_data(&frame.data)
        .map_err(|e| PngEncodeError::Encode(e.to_string()))?;
    Ok(buf.into_inner())
}

#[derive(Debug)]
pub enum PngEncodeError {
    Header(String),
    Encode(String),
}

impl std::fmt::Display for PngEncodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Header(s) => write!(f, "PNG header error: {s}"),
            Self::Encode(s) => write!(f, "PNG encode error: {s}"),
        }
    }
}

impl std::error::Error for PngEncodeError {}
