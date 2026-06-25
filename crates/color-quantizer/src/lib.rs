//! Color quantization for ASS/PGS subtitle rendering.
//!
//! Reduces 32-bit RGBA frames to indexed-palette images with at most 255
//! opaque colors, which is required by the PGS (Presentation Graphic Stream)
//! subtitle format used on Blu-ray discs.
//!
//! The primary entry point is [`ColorPipeline`](crate::pipeline::ColorPipeline),
//! which wires together colour-space conversion, palette generation
//! (median-cut or octree), dithering, and temporal palette reuse.
//!
//! The legacy [`Quantizer`] builder is kept for backwards compatibility but
//! internally delegates to `ColorPipeline`.

#![warn(missing_docs)]

mod types;

pub mod color;
pub mod dither;
pub mod error;
pub mod frame;
pub mod pipeline;
pub mod quantize;

pub use crate::frame::RgbaRef;
pub use types::{DitherMethod, QuantizedFrame, Rgba};

/// Legacy quantizer — delegates to [`ColorPipeline`] internally.
///
/// Prefer [`ColorPipeline`](crate::pipeline::ColorPipeline) for new code;
/// it offers colour-space selection, HDR tonemapping, and octree quantisation.
///
/// # Examples
///
/// ```no_run
/// use color_quantizer::{Quantizer, DitherMethod};
///
/// let q = Quantizer::new(256)
///     .with_dither(DitherMethod::FloydSteinberg);
/// let rgba_pixels: Vec<u8> = vec![0; 1920 * 1080 * 4];
/// let frame = q.quantize(&rgba_pixels, 1920, 1080);
/// assert!(frame.palette_size() <= 256);
/// ```
pub struct Quantizer {
    max_colors: usize,
    dither: DitherMethod,
}

impl Default for Quantizer {
    fn default() -> Self {
        Self {
            max_colors: 255,
            dither: DitherMethod::FloydSteinberg,
        }
    }
}

impl Quantizer {
    /// Creates a new quantizer with the given maximum palette size.
    ///
    /// The value is clamped to 255 because PGS reserves one palette index
    /// for transparency. Values above 255 are silently reduced to 255.
    pub fn new(max_colors: usize) -> Self {
        Self {
            max_colors: max_colors.min(255),
            dither: DitherMethod::FloydSteinberg,
        }
    }

    /// Sets the dithering method (builder pattern).
    pub fn with_dither(mut self, dither: DitherMethod) -> Self {
        self.dither = dither;
        self
    }

    /// Quantize RGBA bytes to an indexed-palette frame.
    ///
    /// Delegates to [`ColorPipeline`](crate::pipeline::ColorPipeline).
    pub fn quantize(&self, rgba: &[u8], width: u32, height: u32) -> QuantizedFrame {
        let pipe = crate::pipeline::ColorPipeline::new()
            .with_max_colors(self.max_colors)
            .with_dither(self.dither);
        pipe.quantize(rgba, width, height)
    }
}

/// Convenience quantisation using default settings (255 colours,
/// Floyd–Steinberg dithering).
///
/// Equivalent to `Quantizer::default().quantize(rgba, width, height)`.
pub fn quantize(rgba: &[u8], width: u32, height: u32) -> QuantizedFrame {
    Quantizer::default().quantize(rgba, width, height)
}

/// Quantise an RGBA frame, optionally reusing a previous palette.
///
/// Delegates to [`ColorPipeline::quantize_with_prev`].
pub fn quantize_with_palette(
    rgba: &[u8],
    width: u32,
    height: u32,
    prev_palette: Option<&[Rgba]>,
    max_colors: usize,
    dither: DitherMethod,
) -> QuantizedFrame {
    let pipe = crate::pipeline::ColorPipeline::new()
        .with_max_colors(max_colors)
        .with_dither(dither);
    let prev = prev_palette.map(|pal| QuantizedFrame {
        width,
        height,
        palette: pal.to_vec(),
        indices: Vec::new(),
        transparent_index: 0,
        x: 0,
        y: 0,
        color_space: crate::color::ColorSpace::Srgb,
        pts_ms: 0,
        duration_ms: 0,
    });
    pipe.quantize_with_prev(rgba, width, height, prev.as_ref())
}

#[cfg(test)]
mod legacy_parity_tests {
    use super::*;

    #[test]
    fn quantize_empty() {
        let f = quantize(&[], 0, 0);
        assert_eq!(f.width, 0);
    }

    #[test]
    fn quantize_single_opaque() {
        let rgba = vec![100, 150, 200, 255];
        let f = Quantizer::new(16).quantize(&rgba, 1, 1);
        assert!(!f.palette.is_empty());
        assert_eq!(f.indices.len(), 1);
    }

    #[test]
    fn quantize_single_transparent() {
        let rgba = vec![0, 0, 0, 0];
        let f = Quantizer::new(16).quantize(&rgba, 1, 1);
        assert_eq!(f.indices[0], f.transparent_index);
    }

    #[test]
    fn quantize_dither_none() {
        let rgba = vec![200u8; 4 * 10 * 10];
        let q = Quantizer::new(16).with_dither(DitherMethod::None);
        let f = q.quantize(&rgba, 10, 10);
        assert_eq!(f.indices.len(), 100);
    }

    #[test]
    fn quantize_with_palette_reuse() {
        let rgba1 = vec![200u8; 4 * 10 * 10];
        let rgba2 = vec![201u8; 4 * 10 * 10];
        let f1 = quantize_with_palette(&rgba1, 10, 10, None, 16, DitherMethod::None);
        let f2 = quantize_with_palette(&rgba2, 10, 10, Some(&f1.palette), 16, DitherMethod::None);
        assert_eq!(f2.indices.len(), 100);
    }
}
