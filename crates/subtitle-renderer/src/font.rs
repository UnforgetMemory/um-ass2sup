use fontdb::{Database, Family, Query};
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
        Self { db: Database::new() }
    }

    pub fn load_system_fonts(&mut self) {
        self.db.load_system_fonts();
    }

    pub fn load_font_file(&mut self, path: &Path) -> Result<fontdb::ID, FontError> {
        self.db
            .load_font_file(path)
            .map_err(|e| FontError::LoadError(e.to_string()))?;
        let id = self.db.faces().last().map(|f| f.id).ok_or_else(|| FontError::LoadError("No face loaded".into()))?;
        Ok(id)
    }

    pub fn load_font_data(&mut self, data: Vec<u8>) -> fontdb::ID {
        self.db.load_font_data(data);
        self.db.faces().last().map(|f| f.id).unwrap_or_else(|| fontdb::ID::dummy())
    }

    pub fn query(&self, family: &str, bold: bool, italic: bool) -> Option<fontdb::ID> {
        let weight = if bold { 700 } else { 400 };
        let style = if italic { fontdb::Style::Italic } else { fontdb::Style::Normal };
        let query = Query {
            families: &[Family::Name(family), Family::SansSerif],
            weight: fontdb::Weight(weight),
            style,
            ..Default::default()
        };
        self.db.query(&query)
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
                family: face.families.first().map(|(s, _)| s.clone()).unwrap_or_default(),
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
