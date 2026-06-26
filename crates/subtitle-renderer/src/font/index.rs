//! High-performance font index with O(1) exact matching and O(k) family lookup.
//!
//! The [`FontIndex`] maintains three hash-map indices for fast font queries
//! by family/weight/style combination, family-wide lookup, and metadata
//! retrieval by identifier.

use std::collections::HashMap;

use crate::font::types::{FontFace, FontId, FontStyle, FontWeight};

/// High-performance font index with O(1) exact matching.
///
/// Lookup paths:
/// * `query_exact(family, weight, style)` — O(1) composite-key hash lookup.
/// * `query_family(family)` — O(k) over all faces in that family.
/// * `get_face(id)` — O(1) by identifier.
pub struct FontIndex {
    /// Exact match: (family_hash, weight, style) → font ids.
    exact: HashMap<(u64, u16, FontStyle), Vec<FontId>>,
    /// Family-wide lookup: family_hash → all font ids in that family.
    families: HashMap<u64, Vec<FontId>>,
    /// Family name → hash mapping for [`list_families`](FontIndex::list_families).
    family_names: HashMap<u64, String>,
    /// Full [`FontFace`] metadata indexed by [`FontId`].
    faces: HashMap<FontId, FontFace>,
}

impl FontIndex {
    /// Create an empty font index.
    pub fn new() -> Self {
        Self {
            exact: HashMap::new(),
            families: HashMap::new(),
            family_names: HashMap::new(),
            faces: HashMap::new(),
        }
    }

    /// Insert a font face into all indices.
    ///
    /// The face is indexed under:
    /// * its exact (family, weight, style) composite key,
    /// * its family-wide bucket,
    /// * its [`FontId`] for direct metadata lookup.
    pub fn insert(&mut self, face: FontFace) {
        let hash = family_hash(&face.family);
        let id = face.id;

        // Index by (family_hash, weight, style)
        let key = (hash, face.weight.as_u16(), face.style);
        self.exact.entry(key).or_default().push(id);

        // Index by family-wide bucket
        self.families.entry(hash).or_default().push(id);

        // Store family name for list_families
        self.family_names
            .entry(hash)
            .or_insert_with(|| face.family.clone());

        // Store full metadata
        self.faces.insert(id, face);
    }

    /// Exact match query: (family, weight, style).
    ///
    /// Returns all font identifiers that match the given family name,
    /// weight, and style exactly. The family name lookup is case-insensitive.
    ///
    /// **Complexity:** O(1) hashtable lookup.
    pub fn query_exact(&self, family: &str, weight: FontWeight, style: FontStyle) -> Vec<FontId> {
        let hash = family_hash(family);
        let key = (hash, weight.as_u16(), style);
        self.exact.get(&key).cloned().unwrap_or_default()
    }

    /// Family-wide query: all fonts in a family regardless of weight or style.
    ///
    /// **Complexity:** O(k) where k is the number of faces in the family.
    pub fn query_family(&self, family: &str) -> Vec<FontId> {
        let hash = family_hash(family);
        self.families.get(&hash).cloned().unwrap_or_default()
    }

    /// Get [`FontFace`] metadata by identifier.
    pub fn get_face(&self, id: FontId) -> Option<&FontFace> {
        self.faces.get(&id)
    }

    /// List all unique family names known to the index.
    pub fn list_families(&self) -> Vec<String> {
        let mut names: Vec<String> = self.family_names.values().cloned().collect();
        names.sort();
        names.dedup();
        names
    }

    /// Number of indexed font faces.
    pub fn len(&self) -> usize {
        self.faces.len()
    }

    /// Returns `true` if no font faces are indexed.
    pub fn is_empty(&self) -> bool {
        self.faces.is_empty()
    }
}

impl Default for FontIndex {
    fn default() -> Self {
        Self::new()
    }
}

