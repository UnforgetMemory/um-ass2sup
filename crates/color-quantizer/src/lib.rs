mod types;
mod median_cut;
mod dithering;

pub use types::{DitherMethod, QuantizedFrame, Rgba};

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
            opaque_pixels.clone()
        } else {
            median_cut::median_cut(&opaque_pixels, self.max_colors)
        };

        let has_transparent = pixels.iter().any(|p| p.a == 0);
        let transparent_index = if has_transparent {
            palette.push(Rgba::new(0, 0, 0, 0));
            (palette.len() - 1) as u8
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
