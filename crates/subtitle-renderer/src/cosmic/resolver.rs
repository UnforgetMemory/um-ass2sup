//! Font resolution and glyph rasterization using cosmic-text.
//!
//! Provides [`FontCosmicResolver`] — a font database and glyph cache backed by
//! [`cosmic_text::FontSystem`] + [`cosmic_text::SwashCache`], as an alternative
//! to the existing `FontManager` (fontdb + ttf-parser based).

use std::collections::HashMap;
use std::path::Path;

use cosmic_text::{CacheKey, CacheKeyFlags, FontSystem, SwashCache, SwashImage, Weight};
use fontdb::ID;
use parking_lot::Mutex;

/// Thread-safe font resolver wrapping cosmic-text's FontSystem and SwashCache.
///
/// FontSystem manages the font database (wrapping fontdb internally) and
/// SwashCache provides glyph rasterization with subpixel caching.
///
/// All mutable operations are serialised behind Mutex because FontSystem
/// methods require `&mut self`.
pub struct FontCosmicResolver {
    font_system: Mutex<FontSystem>,
    swash_cache: Mutex<SwashCache>,
    /// Memoised result of the all-faces CJK scan: `None` = not yet scanned,
    /// `Some(None)` = scanned and nothing found, `Some(Some(id))` = found.
    cjk_scan_cache: Mutex<Option<Option<ID>>>,
    /// Per-font CJK capability: `true` = face has U+4E2D glyph.
    cjk_glyphs_cache: Mutex<HashMap<ID, bool>>,
}

impl FontCosmicResolver {
    /// Creates a new resolver, loading all system fonts.
    ///
    /// Uses `FontSystem::new()` which internally discovers fonts via the
    /// platform-native mechanism (fontconfig on Linux, DirectWrite on Windows,
    /// CoreText on macOS).
    pub fn new() -> Self {
        let font_system = FontSystem::new();
        Self {
            font_system: Mutex::new(font_system),
            swash_cache: Mutex::new(SwashCache::new()),
            cjk_scan_cache: Mutex::new(None),
            cjk_glyphs_cache: Mutex::new(HashMap::new()),
        }
    }

    /// Re-scans all system-installed fonts.
    ///
    /// Clears all font-related caches (query, CJK, glyphs).
    pub fn load_system_fonts(&self) {
        let mut fs = self.font_system.lock();
        fs.db_mut().load_system_fonts();
        self.cjk_scan_cache.lock().take();
        self.cjk_glyphs_cache.lock().clear();
    }

    /// Loads a font file from disk (TTF, OTF, WOFF2).
    ///
    /// Returns the font ID of the last loaded face, or an error string.
    pub fn load_font_file(&self, path: &Path) -> Result<ID, String> {
        let mut fs = self.font_system.lock();
        let db = fs.db_mut();
        db.load_font_file(path).map_err(|e| e.to_string())?;
        let id = db
            .faces()
            .last()
            .map(|f| f.id)
            .ok_or_else(|| "No face loaded".to_string())?;
        self.cjk_scan_cache.lock().take();
        self.cjk_glyphs_cache.lock().clear();
        Ok(id)
    }

    /// Recursively loads fonts from a directory.
    ///
    /// Returns the number of newly added faces.
    pub fn load_fonts_dir(&self, dir: &Path) -> usize {
        let before = self.font_count();
        {
            let mut fs = self.font_system.lock();
            fs.db_mut().load_fonts_dir(dir);
        }
        let after = self.font_count();
        self.cjk_scan_cache.lock().take();
        self.cjk_glyphs_cache.lock().clear();
        after.saturating_sub(before)
    }

    /// Loads font data from memory (e.g. ASS embedded fonts).
    ///
    /// Returns the font ID of the last face, or `ID::dummy()` if parsing failed.
    pub fn load_font_data(&self, data: Vec<u8>) -> ID {
        let mut fs = self.font_system.lock();
        let db = fs.db_mut();
        db.load_font_data(data);
        let id = db.faces().last().map(|f| f.id).unwrap_or_else(ID::dummy);
        self.cjk_scan_cache.lock().take();
        self.cjk_glyphs_cache.lock().clear();
        id
    }

    /// Queries a font by family name, bold flag, and italic flag.
    ///
    /// Uses fontdb's built-in query (which may consult fontconfig / DirectWrite
    /// / CoreText depending on platform).
    pub fn resolve_font(&self, family: &str, bold: bool, italic: bool) -> Option<ID> {
        use fontdb::{Family, Query, Style};
        let weight = if bold { Weight(700) } else { Weight(400) };
        let style = if italic { Style::Italic } else { Style::Normal };
        let query = Query {
            families: &[Family::Name(family), Family::SansSerif],
            weight,
            style,
            ..Default::default()
        };
        self.font_system.lock().db_mut().query(&query)
    }

