use std::path::Path;

use crate::font::error::FontError;
use crate::font::types::{FontFace, FontId, FontStretch, FontStyle, FontWeight};

/// Font database — stores loaded font data and parsed metadata.
pub struct FontDatabase {
    entries: Vec<FontEntry>,
    next_id: u32,
}

struct FontEntry {
    id: FontId,
    data: Vec<u8>,
    face: FontFace,
}

impl Default for FontDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl FontDatabase {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            next_id: 0,
        }
    }

    /// Load a single font file, returns the FontId.
    pub fn load_font_file(&mut self, path: &Path, is_system: bool) -> Result<FontId, FontError> {
        let data = std::fs::read(path).map_err(|e| FontError::Io {
            path: path.to_path_buf(),
            source: e,
        })?;
        let path_str = path.to_string_lossy().into_owned();
        let face = parse_font_metadata(self.next_id.into(), &data, Some(path_str), is_system)?;
        let id = face.id;
        self.entries.push(FontEntry { id, data, face });
        self.next_id += 1;
        Ok(id)
    }

    /// Load font data from bytes (e.g., embedded fonts).
    pub fn load_font_data(&mut self, data: Vec<u8>, is_system: bool) -> Result<FontId, FontError> {
        let face = parse_font_metadata(self.next_id.into(), &data, None, is_system)?;
        let id = face.id;
        self.entries.push(FontEntry { id, data, face });
        self.next_id += 1;
        Ok(id)
    }

    /// Recursively load all fonts from a directory.
    pub fn load_fonts_dir(&mut self, dir: &Path, is_system: bool) -> usize {
        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    count += self.load_fonts_dir(&path, is_system);
                } else if is_font_file(&path) && self.load_font_file(&path, is_system).is_ok() {
                    count += 1;
                }
            }
        }
        count
    }

    /// Get raw font data by id.
    pub fn get_data(&self, id: FontId) -> Option<&[u8]> {
        self.entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.data.as_slice())
    }

    /// Get FontFace metadata by id.
    pub fn get_face(&self, id: FontId) -> Option<&FontFace> {
        self.entries.iter().find(|e| e.id == id).map(|e| &e.face)
    }

    /// Number of loaded fonts.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the database is empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterate all loaded FontFace metadata.
    pub fn faces(&self) -> impl Iterator<Item = &FontFace> {
        self.entries.iter().map(|e| &e.face)
    }
}

fn is_font_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        matches!(
            ext.as_str(),
            "ttf" | "otf" | "ttc" | "otc" | "woff" | "woff2"
        )
    } else {
        false
    }
}

/// Parse font metadata from raw bytes using swash.
fn parse_font_metadata(
    id: FontId,
    data: &[u8],
    path: Option<String>,
    is_system: bool,
) -> Result<FontFace, FontError> {
    let font = swash::FontRef::from_index(data, 0).ok_or_else(|| FontError::Corrupted {
        path: path.clone().unwrap_or_default().into(),
        reason: "swash: could not parse font data".into(),
    })?;

    // Collect ALL family names (primary + typographic/legacy)
    let mut families: Vec<String> = Vec::new();
    for s in font.localized_strings() {
        if s.id() == swash::StringId::Family {
            let name = s.to_string();
            if !name.is_empty() && !families.contains(&name) {
                families.push(name);
            }
        }
    }
    if families.is_empty() {
        families.push("Unknown".to_string());
    }

    // Use the FIRST family name as the primary family (for compatibility)
    let family = families[0].clone();

    let weight = FontWeight::from_u16(font.attributes().weight().0);

    let style = match font.attributes().style() {
        swash::Style::Italic | swash::Style::Oblique(_) => FontStyle::Italic,
        _ => FontStyle::Normal,
    };

    let stretch = if font.attributes().stretch().is_condensed() {
        FontStretch::Condensed
    } else if font.attributes().stretch().is_expanded() {
        FontStretch::Expanded
    } else {
        FontStretch::Normal
    };

    // CJK detection: check if U+4E2D (中) exists
    let cjk = font.charmap().map('\u{4E2D}') != 0;

    Ok(FontFace {
        id,
        family,
        weight,
        style,
        stretch,
        path,
        is_system,
        cjk,
        corrupt: false,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn load_font_file_with_valid_ttf_returns_ok() {
        let mut db = FontDatabase::new();
        let path = PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf");
        if path.exists() {
            let id = db.load_font_file(&path, false);
            assert!(
                id.is_ok(),
                "Expected Ok(FontId) for valid TTF, got: {:?}",
                id
            );
        }
    }

    #[test]
    fn load_font_file_with_nonexistent_file_returns_err() {
        let mut db = FontDatabase::new();
        let path = PathBuf::from("/nonexistent/font.ttf");
        let result = db.load_font_file(&path, false);
        assert!(result.is_err(), "Expected Err for nonexistent file");
    }

    #[test]
    fn load_font_data_with_valid_bytes_succeeds() {
        let mut db = FontDatabase::new();
        let path = PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf");
        if path.exists() {
            let data = std::fs::read(&path).expect("Failed to read test font");
            let id = db.load_font_data(data, false);
            assert!(
                id.is_ok(),
                "Expected Ok(FontId) for valid bytes, got: {:?}",
                id
            );
        }
    }

    #[test]
    fn load_fonts_dir_with_fonts_returns_positive_count() {
        let mut db = FontDatabase::new();
        let dir = PathBuf::from("/usr/share/fonts/truetype/dejavu");
        if dir.exists() && dir.is_dir() {
            let count = db.load_fonts_dir(&dir, true);
            assert!(
                count > 0,
                "Expected >0 fonts loaded from directory, got {}",
                count
            );
        }
    }

    #[test]
    fn corrupted_font_returns_corrupted_error() {
        let mut db = FontDatabase::new();
        let result = db.load_font_data(vec![0x00, 0x01, 0x02, 0x03], false);
        assert!(result.is_err(), "Expected Err for corrupted font data");
        match result.unwrap_err() {
            FontError::Corrupted { .. } => {}
            _ => panic!("Expected FontError::Corrupted"),
        }
    }

    #[test]
    fn font_face_metadata_extracted_correctly() {
        let mut db = FontDatabase::new();
        let path = PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf");
        if path.exists() {
            let id = db
                .load_font_file(&path, false)
                .expect("Failed to load font");
            let face = db.get_face(id).expect("Font face not found");
            assert_eq!(face.family, "DejaVu Sans");
            assert_eq!(face.weight, FontWeight::Normal);
            assert_eq!(face.style, FontStyle::Normal);
            assert_eq!(face.stretch, FontStretch::Normal);
            assert!(!face.cjk);
        }
    }

    #[test]
    fn cjk_detection_returns_true_for_cjk_font() {
        let mut db = FontDatabase::new();
        let cjk_paths = [
            "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
            "/usr/share/fonts/truetype/wqy/wqy-zenhei.ttc",
        ];
        let mut found = false;
        for path in &cjk_paths {
            if PathBuf::from(path).exists() {
                if let Ok(id) = db.load_font_file(&PathBuf::from(path), true) {
                    let face = db.get_face(id).unwrap();
                    assert!(face.cjk, "Expected CJK font to have cjk=true for {}", path);
                    found = true;
                    break;
                }
            }
        }
        if !found {
            eprintln!("SKIP: No CJK font found on system for cjk detection test");
        }
    }
}
