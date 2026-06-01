use fontdb::{Database, Family, Query, Weight};
use std::path::Path;
use thiserror::Error;

/// Errors that can occur during font operations.
#[derive(Error, Debug)]
pub enum FontError {
    /// No font matched the query criteria.
    #[error("Font not found: {0}")]
    NotFound(String),
    /// Failed to load font file or data.
    #[error("Font load error: {0}")]
    LoadError(String),
    /// Failed to parse font data (invalid or unsupported format).
    #[error("Font parse error: {0}")]
    ParseError(String),
}

/// Metadata about a loaded font face.
#[derive(Debug, Clone)]
pub struct FontInfo {
    /// Unique identifier in the font database.
    pub id: fontdb::ID,
    /// Primary font family name (e.g. "Arial", "Noto Sans CJK").
    pub family: String,
    /// Style description (e.g. "Normal", "Italic").
    pub style: String,
    /// Weight value (100–900, where 400=normal, 700=bold).
    pub weight: u16,
    /// Whether this face is italic.
    pub italic: bool,
    /// Whether this face is monospaced.
    pub monospace: bool,
}

/// Font database manager for loading, querying, and retrieving font data.
///
/// Uses [`fontdb`](https://docs.rs/fontdb) internally. Supports system fonts,
/// font files (TTF/OTF/WOFF2), and in-memory font data (for ASS embedded fonts).
///
/// # Query cascade
///
/// [`query_with_fallback`](Self::query_with_fallback) tries these levels in order:
/// 1. Exact match (family + weight + italic) via scoring
/// 2. Scoring-based best match from all loaded fonts
/// 3. Liberation Sans → DejaVu Sans → Noto Sans → Arial → Helvetica
/// 4. Any available font (last resort)
pub struct FontManager {
    db: Database,
}

impl FontManager {
    /// Creates an empty font manager with no loaded fonts.
    pub fn new() -> Self {
        Self {
            db: Database::new(),
        }
    }

    /// Loads all system-installed fonts. May be slow on first call.
    pub fn load_system_fonts(&mut self) {
        self.db.load_system_fonts();
    }

    /// Loads a font file from disk (TTF, OTF, WOFF2).
    ///
    /// # Errors
    ///
    /// Returns [`FontError::LoadError`] if the file cannot be read or contains no valid faces.
    pub fn load_font_file(&mut self, path: &Path) -> Result<fontdb::ID, FontError> {
        self.db
            .load_font_file(path)
            .map_err(|e| FontError::LoadError(e.to_string()))?;
        let id = self
            .db
            .faces()
            .last()
            .map(|f| f.id)
            .ok_or_else(|| FontError::LoadError("No face loaded".into()))?;
        Ok(id)
    }

    /// Loads font data from memory (e.g. ASS embedded fonts).
    ///
    /// Returns the font ID, or [`fontdb::ID::dummy()`] if no face was loaded.
    pub fn load_font_data(&mut self, data: Vec<u8>) -> fontdb::ID {
        self.db.load_font_data(data);
        self.db
            .faces()
            .last()
            .map(|f| f.id)
            .unwrap_or_else(|| fontdb::ID::dummy())
    }

    /// Queries a font by family name, bold, and italic flags using fontdb's
    /// built-in matching. Falls back to SansSerif if the family is not found.
    pub fn query(&self, family: &str, bold: bool, italic: bool) -> Option<fontdb::ID> {
        let weight = if bold { 700 } else { 400 };
        let style = if italic {
            fontdb::Style::Italic
        } else {
            fontdb::Style::Normal
        };
        let query = Query {
            families: &[Family::Name(family), Family::SansSerif],
            weight: Weight(weight),
            style,
            ..Default::default()
        };
        self.db.query(&query)
    }

    /// Queries a font using a scoring algorithm that considers weight difference,
    /// italic match, and family name. Returns the best-scoring font from all loaded faces.
    pub fn query_with_score(&self, family: &str, bold: bool, italic: bool) -> Option<fontdb::ID> {
        let target_weight: u16 = if bold { 700 } else { 400 };
        let target_italic = italic;

        let mut best: Option<(fontdb::ID, f32)> = None;

        for face in self.db.faces() {
            let face_family = face
                .families
                .first()
                .map(|(s, _)| s.as_str())
                .unwrap_or("");
            let weight_diff = (face.weight.0 as f32 - target_weight as f32).abs();
            let italic_penalty = if target_italic != (face.style == fontdb::Style::Italic) {
                100.0
            } else {
                0.0
            };
            let family_bonus = if face_family.eq_ignore_ascii_case(family) {
                0.0
            } else if face_family.eq_ignore_ascii_case("sans-serif") {
                50.0
            } else {
                200.0
            };
            let score = weight_diff + italic_penalty + family_bonus;
            if best.map_or(true, |(_, bs)| score < bs) {
                best = Some((face.id, score));
            }
        }
        best.map(|(id, _)| id)
    }

    /// Queries a font with a 6-level fallback cascade. First tries `query_with_score`,
    /// then falls back through Liberation Sans → DejaVu Sans → Noto Sans → Arial → Helvetica,
    /// and finally returns any available font as a last resort.
    pub fn query_with_fallback(&self, family: &str, bold: bool, italic: bool) -> Option<fontdb::ID> {
        if let Some(id) = self.query_with_score(family, bold, italic) {
            return Some(id);
        }
        let fallbacks = [
            "Liberation Sans",
            "DejaVu Sans",
            "Noto Sans",
            "Arial",
            "Helvetica",
        ];
        for fb in &fallbacks {
            if let Some(id) = self.query(fb, bold, italic) {
                return Some(id);
            }
        }
        self.db.faces().next().map(|f| f.id)
    }

    /// Returns the raw font data (TTF/OTF bytes) for the given font ID.
    pub fn get_font_data(&self, id: fontdb::ID) -> Option<Vec<u8>> {
        self.db.with_face_data(id, |data, _index| data.to_vec())
    }

    pub fn font_count(&self) -> usize {
        self.db.faces().count()
    }

    pub fn list_fonts(&self) -> Vec<FontInfo> {
        self.db
            .faces()
            .map(|face| FontInfo {
                id: face.id,
                family: face
                    .families
                    .first()
                    .map(|(s, _)| s.clone())
                    .unwrap_or_default(),
                style: format!("{:?}", face.style),
                weight: face.weight.0,
                italic: face.style == fontdb::Style::Italic,
                monospace: face.monospaced,
            })
            .collect()
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}
