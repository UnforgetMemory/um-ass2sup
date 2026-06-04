//! Text shaping using rustybuzz.
//!
//! Converts Unicode text runs into positioned glyph sequences with metrics
//! required for rasterization. Handles complex scripts (Arabic, Thai, Indic)
//! through HarfBuzz's shaping engine.

use crate::font::{FontError, FontManager};
use rustybuzz::Face;

/// A single shaped glyph with position and advance metrics.
///
/// Produced by [`Shaper::shape`]. The `cluster` field maps back to the
/// original text byte offset for karaoke syllable boundary detection.
#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub glyph_id: u32,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub cluster: u32,
}

/// Result of shaping a text run: a sequence of positioned glyphs.
///
/// `total_advance` is the sum of all glyph x_advances and can be used for
/// text width measurement and alignment calculations.
#[derive(Debug, Clone)]
pub struct ShapedText {
    pub glyphs: Vec<ShapedGlyph>,
    pub total_advance: f32,
}

/// Shaper that converts Unicode text into positioned glyph sequences.
///
/// Wraps a [`FontManager`] reference and uses rustybuzz (HarfBuzz) for
/// OpenType text shaping. Handles complex scripts, ligatures, and kerning.
pub struct Shaper<'a> {
    font_manager: &'a FontManager,
}

impl<'a> Shaper<'a> {
    /// Create a new shaper bound to the given font manager.
    pub fn new(font_manager: &'a FontManager) -> Self {
        Self { font_manager }
    }

    /// Shape a text run into positioned glyphs.
    ///
    /// `font_size` is in pixels. Returns a [`ShapedText`] with glyph
    /// metrics scaled from font units to pixels.
    ///
    /// # Errors
    ///
    /// Returns [`FontError::NotFound`] if `font_id` is not in the manager, or
    /// [`FontError::ParseError`] if the font data is invalid.
    pub fn shape(
        &self,
        text: &str,
        font_id: fontdb::ID,
        font_size: f32,
    ) -> Result<ShapedText, FontError> {
        let data = self
            .font_manager
            .get_font_data(font_id)
            .ok_or_else(|| FontError::NotFound("Font ID not found".into()))?;

        let face = Face::from_slice(&data, 0)
            .ok_or_else(|| FontError::ParseError("Failed to parse font".into()))?;

        let scale = font_size / face.units_per_em() as f32;
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);
        buffer.guess_segment_properties();

        let output = rustybuzz::shape(&face, &[], buffer);

        let mut total_advance = 0.0f32;
        let glyphs: Vec<ShapedGlyph> = output
            .glyph_infos()
            .iter()
            .zip(output.glyph_positions().iter())
            .map(|(info, pos)| {
                let x_advance = pos.x_advance as f32 * scale;
                let y_advance = pos.y_advance as f32 * scale;
                let x_offset = pos.x_offset as f32 * scale;
                let y_offset = pos.y_offset as f32 * scale;
                total_advance += x_advance;
                ShapedGlyph {
                    glyph_id: info.glyph_id,
                    x_advance,
                    y_advance,
                    x_offset,
                    y_offset,
                    cluster: info.cluster,
                }
            })
            .collect();

        Ok(ShapedText {
            glyphs,
            total_advance,
        })
    }

    /// Get the bounding box for a specific glyph, scaled to `font_size`.
    ///
    /// Returns `None` if the font or glyph is not found.
    pub fn get_glyph_bbox(
        &self,
        font_id: fontdb::ID,
        glyph_id: u32,
        font_size: f32,
    ) -> Option<GlyphBBox> {
        let data = self.font_manager.get_font_data(font_id)?;
        let face = ttf_parser::Face::parse(&data, 0).ok()?;
        let scale = font_size / f32::from(face.units_per_em());
        let bbox = face.glyph_bounding_box(ttf_parser::GlyphId(glyph_id as u16))?;

        Some(GlyphBBox {
            x_min: f32::from(bbox.x_min) * scale,
            y_min: f32::from(bbox.y_min) * scale,
            x_max: f32::from(bbox.x_max) * scale,
            y_max: f32::from(bbox.y_max) * scale,
        })
    }
}

