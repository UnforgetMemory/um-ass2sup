use crate::font::{FontError, FontManager};
use rustybuzz::Face;

#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub glyph_id: u32,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub cluster: u32,
}

#[derive(Debug, Clone)]
pub struct ShapedText {
    pub glyphs: Vec<ShapedGlyph>,
    pub total_advance: f32,
}

pub struct Shaper<'a> {
    font_manager: &'a FontManager,
}

impl<'a> Shaper<'a> {
    pub fn new(font_manager: &'a FontManager) -> Self {
        Self { font_manager }
    }

    pub fn shape(&self, text: &str, font_id: fontdb::ID, font_size: f32) -> Result<ShapedText, FontError> {
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

    pub fn get_glyph_bbox(
        &self,
        font_id: fontdb::ID,
        glyph_id: u32,
        font_size: f32,
    ) -> Option<GlyphBBox> {
        let data = self.font_manager.get_font_data(font_id)?;
        let face = ttf_parser::Face::parse(&data, 0).ok()?;
        let scale = font_size / face.units_per_em() as f32;
        let bbox = face.glyph_bounding_box(ttf_parser::GlyphId(glyph_id as u16))?;

        Some(GlyphBBox {
            x_min: bbox.x_min as f32 * scale,
            y_min: bbox.y_min as f32 * scale,
            x_max: bbox.x_max as f32 * scale,
            y_max: bbox.y_max as f32 * scale,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct GlyphBBox {
    pub x_min: f32,
    pub y_min: f32,
    pub x_max: f32,
    pub y_max: f32,
}
