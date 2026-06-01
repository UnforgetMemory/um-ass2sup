//! Color quantization for ASS/PGS subtitle rendering.
//!
//! Reduces 32-bit RGBA frames to indexed-palette images with at most 255
//! opaque colors, which is required by the PGS (Presentation Graphic Stream)
//! subtitle format used on Blu-ray discs.
//!
//! The main entry point is [`Quantizer`], which supports median-cut palette
//! reduction and optional Floyd–Steinberg or ordered dithering. A convenience
//! [`quantize`] function is provided for one-shot use, and
//! [`quantize_with_palette`] enables palette reuse across consecutive frames
//! to improve temporal compression in the final PGS stream.

mod types;
mod median_cut;
mod dithering;

pub use types::{DitherMethod, QuantizedFrame, Rgba};
pub use median_cut::find_nearest_index;

/// ASS subtitle color quantizer — reduces 32-bit RGBA frames to an
/// indexed-palette image with at most 255 opaque colors plus one
/// transparent entry.
///
/// This is the primary entry point for palette reduction. The quantizer
/// builds a palette using the median-cut algorithm and optionally applies
/// dithering to reduce banding.
///
/// # Examples
///
/// ```ignore
/// let q = Quantizer::new(256)
///     .with_dither(DitherMethod::FloydSteinberg);
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
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let q = Quantizer::new(128); // at most 128 opaque colors
    /// ```
    pub fn new(max_colors: usize) -> Self {
        Self {
            max_colors: max_colors.min(255),
            dither: DitherMethod::FloydSteinberg,
        }
    }

    /// Sets the dithering method and returns the quantizer (builder pattern).
    ///
    /// Dithering trades pixel-level accuracy for smoother gradients. Default
    /// is [`DitherMethod::FloydSteinberg`].
    pub fn with_dither(mut self, dither: DitherMethod) -> Self {
        self.dither = dither;
        self
    }

    /// Quantizes an RGBA frame into an indexed-palette image.
    ///
    /// The input is a flat byte array in row-major order, 4 bytes per pixel
    /// (R, G, B, A). Transparent pixels (`a == 0`) are mapped to a dedicated
    /// transparent palette index rather than being included in the color
    /// histogram.
    ///
    /// # Panics
    ///
    /// Panics if `rgba.len() != width * height * 4`.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let frame = q.quantize(&rgba_bytes, 640, 480);
    /// assert_eq!(frame.width, 640);
    /// assert!(frame.palette_size() <= 256);
    /// ```
    pub fn quantize(&self, rgba: &[u8], width: u32, height: u32) -> QuantizedFrame {
        if self.max_colors == 0 || rgba.is_empty() {
            return QuantizedFrame {
                width,
                height,
                palette: Vec::new(),
                indices: vec![0u8; (width * height) as usize],
                transparent_index: 0,
            };
        }

        assert_eq!(rgba.len(), (width * height * 4) as usize);

        let pixels: Vec<Rgba> = rgba
            .chunks_exact(4)
            .map(|c| Rgba::new(c[0], c[1], c[2], c[3]))
            .collect();

        let opaque_pixels: Vec<Rgba> = pixels
            .iter()
            .filter(|p| p.a > 0)
            .copied()
            .collect();

        let mut palette = if opaque_pixels.len() <= self.max_colors {
            let mut deduped: Vec<Rgba> = Vec::new();
            for p in &opaque_pixels {
                if !deduped.contains(p) {
                    deduped.push(*p);
                }
            }
            deduped
        } else {
            median_cut::median_cut(&opaque_pixels, self.max_colors)
        };

        let has_transparent = pixels.iter().any(|p| p.a == 0);
        let transparent_index = if has_transparent {
            palette.push(Rgba::new(0, 0, 0, 0));
            (palette.len() - 1) as u8
        } else if palette.is_empty() {
            palette.push(Rgba::new(0, 0, 0, 0));
            0
        } else {
            0
        };

        let indices = match self.dither {
            DitherMethod::None => {
                let mut idx = Vec::with_capacity(pixels.len());
                for p in &pixels {
                    if p.a == 0 {
                        idx.push(transparent_index);
                    } else {
                        idx.push(median_cut::find_nearest_index(p, &palette));
                    }
                }
                idx
            }
            DitherMethod::FloydSteinberg => {
                dithering::floyd_steinberg_dither(rgba, width, height, &palette, transparent_index)
            }
            DitherMethod::Ordered => {
                dithering::ordered_dither(rgba, width, height, &palette, transparent_index)
            }
        };

        QuantizedFrame {
            width,
            height,
            palette,
            indices,
            transparent_index,
        }
    }
}

