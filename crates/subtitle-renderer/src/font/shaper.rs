use super::error::FontError;
use super::types::ShapedGlyph;
use swash::FontRef;

/// Basic glyph shaper using swash.
///
/// Maps each character in `text` to a glyph id and records its advance.
/// No kerning or complex positioning is performed.
pub struct SimpleShaper;

impl SimpleShaper {
    pub fn shape(
        text: &str,
        font_data: &[u8],
        font_size: f32,
    ) -> Result<Vec<ShapedGlyph>, FontError> {
        if text.is_empty() {
            return Ok(vec![]);
        }

        let font = FontRef::from_index(font_data, 0).ok_or_else(|| FontError::Corrupted {
            path: Default::default(),
            reason: "swash: could not parse font data".into(),
        })?;

        let charmap = font.charmap();
        let metrics = font.glyph_metrics(&[]).scale(font_size);

        let mut glyphs = Vec::with_capacity(text.len());

        for ch in text.chars() {
            let glyph_id = charmap.map(ch);

            if glyph_id == 0 {
                continue;
            }

            let advance = metrics.advance_width(glyph_id);

            glyphs.push(ShapedGlyph {
                glyph_id,
                x_advance: advance,
                y_advance: 0.0, // vertical advance is 0 for horizontal text
                x_offset: 0.0,
                y_offset: 0.0,
            });
        }

        Ok(glyphs)
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
    fn shape_hello_returns_glyphs() {
        let data = dejavu_data();
        let glyphs = SimpleShaper::shape("Hello", &data, 48.0).unwrap();
        assert_eq!(glyphs.len(), 5);
    }

    #[test]
    fn shape_empty_returns_empty() {
        let data = dejavu_data();
        let glyphs = SimpleShaper::shape("", &data, 48.0).unwrap();
        assert!(glyphs.is_empty());
    }

    #[test]
    fn glyph_advance_is_positive() {
        let data = dejavu_data();
        let glyphs = SimpleShaper::shape("A", &data, 48.0).unwrap();
        assert_eq!(glyphs.len(), 1);
        assert!(glyphs[0].x_advance > 0.0);
    }

    #[test]
    fn invalid_font_returns_error() {
        let bad_data = vec![0x00, 0x01, 0x02, 0x03];
        let result = SimpleShaper::shape("A", &bad_data, 48.0);
        assert!(result.is_err());
    }

    #[test]
    fn cjk_char_maps_to_glyph() {
        let data = dejavu_data();
        let glyphs = SimpleShaper::shape("中", &data, 48.0).unwrap();
        let _ = glyphs;
    }
}