/// FNV-1a hash of family name with ASCII lowercasing for case-insensitive matching.
fn family_hash(family: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for byte in family.bytes() {
        let b = byte.to_ascii_lowercase();
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_face(id: u32, family: &str, weight: FontWeight, style: FontStyle) -> FontFace {
        FontFace {
            id: FontId(id),
            family: family.to_string(),
            weight,
            style,
            stretch: crate::font::types::FontStretch::Normal,
            path: None,
            is_system: false,
            cjk: false,
            corrupt: false,
        }
    }

    #[test]
    fn insert_and_query_exact_returns_correct_id() {
        let mut idx = FontIndex::new();
        let face = make_face(1, "Arial", FontWeight::Normal, FontStyle::Normal);
        idx.insert(face);

        let results = idx.query_exact("Arial", FontWeight::Normal, FontStyle::Normal);
        assert_eq!(results, vec![FontId(1)]);
    }

    #[test]
    fn query_exact_with_different_weight_returns_empty() {
        let mut idx = FontIndex::new();
        idx.insert(make_face(1, "Arial", FontWeight::Normal, FontStyle::Normal));

        let results = idx.query_exact("Arial", FontWeight::Bold, FontStyle::Normal);
        assert!(results.is_empty());
    }

    #[test]
    fn query_exact_with_different_style_returns_empty() {
        let mut idx = FontIndex::new();
        idx.insert(make_face(1, "Arial", FontWeight::Normal, FontStyle::Normal));

        let results = idx.query_exact("Arial", FontWeight::Normal, FontStyle::Italic);
        assert!(results.is_empty());
    }

    #[test]
    fn query_family_returns_all_variants() {
        let mut idx = FontIndex::new();
        idx.insert(make_face(1, "Arial", FontWeight::Normal, FontStyle::Normal));
        idx.insert(make_face(2, "Arial", FontWeight::Bold, FontStyle::Normal));
        idx.insert(make_face(3, "Arial", FontWeight::Normal, FontStyle::Italic));

        let mut results = idx.query_family("Arial");
        results.sort();
        assert_eq!(results, vec![FontId(1), FontId(2), FontId(3)]);
    }

    #[test]
    fn query_family_unknown_family_returns_empty() {
        let mut idx = FontIndex::new();
        idx.insert(make_face(1, "Arial", FontWeight::Normal, FontStyle::Normal));

        let results = idx.query_family("Nonexistent");
        assert!(results.is_empty());
    }

    #[test]
    fn family_hash_is_case_insensitive() {
        assert_eq!(family_hash("Arial"), family_hash("arial"));
        assert_eq!(family_hash("ARIAL"), family_hash("arial"));
        assert_eq!(
            family_hash("Times New Roman"),
            family_hash("times new roman")
        );
    }

    #[test]
    fn family_hash_different_names_differ() {
        assert_ne!(family_hash("Arial"), family_hash("Times"));
        assert_ne!(family_hash("Arial"), family_hash(""));
        assert_ne!(family_hash("a"), family_hash("b"));
    }

    #[test]
    fn family_hash_empty_string() {
        // Empty string should produce a deterministic hash, not panic.
        let h = family_hash("");
        assert_ne!(h, 0);
    }

    #[test]
    fn list_families_returns_unique_names() {
        let mut idx = FontIndex::new();
        idx.insert(make_face(1, "Arial", FontWeight::Normal, FontStyle::Normal));
        idx.insert(make_face(2, "Arial", FontWeight::Bold, FontStyle::Normal));
        idx.insert(make_face(3, "Times", FontWeight::Normal, FontStyle::Normal));

        let mut families = idx.list_families();
        families.sort();
        assert_eq!(families, vec!["Arial", "Times"]);
    }

    #[test]
    fn list_families_empty_index() {
        let idx = FontIndex::new();
        assert!(idx.list_families().is_empty());
    }

    #[test]
    fn get_face_returns_metadata() {
        let mut idx = FontIndex::new();
        let face = make_face(42, "Comic Sans", FontWeight::Bold, FontStyle::Italic);
        idx.insert(face.clone());

        let retrieved = idx.get_face(FontId(42));
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().family, "Comic Sans");
        assert_eq!(retrieved.unwrap().weight, FontWeight::Bold);
        assert_eq!(retrieved.unwrap().style, FontStyle::Italic);
    }

    #[test]
    fn get_face_unknown_id_returns_none() {
        let idx = FontIndex::new();
        assert!(idx.get_face(FontId(999)).is_none());
    }

    #[test]
    fn len_and_is_empty() {
        let mut idx = FontIndex::new();
        assert!(idx.is_empty());
        assert_eq!(idx.len(), 0);

        idx.insert(make_face(1, "Arial", FontWeight::Normal, FontStyle::Normal));
        assert!(!idx.is_empty());
        assert_eq!(idx.len(), 1);

        idx.insert(make_face(2, "Arial", FontWeight::Bold, FontStyle::Normal));
        assert_eq!(idx.len(), 2);
    }

    #[test]
    fn default_is_empty() {
        let idx = FontIndex::default();
        assert!(idx.is_empty());
    }

    #[test]
    fn query_exact_with_case_insensitive_family() {
        let mut idx = FontIndex::new();
        idx.insert(make_face(1, "Arial", FontWeight::Normal, FontStyle::Normal));

        assert_eq!(
            idx.query_exact("arial", FontWeight::Normal, FontStyle::Normal),
            vec![FontId(1)]
        );
        assert_eq!(
            idx.query_exact("ARIAL", FontWeight::Normal, FontStyle::Normal),
            vec![FontId(1)]
        );
    }

    #[test]
    fn multiple_faces_same_exact_key() {
        let mut idx = FontIndex::new();
        // Two faces with same family/weight/style (e.g., different source files)
        idx.insert(make_face(1, "Arial", FontWeight::Normal, FontStyle::Normal));
        idx.insert(make_face(2, "Arial", FontWeight::Normal, FontStyle::Normal));

        let mut results = idx.query_exact("Arial", FontWeight::Normal, FontStyle::Normal);
        results.sort();
        assert_eq!(results, vec![FontId(1), FontId(2)]);
    }
}
