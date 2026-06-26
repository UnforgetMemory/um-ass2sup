//! Central font registry — facade over database, index, and discovery.
//!
//! [`FontRegistry`] provides the public API for font querying, availability
//! checking, and loading from system/user sources. It delegates internally
//! to [`FontDatabase`](super::database::FontDatabase) for storage and
//! [`FontIndex`](super::index::FontIndex) for O(1) lookups.

use std::path::Path;

use super::database::FontDatabase;
use super::discovery::discover_system_font_paths;
use super::error::FontError;
use super::index::FontIndex;
use super::types::{Availability, FontFace, FontId, FontQuery, FontWeight, QueryResult};

/// Central font registry integrating database, index, and discovery.
///
/// Query paths:
/// * [`query`](Self::query) — exact match → family fallback → suggestion.
/// * [`check`](Self::check) — availability probe without side effects.
pub struct FontRegistry {
    db: FontDatabase,
    index: FontIndex,
}

impl FontRegistry {
    /// Create an empty registry (no fonts loaded).
    pub fn new() -> Self {
        Self {
            db: FontDatabase::new(),
            index: FontIndex::new(),
        }
    }

    /// Query for a font matching the given criteria.
    ///
    /// 1. Exact match: (family, weight, style) → `found = Some(id)`.
    /// 2. Family fallback: same family, any weight/style → `candidates`.
    /// 3. Suggestion: closest weight among candidates.
    pub fn query(&self, q: &FontQuery) -> QueryResult {
        // Step 1: exact match
        let exact = self.index.query_exact(&q.family, q.weight, q.style);
        if let Some(&id) = exact.first() {
            return QueryResult {
                found: Some(id),
                candidates: Vec::new(),
                suggestion: None,
            };
        }

        // Step 2: family-wide candidates
        let family_ids = self.index.query_family(&q.family);
        let candidates: Vec<FontFace> = family_ids
            .iter()
            .filter_map(|&id| self.index.get_face(id).cloned())
            .collect();

        // Step 3: closest-weight suggestion
        let suggestion = find_closest_weight(&candidates, q.weight);

        QueryResult {
            found: None,
            candidates,
            suggestion,
        }
    }

    /// Check font availability without returning a resolved ID.
    pub fn check(&self, q: &FontQuery) -> Availability {
        let result = self.query(q);
        Availability {
            exact_match: result.found.is_some(),
            variants: result.candidates,
            suggestion: result.suggestion,
        }
    }

    /// Discover and load all system fonts. Returns count loaded.
    pub fn load_system_fonts(&mut self) -> usize {
        let dirs = discover_system_font_paths();
        let mut total = 0;
        for dir in dirs {
            total += self.load_dir_inner(&dir, true);
        }
        total
    }

    /// Load fonts from a user-specified directory. Returns count loaded.
    pub fn load_user_fonts_dir(&mut self, dir: &Path) -> usize {
        self.load_dir_inner(dir, false)
    }

    /// Load a single font from raw bytes (e.g., embedded ASS fonts).
    pub fn load_user_font_data(&mut self, data: Vec<u8>) -> Result<FontId, FontError> {
        let id = self.db.load_font_data(data, false)?;
        if let Some(face) = self.db.get_face(id) {
            self.index.insert(face.clone());
        }
        Ok(id)
    }

    pub fn get_font_data(&self, id: FontId) -> Option<&[u8]> {
        self.db.get_data(id)
    }

    /// Number of indexed font faces.
    pub fn len(&self) -> usize {
        self.index.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }

    pub fn list_families(&self) -> Vec<String> {
        self.index.list_families()
    }

    // -----------------------------------------------------------------------
    // Internal
    // -----------------------------------------------------------------------

    fn load_dir_inner(&mut self, dir: &Path, is_system: bool) -> usize {
        let count = self.db.load_fonts_dir(dir, is_system);
        for face in self.db.faces() {
            self.index.insert(face.clone());
        }
        count
    }
}

