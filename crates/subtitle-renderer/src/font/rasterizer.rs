//! Glyph rasterization via swash.
//!
//! Converts a glyph ID + font data into an alpha bitmap using
//! `swash::scale::Scaler` and `swash::scale::Render`.

use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source};
use swash::zeno::Format;
use swash::FontRef;

use super::error::FontError;
use super::types::RasterizedGlyph;

/// Rasterizes individual glyphs into alpha bitmaps.
pub struct GlyphRasterizer;

impl GlyphRasterizer {
    /// Rasterize a single glyph at the given pixel size.
    ///
    /// Returns an alpha bitmap in [`RasterizedGlyph`] with placement info.
    pub fn rasterize(
        font_data: &[u8],
        glyph_id: u16,
        size: f32,
    ) -> Result<RasterizedGlyph, FontError> {
        let font = FontRef::from_index(font_data, 0).ok_or_else(|| FontError::Corrupted {
            path: Default::default(),
            reason: "swash: could not parse font data".into(),
        })?;

        let mut ctx = ScaleContext::new();
        let mut scaler = ctx.builder(font)
            .size(size)
            .hint(false)
            .build();

        let image = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .render(&mut scaler, glyph_id)
            .ok_or_else(|| FontError::Corrupted {
                path: Default::default(),
                reason: format!("swash: could not render glyph {glyph_id}"),
            })?;

        let p = &image.placement;
        let w = p.width;
        let h = p.height;

        let data = match image.content {
            Content::Mask | Content::SubpixelMask => image.data.to_vec(),
            Content::Color => image.data.chunks(4).map(|px| px[3]).collect(),
        };

        Ok(RasterizedGlyph {
            data,
            width: w,
            height: h,
            left: p.left,
            top: p.top,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn dejavu_data() -> Vec<u8> {
        std::fs::read("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf")
            .expect("DejaVuSans.ttf not found")
    }

    #[test]
    fn rasterize_a_returns_bitmap() {
        let data = dejavu_data();
        let font = FontRef::from_index(&data, 0).unwrap();
        let glyph_id = font.charmap().map('A');
        let result = GlyphRasterizer::rasterize(&data, glyph_id, 48.0).unwrap();
        assert!(!result.data.is_empty());
        assert!(result.width > 0);
        assert!(result.height > 0);
    }

    #[test]
    fn rasterize_dimensions_reasonable() {
        let data = dejavu_data();
        let font = FontRef::from_index(&data, 0).unwrap();
        let glyph_id = font.charmap().map('A');
        let result = GlyphRasterizer::rasterize(&data, glyph_id, 48.0).unwrap();
        assert!(result.width <= 200, "width too large: {}", result.width);
        assert!(result.height <= 200, "height too large: {}", result.height);
    }

    #[test]
    fn rasterize_different_sizes_differ() {
        let data = dejavu_data();
        let font = FontRef::from_index(&data, 0).unwrap();
        let glyph_id = font.charmap().map('A');
        let small = GlyphRasterizer::rasterize(&data, glyph_id, 16.0).unwrap();
        let large = GlyphRasterizer::rasterize(&data, glyph_id, 48.0).unwrap();
        assert!(large.width >= small.width);
    }

    #[test]
    fn rasterize_invalid_font_returns_error() {
        let bad_data = vec![0x00, 0x01, 0x02, 0x03];
        let result = GlyphRasterizer::rasterize(&bad_data, 1, 48.0);
        assert!(result.is_err());
    }
}
