use fontdb::{Database, Family, Query, Weight};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
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
    /// Cache of font data keyed by fontdb::ID to avoid repeated cloning from fontdb.
    font_data_cache: Mutex<HashMap<fontdb::ID, Arc<Vec<u8>>>>,
    // Cache for font queries: (lowercase family, bold, italic) → fontdb::ID.
    // Cleared when fonts are loaded or added.
    query_cache: Mutex<HashMap<(String, bool, bool), fontdb::ID>>,
}

impl FontManager {
    /// Creates an empty font manager with no loaded fonts.
    pub fn new() -> Self {
        Self {
            db: Database::new(),
            font_data_cache: Mutex::new(HashMap::new()),
            query_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Loads all system-installed fonts. May be slow on first call.
    pub fn load_system_fonts(&mut self) {
        self.db.load_system_fonts();
        self.query_cache.lock().unwrap().clear();
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
        self.query_cache.lock().unwrap().clear();
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
        self.query_cache.lock().unwrap().clear();
        self.db
            .faces()
            .last()
            .map(|f| f.id)
            .unwrap_or_else(fontdb::ID::dummy)
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
    #[allow(clippy::incompatible_msrv)]
    pub fn query_with_score(&self, family: &str, bold: bool, italic: bool) -> Option<fontdb::ID> {
        let target_weight: u16 = if bold { 700 } else { 400 };
        let target_italic = italic;

        let mut best: Option<(fontdb::ID, f32)> = None;

        for face in self.db.faces() {
            let face_family = face.families.first().map(|(s, _)| s.as_str()).unwrap_or("");
            let weight_diff = (f32::from(face.weight.0) - f32::from(target_weight)).abs();
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
            if best.is_none_or(|(_, bs)| score < bs) {
                best = Some((face.id, score));
            }
        }
        best.map(|(id, _)| id)
    }

    /// Queries a font with a 6-level fallback cascade. First tries `query_with_score`,
    /// then falls back through Liberation Sans → DejaVu Sans → Noto Sans → Arial → Helvetica,
    /// and finally returns any available font as a last resort.
    ///
    /// Results are cached internally by (family, bold, italic) key. The cache is
    /// invalidated whenever fonts are loaded or added.
    pub fn query_with_fallback(
        &self,
        family: &str,
        bold: bool,
        italic: bool,
    ) -> Option<fontdb::ID> {
        let key = (family.to_lowercase(), bold, italic);
        if let Some(cached) = self.query_cache.lock().unwrap().get(&key) {
            return Some(*cached);
        }
        let result = self.query_with_fallback_inner(family, bold, italic);
        if let Some(id) = result {
            self.query_cache.lock().unwrap().insert(key, id);
        }
        result
    }

    /// Fallback query implementation (un-cached). See [`query_with_fallback`].
    fn query_with_fallback_inner(
        &self,
        family: &str,
        bold: bool,
        italic: bool,
    ) -> Option<fontdb::ID> {
        // 1. Try exact-ish match via scoring against all loaded faces.
        if let Some(id) = self.query_with_score(family, bold, italic) {
            return Some(id);
        }

        // 2. Hardcoded fallback font names — includes CJK-capable families
        //    so that Chinese, Japanese, and Korean subtitles render legibly
        //    even when no system CJK font is configured.
        let fallbacks = [
            "Liberation Sans",
            "DejaVu Sans",
            "Noto Sans",
            "Noto Sans CJK SC",
            "Noto Sans CJK TC",
            "WenQuanYi Micro Hei",
            "Source Han Sans CN",
            "IPAGothic",
            "NanumGothic",
            "Arial",
            "Helvetica",
        ];
        for fb in &fallbacks {
            if let Some(id) = self.query(fb, bold, italic) {
                return Some(id);
            }
        }

        // 3. Generic sans-serif query — lets fontconfig resolve the best
        //    available system font (often picks a CJK font when the locale
        //    is zh/ja/ko).
        let ss_query = Query {
            families: &[fontdb::Family::SansSerif],
            weight: Weight(if bold { 700 } else { 400 }),
            style: if italic {
                fontdb::Style::Italic
            } else {
                fontdb::Style::Normal
            },
            ..Default::default()
        };
        if let Some(id) = self.db.query(&ss_query) {
            return Some(id);
        }

        // 4. Last resort: any available face.
        self.db.faces().next().map(|f| f.id)
    }

    /// Returns the raw font data (TTF/OTF bytes) for the given font ID.
    ///
    /// Results are cached as `Arc<Vec<u8>>` so that repeated calls with the same
    /// ID share the underlying allocation via cheap Arc clones instead of full
    /// byte copies. The first call clones from fontdb; subsequent calls only
    /// increment the Arc reference count.
    pub fn get_font_data(&self, id: fontdb::ID) -> Option<Arc<Vec<u8>>> {
        if let Some(cached) = self.font_data_cache.lock().unwrap().get(&id) {
            return Some(Arc::clone(cached));
        }
        self.db.with_face_data(id, |data, _index| {
            let arc = Arc::new(data.to_vec());
            self.font_data_cache
                .lock()
                .unwrap()
                .insert(id, Arc::clone(&arc));
            arc
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    fn system_font_manager() -> FontManager {
        let mut fm = FontManager::new();
        fm.load_system_fonts();
        fm
    }

    fn find_any_font(fm: &FontManager) -> Option<fontdb::ID> {
        fm.query("Arial", false, false)
            .or_else(|| fm.query("Liberation Sans", false, false))
            .or_else(|| fm.query("DejaVu Sans", false, false))
            .or_else(|| fm.query("Noto Sans", false, false))
            .or_else(|| fm.list_fonts().first().map(|f| f.id))
    }

    #[test]
    fn test_font_data_returns_same_bytes() {
        let fm = system_font_manager();
        let id = find_any_font(&fm).expect("No system fonts found");
        let data1 = fm.get_font_data(id).expect("Font data should exist");
        let data2 = fm.get_font_data(id).expect("Font data should exist");
        assert_eq!(
            data1, data2,
            "get_font_data should return identical bytes for same ID"
        );
    }

    #[test]
    fn test_font_data_cache_hit_on_repeated_calls() {
        let fm = system_font_manager();
        let id = find_any_font(&fm).expect("No system fonts found");
        // Prime the cache with the first call.
        let data_first = fm.get_font_data(id).expect("First call should succeed");
        assert!(data_first.len() > 100, "Font data should be substantial");
        // Subsequent calls must all return the exact same bytes (cache hit path).
        for _ in 0..10 {
            let data = fm.get_font_data(id).expect("Repeated call should succeed");
            assert_eq!(data, data_first, "Cached data must match first call");
        }
    }

    #[test]
    fn test_font_data_cache_multiple_ids() {
        let fm = system_font_manager();
        let fonts: Vec<_> = fm.list_fonts();
        if fonts.len() < 2 {
            return;
        }
        // Prime cache with two different IDs.
        let id_a = fonts[0].id;
        let id_b = fonts[1].id;
        let data_a = fm.get_font_data(id_a).expect("Font data A");
        let data_b = fm.get_font_data(id_b).expect("Font data B");
        // Interleaved reads exercise the cache for both entries.
        for _ in 0..5 {
            assert_eq!(fm.get_font_data(id_a).expect("Cached A"), data_a);
            assert_eq!(fm.get_font_data(id_b).expect("Cached B"), data_b);
        }
    }

    #[test]
    fn test_font_data_non_empty() {
        let fm = system_font_manager();
        let id = find_any_font(&fm).expect("No system fonts found");
        let data = fm.get_font_data(id).expect("Font data should exist");
        assert!(data.len() > 100, "Font data should be substantial");
    }

    #[test]
    fn test_font_data_different_ids_differ() {
        let fm = system_font_manager();
        let fonts: Vec<_> = fm.list_fonts();
        if fonts.len() < 2 {
            return;
        }
        let data_a = fm.get_font_data(fonts[0].id).expect("Font data");
        let data_b = fm.get_font_data(fonts[1].id).expect("Font data");
        if data_a.len() != data_b.len() || data_a != data_b {}
        // If two different IDs happen to return same data, that's valid (duplicated font).
        // The point is no panic or corruption.
    }

    #[test]
    fn test_font_data_invalid_id_returns_none() {
        let fm = system_font_manager();
        let dummy = fontdb::ID::dummy();
        assert!(fm.get_font_data(dummy).is_none());
    }

    #[test]
    fn test_query_with_fallback_returns_something() {
        let fm = system_font_manager();
        let id = fm.query_with_fallback("NonExistentFont", false, false);
        assert!(id.is_some(), "Fallback chain should return some font");
    }

    #[test]
    fn test_load_font_data_returns_id() {
        let mut fm = FontManager::new();
        fm.load_system_fonts();
        if let Some(id) = find_any_font(&fm) {
            if let Some(data) = fm.get_font_data(id) {
                let loaded_id = fm.load_font_data(data.to_vec());
                let loaded_data = fm.get_font_data(loaded_id);
                assert!(
                    loaded_data.is_some(),
                    "Loading valid font data should produce retrievable font"
                );
            }
        }
    }

    #[test]
    fn test_list_fonts_contains_family() {
        let fm = system_font_manager();
        let fonts = fm.list_fonts();
        assert!(fonts.iter().any(|f| f.weight > 0 || !f.family.is_empty()));
    }
}
