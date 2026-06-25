#![allow(missing_docs)]

//! ColorPipeline — end-to-end quantiser orchestration.
//!
//! Wires together colour-space conversion, palette generation
//! (median-cut or octree), nearest-neighbour mapping, dithering,
//! and temporal palette reuse.

use crate::color::tonemap::{tone_map_rgba, ToneMapOperator};
use crate::color::ColorSpace;
use crate::dither;
use crate::quantize::{self, QuantizeMethod};
use crate::DitherMethod;
use crate::QuantizedFrame;
use crate::Rgba;

/// Convert a flat `[[u8; 4]]` palette slice to `Vec<Rgba>` for the public API.
#[inline]
fn to_rgba_palette(pal: &[[u8; 4]]) -> Vec<Rgba> {
    pal.iter()
        .map(|c| Rgba::new(c[0], c[1], c[2], c[3]))
        .collect()
}

/// Builder for the quantiser pipeline.
///
/// # Example
///
/// ```no_run
/// use color_quantizer::pipeline::ColorPipeline;
/// use color_quantizer::color::ColorSpace;
/// use color_quantizer::DitherMethod;
///
/// let pipeline = ColorPipeline::new()
///     .with_max_colors(128)
///     .with_color_space(ColorSpace::Bt709)
///     .with_dither(DitherMethod::FloydSteinberg);
///
/// let rgba_bytes: Vec<u8> = vec![0u8; 640 * 480 * 4];
/// let frame = pipeline.quantize(&rgba_bytes, 640, 480);
/// ```
pub struct ColorPipeline {
    pub max_colors: usize,
    pub dither: DitherMethod,
    pub color_space: ColorSpace,
    pub quantize_method: QuantizeMethod,
    pub tonemap: Option<ToneMapOperator>,
}

impl Default for ColorPipeline {
    fn default() -> Self {
        Self {
            max_colors: 255,
            dither: DitherMethod::FloydSteinberg,
            color_space: ColorSpace::Srgb,
            quantize_method: QuantizeMethod::MedianCut,
            tonemap: None,
        }
    }
}

impl ColorPipeline {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_max_colors(mut self, n: usize) -> Self {
        self.max_colors = n.min(255);
        self
    }

    pub fn with_dither(mut self, d: DitherMethod) -> Self {
        self.dither = d;
        self
    }

    pub fn with_color_space(mut self, cs: ColorSpace) -> Self {
        self.color_space = cs;
        self
    }

    pub fn with_quantize_method(mut self, qm: QuantizeMethod) -> Self {
        self.quantize_method = qm;
        self
    }

    pub fn with_tonemap(mut self, op: ToneMapOperator) -> Self {
        self.tonemap = Some(op);
        self
    }

