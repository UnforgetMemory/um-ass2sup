use cosmic_text::{Attrs, Buffer, Family, Metrics, Shaping};
use fontdb::ID;

/// A single shaped glyph with metrics, produced by CosmicShaper.
#[derive(Debug, Clone)]
pub struct CosmicShapedGlyph {
    pub glyph_id: u16,
    pub x_advance: f32,
    pub y_advance: f32,
    pub x_offset: f32,
    pub y_offset: f32,
    pub font_id: ID, // per-glyph! supports font fallback
}

/// Shaper using cosmic-text Buffer for text shaping.
pub struct CosmicShaper;

impl CosmicShaper {
    /// Shape a text run using cosmic-text Buffer.
    ///
    /// `font_name` is the CSS font family (e.g. "Arial", "Noto Sans CJK SC").
    /// Pass an empty string to use cosmic-text's default font.
    /// `bold` and `italic` control font weight/style selection.
    ///
    /// Returns positioned glyphs with per-glyph `font_id` for fallback support.
    pub fn shape(
        text: &str,
        font_system: &mut cosmic_text::FontSystem,
        font_size: f32,
        font_name: &str,
        bold: bool,
        italic: bool,
    ) -> Vec<CosmicShapedGlyph> {
        if text.is_empty() {
            return vec![];
        }

        let metrics = Metrics::new(font_size, font_size * 1.2);

        let attrs = if font_name.is_empty() {
            Attrs::new()
                .weight(if bold {
                    cosmic_text::Weight::BOLD
                } else {
                    cosmic_text::Weight::NORMAL
                })
                .style(if italic {
                    cosmic_text::Style::Italic
                } else {
                    cosmic_text::Style::Normal
                })
        } else {
            Attrs::new()
                .family(Family::Name(font_name))
                .weight(if bold {
                    cosmic_text::Weight::BOLD
                } else {
                    cosmic_text::Weight::NORMAL
                })
                .style(if italic {
                    cosmic_text::Style::Italic
                } else {
                    cosmic_text::Style::Normal
                })
        };

        let mut buffer = Buffer::new(font_system, metrics);
        buffer.set_text(text, &attrs, Shaping::Advanced, None);
        buffer.shape_until_scroll(font_system, true);

        let mut glyphs = Vec::new();

        for run in buffer.layout_runs() {
            for glyph in run.glyphs.iter() {
                glyphs.push(CosmicShapedGlyph {
                    glyph_id: glyph.glyph_id,
                    x_advance: glyph.w,
                    y_advance: 0.0,
                    x_offset: glyph.x_offset,
                    y_offset: glyph.y_offset,
                    font_id: glyph.font_id,
                });
            }
        }

        glyphs
    }
}