/// Convenience quantization using a default [`Quantizer`] (255 colors,
/// Floyd–Steinberg dithering).
///
/// Equivalent to `Quantizer::default().quantize(rgba, width, height)`.
///
/// # Examples
///
/// ```ignore
/// let frame = quantize(&rgba, 1920, 1080);
/// ```
pub fn quantize(rgba: &[u8], width: u32, height: u32) -> QuantizedFrame {
    Quantizer::default().quantize(rgba, width, height)
}

/// Quantizes an RGBA frame, reusing a previous palette when all pixels
/// can be mapped to it.
///
/// This is an optimization for consecutive subtitle frames that share a
/// similar color profile (common in ASS dialogue). If every opaque pixel
/// in the frame has a sufficiently close match in `prev_palette`, the
/// existing palette is reused without recomputing it, which improves
/// temporal compression in the PGS output.
///
/// When palette reuse is not possible (or `prev_palette` is `None`/empty),
/// this falls back to a full quantization via [`Quantizer`].
pub fn quantize_with_palette(
    rgba: &[u8],
    width: u32,
    height: u32,
    prev_palette: Option<&[Rgba]>,
    max_colors: usize,
    dither: DitherMethod,
) -> QuantizedFrame {
    if rgba.is_empty() || width == 0 || height == 0 {
        return QuantizedFrame {
            width,
            height,
            palette: Vec::new(),
            indices: vec![0u8; (width * height) as usize],
            transparent_index: 0,
        };
    }

    let quantizer = Quantizer::new(max_colors).with_dither(dither);

    if let Some(prev) = prev_palette {
        if !prev.is_empty() {
            let pixels: Vec<Rgba> = rgba
                .chunks_exact(4)
                .map(|c| Rgba::new(c[0], c[1], c[2], c[3]))
                .collect();

            let mut all_mappable = true;
            let mut indices = Vec::with_capacity(pixels.len());

            for p in &pixels {
                if p.a == 0 {
                    indices.push(0);
                } else {
                    let nearest = median_cut::find_nearest_index(p, prev);
                    if (nearest as usize) < prev.len() {
                        indices.push(nearest);
                    } else {
                        all_mappable = false;
                        break;
                    }
                }
            }

            if all_mappable {
                let has_transparent = pixels.iter().any(|p| p.a == 0);
                let (final_palette, transparent_index, remapped_indices) = if has_transparent {
                    let mut pal = prev.to_vec();
                    let t_idx = pal.iter().position(|p| p.a == 0);
                    let t_idx = match t_idx {
                        Some(i) => i as u8,
                        None => {
                            pal.push(Rgba::new(0, 0, 0, 0));
                            (pal.len() - 1) as u8
                        }
                    };
                    let remapped: Vec<u8> = indices
                        .iter()
                        .zip(pixels.iter())
                        .map(|(&i, p)| if p.a == 0 { t_idx } else { i })
                        .collect();
                    (pal, t_idx, remapped)
                } else {
                    (prev.to_vec(), 0, indices)
                };

                return QuantizedFrame {
                    width,
                    height,
                    palette: final_palette,
                    indices: remapped_indices,
                    transparent_index,
                };
            }
        }
    }

    quantizer.quantize(rgba, width, height)
}