    /// Returns a rasterised glyph image from the swash cache.
    ///
    /// The returned [`SwashImage`] provides alpha mask or RGBA pixel data that
    /// can be composed onto a rendering surface.
    pub fn get_image(&self, font_id: ID, glyph_id: u16, font_size: f32) -> Option<SwashImage> {
        let (cache_key, _integer_x, _integer_y) = CacheKey::new(
            font_id,
            glyph_id,
            font_size,
            (0.0, 0.0),
            Weight::NORMAL,
            CacheKeyFlags::empty(),
        );
        let image = self
            .swash_cache
            .lock()
            .get_image(&mut self.font_system.lock(), cache_key)
            .clone();
        image
    }

    /// Returns the ID of the first loaded face that contains the CJK test
    /// glyph U+4E2D (中), or `None` if no loaded face qualifies.
    ///
    /// The result is memoised so the scan runs at most once per resolver
    /// lifetime (cleared when fonts are added / loaded).
    pub fn query_cjk_capable_any(&self) -> Option<ID> {
        // Fast path: memoised result
        {
            let cache = self.cjk_scan_cache.lock();
            if let Some(cached) = *cache {
                return cached;
            }
        }

        let scan_start = std::time::Instant::now();
        let fs = self.font_system.lock();
        let face_ids: Vec<ID> = fs.db().faces().map(|f| f.id).collect();
        let face_count = face_ids.len();
        drop(fs);

        tracing::debug!(face_count, "query_cjk_capable_any: scanning all faces");

        let result = face_ids
            .into_iter()
            .find(|&id| self.font_has_cjk_glyphs(id));

        *self.cjk_scan_cache.lock() = Some(result);
        let scan_elapsed = scan_start.elapsed();
        tracing::debug!(
            ?result,
            face_count,
            elapsed_ms = scan_elapsed.as_millis() as u64,
            "query_cjk_capable_any: complete"
        );
        if scan_elapsed.as_millis() > 1000 {
            tracing::warn!(
                face_count,
                elapsed_ms = scan_elapsed.as_millis() as u64,
                "query_cjk_capable_any: SLOW scan (>1s)"
            );
        }
        result
    }

    /// Returns `true` if the specified font face contains U+4E2D (中).
    ///
    /// Results are cached per ID to avoid repeated TTF/OTF parsing.
    fn font_has_cjk_glyphs(&self, id: ID) -> bool {
        {
            let cache = self.cjk_glyphs_cache.lock();
            if let Some(&v) = cache.get(&id) {
                return v;
            }
        }

        let fs = self.font_system.lock();
        let result = fs
            .db()
            .with_face_data(id, |data, index| {
                ttf_parser::Face::parse(data, index)
                    .ok()
                    .and_then(|parsed| parsed.glyph_index('\u{4E2D}'))
                    .is_some_and(|g| g.0 != 0)
            })
            .unwrap_or(false);
        drop(fs);

        self.cjk_glyphs_cache.lock().insert(id, result);
        result
    }

    /// Eagerly populates the CJK face scan cache so the first render does not
    /// trigger a full scan that would block all parallel workers.
    ///
    /// Safe to call multiple times — the scan runs at most once.
    pub fn warmup_cjk_scan(&self) {
        let _ = self.query_cjk_capable_any();
    }

    /// Returns the number of loaded font faces.
    pub fn font_count(&self) -> usize {
        self.font_system.lock().db().faces().count()
    }

    /// Provides mutable access to the underlying FontSystem for shaping.
    pub fn font_system(&self) -> parking_lot::MutexGuard<'_, FontSystem> {
        self.font_system.lock()
    }

    /// Provides mutable access to the underlying SwashCache for rasterization.
    pub fn swash_cache(&self) -> parking_lot::MutexGuard<'_, SwashCache> {
        self.swash_cache.lock()
    }

    /// Returns the inner FontSystem mutex for more complex locking patterns.
    pub fn font_system_mutex(&self) -> &Mutex<FontSystem> {
        &self.font_system
    }

    /// Returns the inner SwashCache mutex for more complex locking patterns.
    pub fn swash_cache_mutex(&self) -> &Mutex<SwashCache> {
        &self.swash_cache
    }
}

impl Default for FontCosmicResolver {
    fn default() -> Self {
        Self::new()
    }
}
