use std::path::Path;
use std::sync::Arc;

use crate::font::error::FontError;
use crate::font::telemetry::{FontEvent, FontTelemetry};
use crate::font::types::{FontFace, FontId, FontStretch, FontStyle, FontWeight};

/// Font database — stores loaded font data and parsed metadata.
pub struct FontDatabase {
    entries: Vec<FontEntry>,
    next_id: u32,
    telemetry: FontTelemetry,
}

struct FontEntry {
    id: FontId,
    data: Arc<[u8]>,
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
            telemetry: FontTelemetry::new(),
        }
    }

    /// Load a single font file, registers every face (all TTC faces), returns
    /// the [`FontId`] of the first face.
    pub fn load_font_file(&mut self, path: &Path, is_system: bool) -> Result<FontId, FontError> {
        let data: Arc<[u8]> = std::fs::read(path)
            .map(Arc::from)
            .map_err(|e| FontError::Io {
                path: path.to_path_buf(),
                source: e,
            })?;
        let path_str = path.to_string_lossy().into_owned();
        let faces = parse_font_metadata(self.next_id.into(), &data, Some(path_str), is_system)?;
        Ok(self.register_faces(faces, data))
    }

    /// Load font data from bytes (e.g., embedded fonts), registers every face
    /// (all TTC faces), returns the [`FontId`] of the first face.
    pub fn load_font_data(&mut self, data: Vec<u8>, is_system: bool) -> Result<FontId, FontError> {
        let data: Arc<[u8]> = Arc::from(data);
        let faces = parse_font_metadata(self.next_id.into(), &data, None, is_system)?;
        Ok(self.register_faces(faces, data))
    }

    /// Push every parsed face as its own entry, assign consecutive ids, and
    /// return the id of the first face. The underlying font bytes are shared
    /// between faces via [`Arc`] clones.
    fn register_faces(&mut self, faces: Vec<FontFace>, data: Arc<[u8]>) -> FontId {
        let first_id = faces[0].id;
        let face_count = faces.len();
        for face in faces {
            self.entries.push(FontEntry {
                id: face.id,
                data: Arc::clone(&data),
                face,
            });
        }
        self.next_id += face_count as u32;
        first_id
    }

    /// Recursively load all fonts from a directory.
    pub fn load_fonts_dir(&mut self, dir: &Path, is_system: bool) -> usize {
        let mut count = 0;
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    count += self.load_fonts_dir(&path, is_system);
                } else if is_font_file(&path) {
                    match self.load_font_file(&path, is_system) {
                        Ok(_) => count += 1,
                        Err(e) => {
                            // Record the skipped file so corruption is observable
                            // instead of being silently dropped.
                            self.telemetry.record(FontEvent::Corrupted {
                                path: path.to_string_lossy().into_owned(),
                                reason: e.to_string(),
                                recoverable: true,
                            });
                        }
                    }
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
            .map(|e| e.data.as_ref())
    }

    /// Get raw font data by id as a cheaply-cloneable `Arc`.
    ///
    /// Render hot paths clone the `Arc` (one refcount bump) instead of copying
    /// the whole font file (CJK fonts are 10–40 MB).
    pub fn get_data_arc(&self, id: FontId) -> Option<Arc<[u8]>> {
        self.entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| Arc::clone(&e.data))
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

    /// Read-only access to font-telemetry events recorded during directory scans.
    ///
    /// Corrupted font files that are skipped during [`Self::load_fonts_dir`] are
    /// recorded here as [`FontEvent::Corrupted`], making previously silent
    /// drops observable.
    pub fn telemetry(&self) -> &FontTelemetry {
        &self.telemetry
    }
}

fn is_font_file(path: &Path) -> bool {
    if let Some(ext) = path.extension() {
        let ext = ext.to_string_lossy().to_lowercase();
        matches!(ext.as_str(), "ttf" | "otf" | "ttc" | "otc")
    } else {
        false
    }
}

/// Parse font metadata for every face in the data using swash.
///
/// TrueType Collections (`ttcf` magic, e.g. `.ttc`/`.otc`) are fully
/// enumerated: every face is parsed and returned with a consecutive
/// [`FontId`] starting at `id`, instead of silently dropping faces after the
/// first. The face count is read from the TTC header (big-endian u32 at
/// offset 8) and per-face offsets from offset 12 onward — both via swash's
/// checked [`swash::FontDataRef`] reader, so malformed collections surface as
/// [`FontError::Corrupted`] rather than panicking.
fn parse_font_metadata(
    id: FontId,
    data: &[u8],
    path: Option<String>,
    is_system: bool,
) -> Result<Vec<FontFace>, FontError> {
    let font_data = swash::FontDataRef::new(data).ok_or_else(|| FontError::Corrupted {
        path: path.clone().unwrap_or_default().into(),
        reason: "swash: could not parse font data".into(),
    })?;

    let num_faces = font_data.len();
    if num_faces == 0 {
        return Err(FontError::Corrupted {
            path: path.clone().unwrap_or_default().into(),
            reason: "swash: font data contains no faces".into(),
        });
    }

    let mut faces = Vec::with_capacity(num_faces);
    for index in 0..num_faces {
        let font = font_data.get(index).ok_or_else(|| FontError::Corrupted {
            path: path.clone().unwrap_or_default().into(),
            reason: format!("swash: could not parse font face at index {index}"),
        })?;
        faces.push(build_face(
            FontId(id.0 + index as u32),
            font,
            path.clone(),
            is_system,
        ));
    }
    Ok(faces)
}