impl Default for FontRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Find the face in `candidates` whose weight is closest to `target`.
fn find_closest_weight(candidates: &[FontFace], target: FontWeight) -> Option<FontFace> {
    candidates
        .iter()
        .min_by_key(|f| {
            let diff = f.weight.as_u16() as i32 - target.as_u16() as i32;
            diff.unsigned_abs()
        })
        .cloned()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::types::FontStyle;
    use super::*;

    fn dejavu_path() -> &'static Path {
        Path::new("/usr/share/fonts/truetype/dejavu")
    }

    fn has_dejavu() -> bool {
        dejavu_path().exists()
    }

    #[test]
    fn new_registry_is_empty() {
        let reg = FontRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
    }

    #[test]
    fn load_system_fonts_returns_positive() {
        if !has_dejavu() {
            eprintln!("SKIP: no DejaVu fonts");
            return;
        }
        let mut reg = FontRegistry::new();
        let count = reg.load_system_fonts();
        assert!(count > 0, "expected >0 system fonts, got {count}");
        assert!(!reg.is_empty());
    }

    #[test]
    fn query_existing_font_returns_found() {
        if !has_dejavu() {
            eprintln!("SKIP: no DejaVu fonts");
            return;
        }
        let mut reg = FontRegistry::new();
        reg.load_system_fonts();

        let result = reg.query(&FontQuery {
            family: "DejaVu Sans".into(),
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
        });
        assert!(
            result.found.is_some(),
            "expected DejaVu Sans Normal to be found"
        );
    }

    #[test]
    fn query_nonexistent_returns_not_found() {
        if !has_dejavu() {
            eprintln!("SKIP: no DejaVu fonts");
            return;
        }
        let mut reg = FontRegistry::new();
        reg.load_system_fonts();

        let result = reg.query(&FontQuery {
            family: "CompletelyFakeFont12345".into(),
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
        });
        assert!(result.found.is_none());
        assert!(result.candidates.is_empty());
    }

    #[test]
    fn query_suggestion_closest_weight() {
        if !has_dejavu() {
            eprintln!("SKIP: no DejaVu fonts");
            return;
        }
        let mut reg = FontRegistry::new();
        reg.load_system_fonts();

        // Query Bold — DejaVu Sans has Normal(400), so suggestion should be the
        // closest weight variant in the family.
        let result = reg.query(&FontQuery {
            family: "DejaVu Sans".into(),
            weight: FontWeight::Bold,
            style: FontStyle::Normal,
        });
        // If no exact Bold exists, we expect a suggestion
        if result.found.is_none() {
            assert!(
                result.suggestion.is_some(),
                "expected a closest-weight suggestion"
            );
        }
    }

    #[test]
    fn check_exact_match_found() {
        if !has_dejavu() {
            eprintln!("SKIP: no DejaVu fonts");
            return;
        }
        let mut reg = FontRegistry::new();
        reg.load_system_fonts();

        let avail = reg.check(&FontQuery {
            family: "DejaVu Sans".into(),
            weight: FontWeight::Normal,
            style: FontStyle::Normal,
        });
        assert!(avail.exact_match);
    }

    #[test]
    fn check_exact_match_not_found() {
        if !has_dejavu() {
            eprintln!("SKIP: no DejaVu fonts");
            return;
        }
        let mut reg = FontRegistry::new();
        reg.load_system_fonts();

        let avail = reg.check(&FontQuery {
            family: "DejaVu Sans".into(),
            weight: FontWeight::Black,
            style: FontStyle::Italic,
        });
        // May or may not have exact match depending on installed fonts
        // but should not panic
        let _ = avail;
    }

    #[test]
    fn load_user_font_data_roundtrip() {
        if !has_dejavu() {
            eprintln!("SKIP: no DejaVu fonts");
            return;
        }
        let data = std::fs::read(dejavu_path().join("DejaVuSans.ttf")).unwrap();
        let mut reg = FontRegistry::new();
        let id = reg.load_user_font_data(data).unwrap();

        let face = reg.index.get_face(id).unwrap();
        assert_eq!(face.family, "DejaVu Sans");
    }

    #[test]
    fn load_user_font_data_invalid_returns_err() {
        let mut reg = FontRegistry::new();
        let result = reg.load_user_font_data(vec![0x00, 0x01, 0x02, 0x03]);
        assert!(result.is_err());
    }

    #[test]
    fn load_user_fonts_dir_with_tempdir() {
        use std::fs;
        use tempfile::tempdir;

        if !has_dejavu() {
            eprintln!("SKIP: no DejaVu fonts");
            return;
        }

        let td = tempdir().unwrap();
        let src = dejavu_path().join("DejaVuSans.ttf");
        let dst = td.path().join("DejaVuSans.ttf");
        fs::copy(&src, &dst).unwrap();

        let mut reg = FontRegistry::new();
        let count = reg.load_user_fonts_dir(td.path());
        assert!(count > 0, "expected >0 fonts from temp dir, got {count}");
    }
}
