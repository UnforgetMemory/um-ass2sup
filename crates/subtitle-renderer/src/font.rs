use fontdb::{Database, Family, Query, Weight};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FontError {
    #[error("Font not found: {0}")]
    NotFound(String),
    #[error("Font load error: {0}")]
    LoadError(String),
    #[error("Font parse error: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone)]
pub struct FontInfo {
    pub id: fontdb::ID,
    pub family: String,
    pub style: String,
    pub weight: u16,
    pub italic: bool,
    pub monospace: bool,
}

pub struct FontManager {
    db: Database,
}

impl FontManager {
    pub fn new() -> Self {
        Self {
            db: Database::new(),
        }
    }

    pub fn load_system_fonts(&mut self) {
        self.db.load_system_fonts();
    }

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

    pub fn load_font_data(&mut self, data: Vec<u8>) -> fontdb::ID {
        self.db.load_font_data(data);
        self.db
            .faces()
            .last()
            .map(|f| f.id)
            .unwrap_or_else(|| fontdb::ID::dummy())
    }

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
