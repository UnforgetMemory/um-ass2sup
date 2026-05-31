mod types;
mod median_cut;
mod dithering;

pub use types::{DitherMethod, QuantizedFrame, Rgba};
pub use median_cut::find_nearest_index;

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
    pub fn new(max_colors: usize) -> Self {
        Self {
            max_colors: max_colors.min(255),
            dither: DitherMethod::FloydSteinberg,
        }
    }

    pub fn with_dither(mut self, dither: DitherMethod) -> Self {
        self.dither = dither;
        self
    }

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

pub fn quantize(rgba: &[u8], width: u32, height: u32) -> QuantizedFrame {
    Quantizer::default().quantize(rgba, width, height)
}

/// Quantize reusing a previous palette when possible (palette reuse optimization).
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