    /// Quantise an RGBA frame to an indexed palette image.
    ///
    /// Flows: RGBA → [Tone map] → [Quantise] → [Dither] → QuantizedFrame
    pub fn quantize(&self, rgba: &[u8], width: u32, height: u32) -> QuantizedFrame {
        if rgba.is_empty() || width == 0 || height == 0 {
            return QuantizedFrame {
                width,
                height,
                palette: vec![Rgba::new(0, 0, 0, 0)],
                indices: vec![0u8; (width * height) as usize],
                transparent_index: 0,
                x: 0,
                y: 0,
                color_space: self.color_space,
                pts_ms: 0,
                duration_ms: 0,
            };
        }

        let n = (width * height) as usize;

        // Step 1: optional HDR → SDR tone mapping.
        let working_rgba = match self.tonemap {
            Some(op) => {
                let mut mapped = Vec::with_capacity(rgba.len());
                for chunk in rgba.chunks_exact(4) {
                    let (r, g, b, a) = tone_map_rgba(
                        chunk[0] as f32 / 255.0,
                        chunk[1] as f32 / 255.0,
                        chunk[2] as f32 / 255.0,
                        chunk[3] as f32 / 255.0,
                        op,
                    );
                    mapped.push((r * 255.0) as u8);
                    mapped.push((g * 255.0) as u8);
                    mapped.push((b * 255.0) as u8);
                    mapped.push((a * 255.0) as u8);
                }
                mapped
            }
            None => rgba.to_vec(),
        };

        // Step 2: collect opaque pixels for palette building.
        let opaque_pixels: Vec<[u8; 4]> = working_rgba
            .chunks_exact(4)
            .filter(|c| c[3] > 0)
            .map(|c| [c[0], c[1], c[2], c[3]])
            .collect();

        let has_transparent = working_rgba.chunks_exact(4).any(|c| c[3] == 0);

        // Step 3: build palette.
        let palette = if opaque_pixels.len() <= self.max_colors {
            let mut unique: Vec<[u8; 4]> = Vec::new();
            for p in &opaque_pixels {
                if !unique.contains(p) {
                    unique.push(*p);
                    if unique.len() >= self.max_colors {
                        break;
                    }
                }
            }
            unique
        } else {
            match self.quantize_method {
                QuantizeMethod::MedianCut => {
                    quantize::median_cut::quantize(&opaque_pixels, self.max_colors)
                }
                QuantizeMethod::Naarahara => {
                    quantize::naarahara::quantize(&opaque_pixels, self.max_colors)
                }
            }
        };

        // Step 4: build full palette with transparent entry at index 0.
        let mut full_palette = Vec::with_capacity(palette.len() + 1);
        if has_transparent {
            full_palette.push([0u8, 0, 0, 0]); // transparent entry
        }
        full_palette.extend(palette);

        let transparent_index = 0u8;

        // Step 5: map pixels → indices (with optional dither).
        let indices = match (self.dither, has_transparent) {
            (DitherMethod::None, _) => {
                let mut idx = Vec::with_capacity(n);
                let pal_slice = &full_palette[if has_transparent { 1.. } else { 0.. }];
                for chunk in working_rgba.chunks_exact(4) {
                    if chunk[3] == 0 {
                        idx.push(transparent_index);
                    } else {
                        let nearest = if has_transparent {
                            quantize::nearest::find_nearest_index(
                                &[chunk[0], chunk[1], chunk[2], chunk[3]],
                                pal_slice,
                            ) + 1
                        } else {
                            quantize::nearest::find_nearest_index(
                                &[chunk[0], chunk[1], chunk[2], chunk[3]],
                                &full_palette,
                            )
                        };
                        idx.push(nearest);
                    }
                }
                idx
            }
            (DitherMethod::FloydSteinberg, _) => {
                let raw =
                    dither::floyd_steinberg::dither(&working_rgba, width, height, &full_palette);
                if has_transparent {
                    raw.into_iter()
                        .zip(working_rgba.chunks_exact(4))
                        .map(|(i, c)| {
                            if c[3] == 0 {
                                0
                            } else if i == 0 {
                                quantize::nearest::find_nearest_index(
                                    &[c[0], c[1], c[2], c[3]],
                                    &full_palette[1..],
                                ) + 1
                            } else {
                                i
                            }
                        })
                        .collect()
                } else {
                    raw
                }
            }
            (DitherMethod::Ordered, _) => {
                let raw = dither::ordered::dither(&working_rgba, width, height, &full_palette);
                if has_transparent {
                    raw.into_iter()
                        .zip(working_rgba.chunks_exact(4))
                        .map(|(i, c)| {
                            if c[3] == 0 {
                                0
                            } else if i == 0 {
                                quantize::nearest::find_nearest_index(
                                    &[c[0], c[1], c[2], c[3]],
                                    &full_palette[1..],
                                ) + 1
                            } else {
                                i
                            }
                        })
                        .collect()
                } else {
                    raw
                }
            }
        };

        QuantizedFrame {
            width,
            height,
            palette: to_rgba_palette(&full_palette),
            indices,
            transparent_index,
            x: 0,
            y: 0,
            color_space: self.color_space,
            pts_ms: 0,
            duration_ms: 0,
        }
    }