/// Axis-aligned bounding box for a glyph, in pixels.
///
/// Used by the rasterizer to allocate the correct bitmap region for each glyph.
#[derive(Debug, Clone, Copy)]
pub struct GlyphBBox {
    pub x_min: f32,
    pub y_min: f32,
    pub x_max: f32,
    pub y_max: f32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::FontManager;

    fn setup_shaper() -> (FontManager, fontdb::ID) {
        let mut fm = FontManager::new();
        fm.load_system_fonts();
        let id = fm
            .query("Arial", false, false)
            .or_else(|| fm.query("Liberation Sans", false, false))
            .or_else(|| fm.query("DejaVu Sans", false, false))
            .or_else(|| fm.query("Noto Sans", false, false))
            .or_else(|| fm.list_fonts().first().map(|f| f.id))
            .expect("No system fonts found");
        (fm, id)
    }

    #[test]
    fn test_shape_idempotent_same_input() {
        let (fm, id) = setup_shaper();
        let shaper = Shaper::new(&fm);
        let r1 = shaper.shape("Hello World", id, 48.0).unwrap();
        let r2 = shaper.shape("Hello World", id, 48.0).unwrap();
        assert_eq!(r1.glyphs.len(), r2.glyphs.len());
        assert!((r1.total_advance - r2.total_advance).abs() < 0.01);
        for (g1, g2) in r1.glyphs.iter().zip(r2.glyphs.iter()) {
            assert_eq!(g1.glyph_id, g2.glyph_id);
            assert!((g1.x_advance - g2.x_advance).abs() < 0.01);
            assert!((g1.x_offset - g2.x_offset).abs() < 0.01);
        }
    }

    #[test]
    fn test_shape_different_text_different_output() {
        let (fm, id) = setup_shaper();
        let shaper = Shaper::new(&fm);
        let r1 = shaper.shape("A", id, 48.0).unwrap();
        let r2 = shaper.shape("BBBBBBBB", id, 48.0).unwrap();
        assert!(r1.glyphs.len() != r2.glyphs.len() || r1.total_advance != r2.total_advance);
    }

    #[test]
    fn test_shape_different_sizes_different_advance() {
        let (fm, id) = setup_shaper();
        let shaper = Shaper::new(&fm);
        let r1 = shaper.shape("Hello", id, 24.0).unwrap();
        let r2 = shaper.shape("Hello", id, 48.0).unwrap();
        assert!(
            r2.total_advance > r1.total_advance,
            "Larger font size should produce wider advance"
        );
    }

    #[test]
    fn test_shape_empty_string() {
        let (fm, id) = setup_shaper();
        let shaper = Shaper::new(&fm);
        let r = shaper.shape("", id, 48.0).unwrap();
        assert!(r.glyphs.is_empty());
        assert!((r.total_advance - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_shape_single_char_has_one_glyph() {
        let (fm, id) = setup_shaper();
        let shaper = Shaper::new(&fm);
        let r = shaper.shape("A", id, 48.0).unwrap();
        assert_eq!(r.glyphs.len(), 1);
        assert!(r.total_advance > 0.0);
    }

    #[test]
    fn test_get_glyph_bbox_returns_some() {
        let (fm, id) = setup_shaper();
        let shaper = Shaper::new(&fm);
        let bbox = shaper.get_glyph_bbox(id, 0, 48.0);
        assert!(
            bbox.is_some(),
            "Glyph bbox should be available for valid glyph"
        );
    }

    #[test]
    fn test_get_glyph_bbox_scaled() {
        let (fm, id) = setup_shaper();
        let shaper = Shaper::new(&fm);
        let b1 = shaper.get_glyph_bbox(id, 0, 24.0).unwrap();
        let b2 = shaper.get_glyph_bbox(id, 0, 48.0).unwrap();
        let w1 = b1.x_max - b1.x_min;
        let w2 = b2.x_max - b2.x_min;
        assert!(
            (w2 - w1 * 2.0).abs() < 1.0,
            "2x font size should produce ~2x glyph width"
        );
    }
}