/// Extract a [`FontFace`] from an already-parsed swash font.
fn build_face(
    id: FontId,
    font: swash::FontRef<'_>,
    path: Option<String>,
    is_system: bool,
) -> FontFace {
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

    FontFace {
        id,
        family,
        weight,
        style,
        stretch,
        path,
        is_system,
        cjk,
    }
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
            assert_eq!(db.len(), 1, "a single TTF must register exactly one face");
        }
    }

    /// Build a synthetic TrueType Collection wrapping `font_bytes` `num_faces` times.
    ///
    /// Per the OpenType spec, table offsets in a collection are measured from
    /// the beginning of the TTC file, so each embedded copy's table-directory
    /// offsets are rewritten to be file-absolute.
    fn synthetic_ttc(font_bytes: &[u8], num_faces: usize) -> Vec<u8> {
        let header_len = 12 + num_faces * 4;
        let face_len = font_bytes.len();
        let mut data = Vec::with_capacity(header_len + face_len * num_faces);
        data.extend_from_slice(b"ttcf");
        data.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        data.extend_from_slice(&(num_faces as u32).to_be_bytes());
        let mut face_start = header_len as u32;
        for _ in 0..num_faces {
            data.extend_from_slice(&face_start.to_be_bytes());
            face_start += face_len as u32;
        }
        for face_idx in 0..num_faces {
            let face_offset = (header_len + face_idx * face_len) as u32;
            let mut face = font_bytes.to_vec();
            let num_tables = u16::from_be_bytes([face[4], face[5]]) as usize;
            for i in 0..num_tables {
                let rec = 12 + i * 16;
                let table_offset = u32::from_be_bytes([
                    face[rec + 8],
                    face[rec + 9],
                    face[rec + 10],
                    face[rec + 11],
                ]);
                face[rec + 8..rec + 12]
                    .copy_from_slice(&(table_offset + face_offset).to_be_bytes());
            }
            data.extend_from_slice(&face);
        }
        data
    }

    #[test]
    fn is_font_file_rejects_woff_and_woff2() {
        assert!(
            !is_font_file(Path::new("font.woff")),
            ".woff must be rejected"
        );
        assert!(
            !is_font_file(Path::new("font.woff2")),
            ".woff2 must be rejected"
        );
        assert!(
            !is_font_file(Path::new("FONT.WOFF")),
            "uppercase .WOFF must be rejected"
        );
        // Unchanged acceptance:
        assert!(is_font_file(Path::new("font.ttf")));
        assert!(is_font_file(Path::new("font.otf")));
        assert!(is_font_file(Path::new("font.ttc")));
        assert!(is_font_file(Path::new("font.otc")));
    }

    #[test]
    fn load_ttc_registers_all_faces() {
        let ttf_path = PathBuf::from("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf");
        if !ttf_path.exists() {
            eprintln!("SKIP: DejaVuSans.ttf not present");
            return;
        }
        let font_bytes = std::fs::read(&ttf_path).expect("Failed to read test font");
        let mut db = FontDatabase::new();
        let id = db
            .load_font_data(synthetic_ttc(&font_bytes, 2), false)
            .expect("synthetic TTC should load");
        assert_eq!(db.len(), 2, "both TTC faces must be registered");
        let face0 = db.get_face(id).expect("first face");
        let face1 = db.get_face(FontId(id.0 + 1)).expect("second face");
        assert_eq!(face0.family, "DejaVu Sans");
        assert_eq!(face1.family, "DejaVu Sans");
        assert_eq!(face0.weight, face1.weight);
    }

    #[test]
    fn load_fonts_dir_records_corrupted_font() {
        let dir = std::env::temp_dir().join(format!("ass2sup-telemetry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("create temp dir");
        std::fs::write(dir.join("broken.ttf"), vec![0x00, 0x01, 0x02, 0x03])
            .expect("write broken font");

        let mut db = FontDatabase::new();
        db.load_fonts_dir(&dir, false);

        let corrupted: Vec<_> = db
            .telemetry()
            .events()
            .iter()
            .filter(|e| matches!(e, FontEvent::Corrupted { .. }))
            .collect();
        assert_eq!(
            corrupted.len(),
            1,
            "broken font must be recorded exactly once in telemetry"
        );
        assert_eq!(db.len(), 0, "corrupted font must not be registered");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_fonts_dir_valid_fonts_record_no_corruption() {
        let dir = PathBuf::from("/usr/share/fonts/truetype/dejavu");
        if !dir.exists() {
            eprintln!("SKIP: dejavu dir not present");
            return;
        }
        let mut db = FontDatabase::new();
        let count = db.load_fonts_dir(&dir, true);
        assert!(count > 0, "expected >0 fonts loaded");
        let corrupted = db
            .telemetry()
            .events()
            .iter()
            .filter(|e| matches!(e, FontEvent::Corrupted { .. }))
            .count();
        assert_eq!(corrupted, 0, "valid font dir must not record corruption");
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
            if PathBuf::from(path).exists()
                && let Ok(id) = db.load_font_file(&PathBuf::from(path), true)
            {
                let face = db.get_face(id).unwrap();
                assert!(face.cjk, "Expected CJK font to have cjk=true for {}", path);
                found = true;
                break;
            }
        }
        if !found {
            eprintln!("SKIP: No CJK font found on system for cjk detection test");
        }
    }
}