    /// Quantise with optional temporal palette reuse.
    pub fn quantize_with_prev(
        &self,
        rgba: &[u8],
        width: u32,
        height: u32,
        prev_frame: Option<&QuantizedFrame>,
    ) -> QuantizedFrame {
        let _n = (width * height) as usize;
        if rgba.is_empty() || width == 0 || height == 0 {
            return QuantizedFrame::default();
        }

        // Try to reuse previous palette if all pixels map within threshold.
        if let Some(prev) = prev_frame {
            if !prev.palette.is_empty() {
                let pixels: Vec<[u8; 4]> = rgba
                    .chunks_exact(4)
                    .map(|c| [c[0], c[1], c[2], c[3]])
                    .collect();
                // Convert previous Rgba palette to flat [[u8;4]] for internal use.
                let flat_pal: Vec<[u8; 4]> =
                    prev.palette.iter().map(|c| [c.r, c.g, c.b, c.a]).collect();
                // The first palette entry is the transparent colour [0,0,0,0].
                // Exclude it from nearest-neighbour search so dark opaque
                // pixels do not accidentally match the transparent entry.
                let has_tr = flat_pal.first().map(|c| c[3] == 0).unwrap_or(false);
                let pal_for_search = if has_tr {
                    &flat_pal[1..]
                } else {
                    &flat_pal[..]
                };
                if quantize::temporal::all_mappable(&pixels, &flat_pal, 30.0) {
                    // Reuse previous palette: just remap.
                    let indices = pixels
                        .iter()
                        .map(|p| {
                            if p[3] == 0 {
                                prev.transparent_index
                            } else {
                                if has_tr {
                                    quantize::nearest::find_nearest_index(p, pal_for_search) + 1
                                } else {
                                    quantize::nearest::find_nearest_index(p, pal_for_search)
                                }
                            }
                        })
                        .collect();
                    return QuantizedFrame {
                        width,
                        height,
                        palette: prev.palette.clone(),
                        indices,
                        transparent_index: prev.transparent_index,
                        x: 0,
                        y: 0,
                        color_space: self.color_space,
                        pts_ms: 0,
                        duration_ms: 0,
                    };
                }
            }
        }

        self.quantize(rgba, width, height)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_input() {
        let p = ColorPipeline::new();
        let f = p.quantize(&[], 0, 0);
        assert_eq!(f.width, 0);
    }

    #[test]
    fn single_pixel_opaque() {
        let rgba = vec![255, 0, 0, 255];
        let p = ColorPipeline::new().with_dither(DitherMethod::None);
        let f = p.quantize(&rgba, 1, 1);
        assert!(!f.palette.is_empty());
        assert_eq!(f.indices.len(), 1);
    }

    #[test]
    fn single_pixel_transparent() {
        let rgba = vec![0, 0, 0, 0];
        let p = ColorPipeline::new().with_dither(DitherMethod::None);
        let f = p.quantize(&rgba, 1, 1);
        assert!(!f.palette.is_empty());
        assert_eq!(f.indices[0], f.transparent_index);
    }

    #[test]
    fn quantize_with_prev_reuses_palette() {
        let p = ColorPipeline::new().with_dither(DitherMethod::None);
        let rgba1 = vec![200u8; 4 * 10 * 10];
        let f1 = p.quantize(&rgba1, 10, 10);

        let rgba2 = vec![201u8; 4 * 10 * 10];
        let f2 = p.quantize_with_prev(&rgba2, 10, 10, Some(&f1));
        assert_eq!(f2.indices.len(), 100);
    }

    #[test]
    fn max_colors_respected() {
        let pixels: Vec<u8> = (0u16..256)
            .flat_map(|i| [i as u8, i as u8, i as u8, 255])
            .collect();
        let p = ColorPipeline::new()
            .with_max_colors(32)
            .with_dither(DitherMethod::None);
        let f = p.quantize(&pixels, 256, 1);
        assert!(
            f.palette_size() <= 33,
            "palette too large: {}",
            f.palette_size()
        );
    }
}
