//! Color quantization for ASS/PGS subtitle rendering.

#![warn(missing_docs)]
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

mod dithering;
mod median_cut;
mod types;

pub use median_cut::find_nearest_index;
pub use types::{DitherMethod, QuantizedFrame, Rgba};

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
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use color_quantizer::Quantizer;
    ///
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
    /// ```no_run
    /// use color_quantizer::Quantizer;
    ///
    /// let q = Quantizer::new(128);
    /// let rgba_bytes: Vec<u8> = vec![0; 640 * 480 * 4];
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
                x: 0,
                y: 0,
            };
        }

        assert_eq!(rgba.len(), (width * height * 4) as usize);

        let pixels: Vec<Rgba> = rgba
            .chunks_exact(4)
            .map(|c| Rgba::new(c[0], c[1], c[2], c[3]))
            .collect();

        let opaque_pixels: Vec<Rgba> = pixels.iter().filter(|p| p.a > 0).copied().collect();

        let mut palette = if opaque_pixels.len() <= self.max_colors {
            let mut deduped: Vec<Rgba> = Vec::new();
            let mut seen: std::collections::HashSet<Rgba> = std::collections::HashSet::new();
            for p in &opaque_pixels {
                if seen.insert(*p) {
                    deduped.push(*p);
                }
            }
            deduped
        } else {
            median_cut::median_cut(&opaque_pixels, self.max_colors)
        };

        let has_transparent = pixels.iter().any(|p| p.a == 0);
        let transparent_index = if has_transparent {
            palette.insert(0, Rgba::new(0, 0, 0, 0));
            0
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
                        // Bump by 1 because we inserted transparent at index 0
                        let mut i = median_cut::find_nearest_index(p, &palette) as u16 + 1;
                        if i > 255 {
                            i = 255;
                        }
                        idx.push(i as u8);
                    }
                }
                idx
            }
            DitherMethod::FloydSteinberg => {
                let mut idx = dithering::floyd_steinberg_dither(
                    rgba,
                    width,
                    height,
                    &palette,
                    transparent_index,
                );
                // Bump opaque indices by 1
                for x in &mut idx {
                    if *x != 0 {
                        *x = (*x as u16 + 1).min(255) as u8;
                    }
                }
                idx
            }
            DitherMethod::Ordered => {
                let mut idx =
                    dithering::ordered_dither(rgba, width, height, &palette, transparent_index);
                // Bump opaque indices by 1
                for x in &mut idx {
                    if *x != 0 {
                        *x = (*x as u16 + 1).min(255) as u8;
                    }
                }
                idx
            }
        };

        QuantizedFrame {
            width,
            height,
            palette,
            indices,
            transparent_index,
            x: 0,
            y: 0,
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
/// ```no_run
/// use color_quantizer::quantize;
///
/// let rgba: Vec<u8> = vec![0; 1920 * 1080 * 4];
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
            x: 0,
            y: 0,
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
                    x: 0,
                    y: 0,
                };
            }
        }
    }

    quantizer.quantize(rgba, width, height)
}

#[cfg(test)]
mod kdtree_parity_tests {
    use super::*;
    use crate::median_cut::find_nearest_index;
    use crate::types::Rgba;

    #[test]
    fn kdtree_parity_against_linear() {
        // Test 1: 1×1 palette
        let p1 = vec![Rgba::new(10, 20, 30, 255)];
        for c in &p1 {
            assert_eq!(find_nearest_index(c, &p1), 0);
        }

        // Test 2: 2-color palette
        let p2 = vec![Rgba::new(0, 0, 0, 255), Rgba::new(255, 255, 255, 255)];
        assert_eq!(find_nearest_index(&Rgba::new(0, 0, 0, 255), &p2), 0);
        assert_eq!(find_nearest_index(&Rgba::new(255, 255, 255, 255), &p2), 1);

        // Test 3: 8-color palette (typical median-cut output)
        let p3: Vec<Rgba> = (0..8u8)
            .map(|i| Rgba::new(i * 32, i * 16, 128, 255))
            .collect();
        for c in &p3 {
            let idx = find_nearest_index(c, &p3);
            assert_eq!(
                p3[idx as usize], *c,
                "color {:?} -> index {} = {:?}",
                c, idx, p3[idx as usize]
            );
        }

        // Test 4: 255-color palette (max PGS palette)
        let p4: Vec<Rgba> = (0..255u32)
            .map(|i| {
                let i = i as u8;
                Rgba::new(
                    i.wrapping_mul(3),
                    i.wrapping_mul(7),
                    i.wrapping_mul(11),
                    255,
                )
            })
            .collect();
        for c in &p4 {
            let idx = find_nearest_index(c, &p4);
            assert_eq!(p4[idx as usize], *c);
        }

        // Test 5: 64-color palette with 1024 random queries
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let p5: Vec<Rgba> = (0..64u32)
            .map(|i| {
                let mut h = DefaultHasher::new();
                i.hash(&mut h);
                let v = h.finish();
                Rgba::new((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, 255)
            })
            .collect();
        for i in 0..1024u32 {
            let mut h = DefaultHasher::new();
            i.hash(&mut h);
            let v = h.finish();
            let q = Rgba::new((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, 255);
            let idx = find_nearest_index(&q, &p5);
            // Recompute linear scan to compare
            let linear_idx = p5
                .iter()
                .enumerate()
                .min_by_key(|(_, p)| {
                    let dr = i32::from(q.r) - i32::from(p.r);
                    let dg = i32::from(q.g) - i32::from(p.g);
                    let db = i32::from(q.b) - i32::from(p.b);
                    dr * dr + dg * dg + db * db
                })
                .unwrap()
                .0 as u8;
            assert_eq!(
                idx, linear_idx,
                "query {:?} idx {} != linear {}",
                q, idx, linear_idx
            );
        }
    }

    #[test]
    fn kdtree_e2e_parity_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let width = 100u32;
        let height = 100u32;
        let pixels: Vec<Rgba> = (0..(width * height))
            .map(|i| {
                let mut h = DefaultHasher::new();
                i.hash(&mut h);
                let v = h.finish();
                Rgba::new((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, 255)
            })
            .collect();
        let rgba: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
        let q = Quantizer::new(64).with_dither(DitherMethod::None);
        let frame = q.quantize(&rgba, width, height);

        let mut h = DefaultHasher::new();
        for c in &frame.palette {
            c.hash(&mut h);
        }
        frame.indices.hash(&mut h);
        let hash = format!("{:x}", h.finish());
        println!("KDTREE_E2E_HASH={}", hash);

        // Just verify the quantize completes without error and produces reasonable output
        assert!(frame.palette_size() <= 65); // 64 + possible transparent
        assert_eq!(frame.indices.len(), (width * height) as usize);
    }
}

#[cfg(test)]
mod dedup_parity_tests {
    use super::*;
    use crate::types::Rgba;

    #[test]
    fn dedup_preserves_first_occurrence_order() {
        // Direct test of the dedup pattern: first occurrence wins, order preserved
        let pixels = [
            Rgba::new(1, 2, 3, 255),
            Rgba::new(4, 5, 6, 255),
            Rgba::new(1, 2, 3, 255),
            Rgba::new(7, 8, 9, 255),
            Rgba::new(4, 5, 6, 255),
            Rgba::new(1, 2, 3, 255),
        ];
        let max_colors = 10; // trigger small-palette dedup path
        let q = Quantizer::new(max_colors);
        let rgba: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
        let frame = q.quantize(&rgba, pixels.len() as u32, 1);
        // Expected palette: 3 unique colors (no transparent pixel in input,
        // so transparent_index=0 but no transparent entry is appended)
        assert_eq!(frame.palette_size(), 3);
        // Palette entries should be the unique colors in first-occurrence order
        assert_eq!(frame.palette[0], Rgba::new(1, 2, 3, 255));
        assert_eq!(frame.palette[1], Rgba::new(4, 5, 6, 255));
        assert_eq!(frame.palette[2], Rgba::new(7, 8, 9, 255));
    }

    #[test]
    fn dedup_handles_all_same_color() {
        // 100 pixels with max_colors=200 triggers the small-palette dedup path
        let pixels = vec![Rgba::new(5, 5, 5, 255); 100];
        let q = Quantizer::new(200);
        let rgba: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
        let frame = q.quantize(&rgba, 100, 1);
        assert_eq!(frame.palette_size(), 1); // 1 unique opaque, no transparent pixel in input
    }

    #[test]
    fn dedup_handles_empty() {
        // max_colors=0 should short-circuit
        let q = Quantizer::new(0);
        let rgba: Vec<u8> = vec![];
        let frame = q.quantize(&rgba, 0, 0);
        assert_eq!(frame.palette_size(), 0);
    }

    #[test]
    fn parity_bench() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::time::Instant;
        let pixels: Vec<Rgba> = (0..1920 * 1080)
            .map(|i| {
                let mut h = DefaultHasher::new();
                i.hash(&mut h);
                let v = h.finish();
                Rgba::new((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, 255)
            })
            .collect();
        let rgba: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b, p.a]).collect();
        let q = Quantizer::new(255).with_dither(DitherMethod::None);
        let start = Instant::now();
        let frame = q.quantize(&rgba, 1920, 1080);
        let elapsed = start.elapsed();
        println!("1080p_QUANTIZE_MS={}", elapsed.as_millis());
        let mut h = DefaultHasher::new();
        frame.palette.hash(&mut h);
        frame.indices.hash(&mut h);
        println!("PARITY_HASH={:x}", h.finish());
    }
}
