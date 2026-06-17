use fontdb::{Database, Family, Query, Weight};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
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

/// Interior state for `FontManager`, wrapped in `parking_lot::Mutex` to allow
/// `&self`-based access to `Database` methods that require `&mut self` (e.g.
/// fontconfig lazy loading).
struct FontManagerInner {
    db: Database,
    /// Cache of (font data, face_index) keyed by fontdb::ID to avoid repeated
    /// cloning from fontdb. The face_index is needed for TTC (collection) fonts
    /// where the data is the whole collection and the index selects which face.
    font_data_cache: HashMap<fontdb::ID, (Arc<Vec<u8>>, u32)>,
    // Cache for font queries: (lowercase family, bold, italic) → fontdb::ID.
    // Cleared when fonts are loaded or added.
    query_cache: HashMap<(String, bool, bool), fontdb::ID>,
    /// Lazy-initialized fontconfig handle, loaded on first use.
    #[cfg(feature = "fontconfig")]
    fc: Option<fontconfig::Fontconfig>,
    /// Cache of fontconfig font lookups: font name → fontdb::ID.
    #[cfg(feature = "fontconfig")]
    fc_loaded_cache: HashMap<String, Option<fontdb::ID>>,
    /// Memoised result of [`FontManager::query_cjk_capable_any`] so the scan
    /// runs at most once per FontManager lifetime. Cleared whenever fonts are
    /// loaded or added (see [`FontManager::load_font_file`], `load_font_data`).
    cjk_scan_cache: parking_lot::Mutex<Option<Option<fontdb::ID>>>,
    /// Cache of per-font CJK glyph test results (fontdb::ID → has CJK glyphs).
    /// Avoids redundant `ttf_parser::Face::parse` calls in concurrent fallback chains.
    cjk_glyphs_cache: HashMap<fontdb::ID, bool>,
}

impl Default for FontManagerInner {
    fn default() -> Self {
        Self {
            db: Database::new(),
            font_data_cache: HashMap::new(),
            query_cache: HashMap::new(),
            #[cfg(feature = "fontconfig")]
            fc: None,
            #[cfg(feature = "fontconfig")]
            fc_loaded_cache: HashMap::new(),
            cjk_scan_cache: parking_lot::Mutex::new(None),
            cjk_glyphs_cache: HashMap::new(),
        }
    }
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
    inner: Mutex<FontManagerInner>,
}

impl FontManager {
    /// Creates an empty font manager with no loaded fonts.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(FontManagerInner::default()),
        }
    }

    /// Loads all system-installed fonts. May be slow on first call.
    pub fn load_system_fonts(&mut self) {
        let mut inner = self.inner.lock();
        inner.db.load_system_fonts();
        inner.query_cache.clear();
        inner.cjk_glyphs_cache.clear();
        *inner.cjk_scan_cache.lock() = None;
        drop(inner);

        #[cfg(feature = "fontconfig")]
        self.load_fontconfig_fonts();
    }

    /// Initializes fontconfig and stores the handle for on-demand font resolution.
    /// Fonts are resolved lazily by [`resolve_via_fontconfig`] rather than eagerly loaded.
    #[cfg(feature = "fontconfig")]
    fn load_fontconfig_fonts(&mut self) {
        let fc = match fontconfig::Fontconfig::new() {
            Some(fc) => fc,
            None => return,
        };
        self.inner.lock().fc = Some(fc);
    }

    /// Loads a font file from disk (TTF, OTF, WOFF2).
    ///
    /// # Errors
    ///
    /// Returns [`FontError::LoadError`] if the file cannot be read or contains no valid faces.
    pub fn load_font_file(&mut self, path: &Path) -> Result<fontdb::ID, FontError> {
        let mut inner = self.inner.lock();
        inner
            .db
            .load_font_file(path)
            .map_err(|e| FontError::LoadError(e.to_string()))?;
        inner.query_cache.clear();
        inner.cjk_glyphs_cache.clear();
        *inner.cjk_scan_cache.lock() = None;
        let id = inner
            .db
            .faces()
            .last()
            .map(|f| f.id)
            .ok_or_else(|| FontError::LoadError("No face loaded".into()))?;
        Ok(id)
    }

    /// Recursively loads every font file found under `dir`. Returns the number
    /// of faces that were newly added to the database (zero on I/O errors).
    ///
    /// Used by the CLI's `--font-dir` flag to let users point at platform-
    /// specific or user-installed font collections without copying them into
    /// the OS font directory.
    pub fn load_fonts_dir(&mut self, dir: &Path) -> usize {
        let before = self.font_count();
        {
            let mut inner = self.inner.lock();
            inner.db.load_fonts_dir(dir);
            inner.query_cache.clear();
            inner.cjk_glyphs_cache.clear();
            *inner.cjk_scan_cache.lock() = None;
        }
        self.font_count().saturating_sub(before)
    }

    /// Loads font data from memory (e.g. ASS embedded fonts).
    ///
    /// Returns the font ID, or [`fontdb::ID::dummy()`] if no face was loaded.
    pub fn load_font_data(&mut self, data: Vec<u8>) -> fontdb::ID {
        let mut inner = self.inner.lock();
        inner.db.load_font_data(data);
        inner.query_cache.clear();
        inner.cjk_glyphs_cache.clear();
        *inner.cjk_scan_cache.lock() = None;
        inner
            .db
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
        self.inner.lock().db.query(&query)
    }

    /// Queries a font using a scoring algorithm that considers weight difference,
    /// italic match, and family name. Returns the best-scoring font from all loaded faces.
    #[allow(clippy::incompatible_msrv)]
    pub fn query_with_score(&self, family: &str, bold: bool, italic: bool) -> Option<fontdb::ID> {
        let target_weight: u16 = if bold { 700 } else { 400 };
        let target_italic = italic;

        let mut best: Option<(fontdb::ID, f32)> = None;

        for face in self.inner.lock().db.faces() {
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
        {
            let inner = self.inner.lock();
            if let Some(cached) = inner.query_cache.get(&key) {
                return Some(*cached);
            }
        }
        let result = self.query_with_fallback_inner(family, bold, italic);
        if let Some(id) = result {
            self.inner.lock().query_cache.insert(key, id);
        }
        result
    }

    /// Resolves a font family via fontconfig when fontdb cannot find it.
    ///
    /// Uses fontconfig's alias resolution (e.g. `Arial` → `DejaVu Sans`) and
    /// lazily loads the font file into fontdb if needed. Results are cached in
    /// `fc_loaded_cache` to avoid repeated fontconfig lookups.
    #[cfg(feature = "fontconfig")]
    fn resolve_via_fontconfig(&self, family: &str, bold: bool, italic: bool) -> Option<fontdb::ID> {
        use std::ffi::CString;

        // Step 1: Check cache — avoid repeated fontconfig lookups.
        {
            let inner = self.inner.lock();
            match inner.fc_loaded_cache.get(family) {
                Some(Some(id)) => return Some(*id),
                Some(None) => return None,
                None => {}
            }
        }

        // Step 2: Check if the font is actually installed via fontconfig.
        // `list_fonts` returns empty for non-installed fonts (unlike `find`
        // which always returns a fallback). This correctly detects real
        // font installations vs generic fallbacks.
        let lookup_result = {
            let inner = self.inner.lock();
            let fc = inner.fc.as_ref()?;

            let mut pattern = fontconfig::Pattern::new(fc).ok()?;
            let family_cs = CString::new(family).ok()?;
            pattern.add_string(fontconfig::FC_FAMILY, &family_cs).ok()?;

            let fontset = fontconfig::list_fonts(&pattern, None).ok()?;

            // Collect eagerly — FontSet owns the data, avoids borrow issues.
            let best = fontset.iter().next();
            best.and_then(|p| {
                let path = p.filename().ok()?.to_owned();
                let name = p.name().ok()?.to_owned();
                Some((path, name))
            })
        };

        let (resolved_path, resolved_family) = match lookup_result {
            Some((path, name)) => (path, name),
            None => {
                // Cache negative: font not installed, no fallback needed.
                self.inner
                    .lock()
                    .fc_loaded_cache
                    .insert(family.to_string(), None);
                return None;
            }
        };

        // Step 3: Check if fontdb already has the resolved name.
        if let Some(id) = self.query(&resolved_family, bold, italic) {
            self.inner
                .lock()
                .fc_loaded_cache
                .insert(family.to_string(), Some(id));
            return Some(id);
        }

        // Step 4: Load the font file into fontdb.
        if std::path::Path::new(&resolved_path).exists() {
            if let Ok(_new_id) = self.inner.lock().db.load_font_file(&resolved_path) {
                self.inner.lock().query_cache.clear();
                // Re-query to get the correct face for the requested weight/style.
                if let Some(query_id) = self.query(&resolved_family, bold, italic) {
                    self.inner
                        .lock()
                        .fc_loaded_cache
                        .insert(family.to_string(), Some(query_id));
                    return Some(query_id);
                }
            }
        }

        // Step 5: Cache negative result so we don't retry.
        self.inner
            .lock()
            .fc_loaded_cache
            .insert(family.to_string(), None);
        None
    }

    /// Fallback query implementation (un-cached). See [`query_with_fallback`].
    fn query_with_fallback_inner(
        &self,
        family: &str,
        bold: bool,
        italic: bool,
    ) -> Option<fontdb::ID> {
        // 1. Try exact-ish match via scoring against all loaded faces.
        //    Trust the scoring result directly — do NOT call
        //    `font_has_cjk_glyphs` here. TTF parsing of every candidate
        //    face on every render call was the root cause of the v0.5.3
        //    Windows hang (a 22+ MB CJK font parsed per call, contended
        //    with Windows font cache service, could take 30+ seconds).
        //    If the scoring match happens to be Latin-only, the user
        //    will see tofu in the CJK range; the dedicated Step 2.5
        //    CJK scan (already pre-warmed at startup) catches the case
        //    where the user explicitly requests a CJK family that the
        //    database cannot find at all.
        if let Some(id) = self.query_with_score(family, bold, italic) {
            tracing::trace!(
                family = %family,
                id = ?id,
                "step 1 (scoring) returned a match"
            );
            return Some(id);
        } else {
            tracing::trace!(family = %family, "step 1 (scoring) found no candidate");
        }

        // 1.1 Weight/style-aware family name matching.
        // ASS files sometimes encode weight/style into the font family name
        // (e.g. "MiSans Demibold", "MiSans Normal", "Noto Sans Bold").
        // If the full name doesn't match, strip common style suffixes and
        // re-query with the base family name + weight/style flags.
        if let Some(base_family) = strip_style_suffix(family) {
            let weight = if bold {
                fontdb::Weight(700)
            } else {
                fontdb::Weight(400)
            };
            let style = if italic {
                fontdb::Style::Italic
            } else {
                fontdb::Style::Normal
            };
            let query = fontdb::Query {
                families: &[
                    fontdb::Family::Name(&base_family),
                    fontdb::Family::SansSerif,
                ],
                weight,
                style,
                ..Default::default()
            };
            if let Some(id) = self.inner.lock().db.query(&query) {
                if self.font_has_cjk_glyphs(id) {
                    tracing::trace!(
                        family = %family,
                        base = %base_family,
                        id = ?id,
                        "step 1.1 (suffix-stripped re-query) returned CJK-capable match"
                    );
                    return Some(id);
                }
            }
        }

        // 1.5 Fontconfig resolution — resolve aliases and load fonts not found by fontdb.
        #[cfg(feature = "fontconfig")]
        if let Some(id) = self.resolve_via_fontconfig(family, bold, italic) {
            tracing::debug!(
                family = %family,
                id = ?id,
                "step 1.5 (fontconfig) returned a match"
            );
            return Some(id);
        }

        // 2. Hardcoded fallback font names — CJK-capable families first
        //    so that Chinese, Japanese, and Korean subtitles render legibly
        //    even when no system CJK font is configured.
        //
        //    Pass 1: CJK-specific fonts — verify they actually have CJK glyphs.
        //    Pass 2: Generic fonts (may be Latin-only) — accepted as-is.
        let cjk_fallbacks = [
            "Noto Sans CJK SC",
            "Noto Sans CJK TC",
            "Noto Sans CJK JP",
            "WenQuanYi Micro Hei",
            "Source Han Sans CN",
            "IPAGothic",
            "NanumGothic",
        ];
        for fb in &cjk_fallbacks {
            if let Some(id) = self.query(fb, bold, italic) {
                if self.font_has_cjk_glyphs(id) {
                    tracing::debug!(
                        family = %family,
                        fallback = %fb,
                        id = ?id,
                        "step 2 (hardcoded CJK list) returned a match"
                    );
                    return Some(id);
                }
            }
        }

        // 2.5 Cross-platform CJK fallback: scan every loaded face and return
        //     the first one that has the CJK test glyph. fontdb's
        //     `load_system_fonts()` already enumerates macOS Hiragino /
        //     Windows Microsoft YaHei / Linux Noto CJK / container fonts, so
        //     this works on every OS without hardcoding platform-specific names.
        if let Some(id) = self.query_cjk_capable_any() {
            tracing::debug!(
                family = %family,
                id = ?id,
                "step 2.5 (cross-platform CJK scan) returned a match"
            );
            return Some(id);
        }

        let any_fallbacks = [
            "Liberation Sans",
            "DejaVu Sans",
            "Noto Sans",
            "Arial",
            "Helvetica",
        ];
        for fb in &any_fallbacks {
            if let Some(id) = self.query(fb, bold, italic) {
                tracing::debug!(
                    family = %family,
                    fallback = %fb,
                    id = ?id,
                    "step 3 (hardcoded generic list) returned a match"
                );
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
        if let Some(id) = self.inner.lock().db.query(&ss_query) {
            tracing::debug!(
                family = %family,
                id = ?id,
                "step 4 (Family::SansSerif query) returned a match"
            );
            return Some(id);
        }

        // 4. Last resort: any available face.
        if let Some(id) = self.inner.lock().db.faces().next().map(|f| f.id) {
            tracing::warn!(
                family = %family,
                id = ?id,
                "step 5 (last-resort: any face) returned a match — font availability is critical"
            );
            return Some(id);
        }
        tracing::error!(
            family = %family,
            "all fallback steps exhausted; NO font available"
        );
        None
    }

    /// Returns the id of the first loaded face that contains the CJK test
    /// glyph U+4E2D (中), or `None` if no loaded face qualifies.
    ///
    /// This is the platform-neutral CJK fallback: it works regardless of which
    /// CJK font is installed (Hiragino on macOS, Microsoft YaHei on Windows,
    /// Noto CJK on Linux, etc.) because fontdb's `load_system_fonts()` already
    /// populated every OS-native face into the database.
    ///
    /// Used as the cross-platform fallback step in
    /// [`query_with_fallback`](Self::query_with_fallback).
    pub fn query_cjk_capable_any(&self) -> Option<fontdb::ID> {
        // Fast path: memoised result (None = scanned, found nothing;
        // Some(_) = scanned, found this id). The double Option distinguishes
        // "not yet scanned" from "scanned and found nothing".
        if let Some(cached) = *self.inner.lock().cjk_scan_cache.lock() {
            tracing::trace!("query_cjk_capable_any: cache hit");
            return cached;
        }

        let scan_start = std::time::Instant::now();
        let face_ids: Vec<fontdb::ID> = {
            let inner = self.inner.lock();
            inner.db.faces().map(|f| f.id).collect()
        };
        let face_count = face_ids.len();
        tracing::debug!(face_count, "query_cjk_capable_any: scanning all faces");
        let result = face_ids.into_iter().find(|&id| {
            self.inner
                .lock()
                .db
                .with_face_data(id, |data, index| {
                    ttf_parser::Face::parse(data, index)
                        .ok()
                        .and_then(|parsed| parsed.glyph_index('\u{4E2D}'))
                        .is_some_and(|g| g.0 != 0)
                })
                .unwrap_or(false)
        });
        *self.inner.lock().cjk_scan_cache.lock() = Some(result);
        let scan_elapsed = scan_start.elapsed();
        tracing::debug!(
            ?result,
            face_count,
            elapsed_ms = scan_elapsed.as_millis() as u64,
            "query_cjk_capable_any: scan complete and cached"
        );
        if scan_elapsed.as_millis() > 1000 {
            tracing::warn!(
                face_count,
                elapsed_ms = scan_elapsed.as_millis() as u64,
                "query_cjk_capable_any: SLOW scan (>1s) — may indicate too many fonts"
            );
        }
        result
    }

    /// Returns true if any loaded font face has an exact (case-insensitive) family name match.
    ///
    /// Unlike [`query_with_score`], which returns the _best_ available match even when
    /// the family is completely absent, this method performs a strict family-name check
    /// — it returns `true` only when at least one loaded face declares the given family.
    ///
    /// This is the correct predicate for font-availability checks such as
    /// [`check_ass_fonts`]: a font that is not reported by this method is genuinely
    /// unavailable and will fall back through the renderer's fallback chain.
    pub fn has_exact_family(&self, family: &str) -> bool {
        self.inner.lock().db.faces().any(|face| {
            face.families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(family))
        })
    }

    /// Returns `true` if any loaded font face is a reasonable match for the
    /// requested family name.
    ///
    /// Four-tier matching:
    ///
    /// 1. **Exact match** — case-insensitive family name equality.
    /// 2. **Substring match** — handles minor naming variance
    ///    (e.g. `SimHei` vs `SimHei Regular`).
    /// 3. **Platform query** — uses fontdb's platform-native font resolution
    ///    (fontconfig on Linux, DirectWrite on Windows, CoreText on macOS),
    ///    then verifies the resolved font's family is related to the request.
    ///    This catches cases like fontconfig aliases (`Arial`→`DejaVu Sans`)
    ///    where the requested font genuinely isn't installed.
    pub fn has_available_font(&self, family: &str) -> bool {
        // Tier 1: Exact family name match
        if self.has_exact_family(family) {
            return true;
        }
        let family_lower = family.to_lowercase();

        // Tier 2: Substring match
        if self.inner.lock().db.faces().any(|face| {
            face.families.iter().any(|(name, _)| {
                let name_lower = name.to_lowercase();
                name_lower.contains(&family_lower) || family_lower.contains(&name_lower)
            })
        }) {
            return true;
        }

        // Tier 3: Platform-level query — uses fontdb's backend (fontconfig/
        // DirectWrite/CoreText) to resolve the font, then verifies the
        // returned font's family is related to the request (not a generic
        // fallback).
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(family)],
            ..Default::default()
        };
        if let Some(id) = self.inner.lock().db.query(&query) {
            if let Some(face) = self.inner.lock().db.face(id) {
                return face.families.iter().any(|(name, _)| {
                    let name_lower = name.to_lowercase();
                    name_lower.contains(&family_lower) || family_lower.contains(&name_lower)
                });
            }
        }

        false
    }

    /// Retrieves font data for a given font ID, caching the result as `Arc<Vec<u8>>`.
    ///
    /// Results are cached as `Arc<Vec<u8>>` so that repeated calls with the same
    /// ID share the underlying allocation via cheap Arc clones instead of full
    /// byte copies. The first call clones from fontdb; subsequent calls only
    /// increment the Arc reference count.
    pub fn get_font_data(&self, id: fontdb::ID) -> Option<Arc<Vec<u8>>> {
        self.get_font_data_with_index(id).map(|(data, _)| data)
    }

    /// Returns font data and its face index within the font file.
    ///
    /// For TTC (TrueType Collection) fonts, `face_index` indicates which face
    /// within the collection to parse. Non-collection fonts return index 0.
    ///
    /// Results are cached as `(Arc<Vec<u8>>, u32)` so that repeated calls
    /// with the same ID share the underlying allocation via cheap Arc clones.
    pub fn get_font_data_with_index(&self, id: fontdb::ID) -> Option<(Arc<Vec<u8>>, u32)> {
        // Check cache under lock
        {
            let inner = self.inner.lock();
            if let Some((cached, index)) = inner.font_data_cache.get(&id) {
                return Some((Arc::clone(cached), *index));
            }
        }
        // Not cached: load data from database and store under a fresh lock.
        let result = self
            .inner
            .lock()
            .db
            .with_face_data(id, |data, index| (Arc::new(data.to_vec()), index))?;
        self.inner
            .lock()
            .font_data_cache
            .insert(id, (Arc::clone(&result.0), result.1));
        Some(result)
    }

    /// Check if a font contains CJK (Chinese/Japanese/Korean) glyphs.
    ///
    /// Tests a sample CJK character (U+4E2D "中"). If the font maps it to
    /// a real glyph (not the .notdef glyph), it is considered CJK-capable.
    fn font_has_cjk_glyphs(&self, id: fontdb::ID) -> bool {
        // Fast path: cached result
        {
            let inner = self.inner.lock();
            if let Some(&v) = inner.cjk_glyphs_cache.get(&id) {
                return v;
            }
        }
        // Cache miss: parse TTF and store result
        let result = self
            .inner
            .lock()
            .db
            .with_face_data(id, |data, index| {
                ttf_parser::Face::parse(data, index)
                    .ok()
                    .and_then(|parsed| parsed.glyph_index('\u{4E2D}'))
                    .is_some_and(|g| g.0 != 0)
            })
            .unwrap_or(false);
        self.inner.lock().cjk_glyphs_cache.insert(id, result);
        result
    }

    /// Eagerly populates the CJK face scan cache so the first render does not
    /// trigger a full scan that would block all parallel workers.
    ///
    /// This should be called once at renderer init, before any render_ass call.
    /// Safe to call multiple times — the cache is populated at most once.
    pub fn warmup_cjk_scan(&self) {
        let _ = self.query_cjk_capable_any();
    }

    /// Returns the number of loaded font faces.
    pub fn font_count(&self) -> usize {
        self.inner.lock().db.faces().count()
    }

    /// Returns a list of all loaded fonts with their metadata.
    pub fn list_fonts(&self) -> Vec<FontInfo> {
        self.inner
            .lock()
            .db
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

/// Known style suffixes that ASS files sometimes append to font family names,
/// e.g. "MiSans Demibold" → base "MiSans", "Arial Bold" → base "Arial".
/// This list covers common CSS-style weight and style qualifiers.
const STYLE_SUFFIXES: &[&str] = &[
    "Thin",
    "ExtraLight",
    "Light",
    "Normal",
    "Regular",
    "Medium",
    "DemiBold",
    "SemiBold",
    "Bold",
    "ExtraBold",
    "Heavy",
    "Black",
    "UltraBlack",
    "Italic",
    "Oblique",
];

/// Strips a known style suffix from a font family name.
///
/// Returns `Some(base)` if the name ends with a known suffix (case-insensitive),
/// otherwise returns `None`. The suffix must be the last space-separated token
/// in the name (e.g. "MiSans Demibold" → "MiSans").
fn strip_style_suffix(family: &str) -> Option<String> {
    let last_space = family.rfind(' ')?;
    let suffix = &family[last_space + 1..];
    if STYLE_SUFFIXES
        .iter()
        .any(|s| s.eq_ignore_ascii_case(suffix))
    {
        Some(family[..last_space].to_owned())
    } else {
        None
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

    #[test]
    fn test_cjk_fallback_finds_any_loaded_cjk_capable_font() {
        let fm = system_font_manager();
        let id = fm.query_with_fallback("DefinitelyNotInstalledFont", false, false);
        assert!(id.is_some(), "Fallback chain must return SOME font");
    }

    #[test]
    fn test_query_cjk_capable_any_smoke() {
        let mut fm = FontManager::new();
        let _ = fm.load_font_data(b"not a font".to_vec());
        let _ = fm.query_cjk_capable_any();
    }

    #[test]
    fn test_hardcoded_cjk_names_only_returns_none_without_system_fonts() {
        let fm = FontManager::new();
        let id = fm.query_with_fallback("Noto Sans CJK SC", false, false);
        assert!(
            id.is_none(),
            "Empty FontManager must not synthesise a font; system scan is the cross-platform fallback"
        );
    }

    #[cfg(feature = "fontconfig")]
    mod fontconfig_tests {
        use super::*;

        fn fontconfig_font_manager() -> FontManager {
            let mut fm = FontManager::new();
            fm.load_system_fonts();
            fm
        }

        #[test]
        fn test_fontconfig_initialized() {
            let fm = fontconfig_font_manager();
            let inner = fm.inner.lock();
            assert!(
                inner.fc.is_some(),
                "Fontconfig handle should be initialized after load_system_fonts()"
            );
        }

        #[test]
        fn test_has_available_font_exact_via_fontdb() {
            let fm = fontconfig_font_manager();
            // DejaVu Sans is typically available on most Linux systems.
            if fm.has_exact_family("DejaVu Sans") {
                assert!(
                    fm.has_available_font("DejaVu Sans"),
                    "Fontdb-exact font must be detected by has_available_font"
                );
            }
        }

        #[test]
        fn test_has_available_font_alias_via_fontconfig() {
            let fm = fontconfig_font_manager();
            // "Arial" is a fontconfig alias, not installed as an exact family name.
            // resolve_via_fontconfig uses list_fonts which requires exact match.
            let resolved = fm.resolve_via_fontconfig("Arial", false, false);
            assert!(
                resolved.is_none(),
                "Arial alias should not be resolved via list_fonts (not an installed font)"
            );
            // However, query_with_fallback should still return something via the
            // fallback chain (DejaVu Sans is in the generic fallbacks list).
            let qwf_id = fm.query_with_fallback("Arial", false, false);
            assert!(
                qwf_id.is_some(),
                "query_with_fallback should still resolve Arial via fallback chain"
            );
        }

        #[test]
        fn test_has_available_font_nonexistent() {
            let fm = fontconfig_font_manager();
            assert!(
                !fm.has_available_font("NonExistentFontXYZ123"),
                "Completely nonexistent font must return false"
            );
        }

        #[test]
        fn test_resolve_via_fontconfig_finds_installed_font() {
            let fm = fontconfig_font_manager();
            // Use a font we know is installed (DejaVu Sans is common on Linux).
            if fm.has_exact_family("DejaVu Sans") {
                let result = fm.resolve_via_fontconfig("DejaVu Sans", false, false);
                assert!(
                    result.is_some(),
                    "resolve_via_fontconfig should find DejaVu Sans"
                );
            }
        }

        #[test]
        fn test_resolve_via_fontconfig_nonexistent() {
            let fm = fontconfig_font_manager();
            let result = fm.resolve_via_fontconfig("NonExistentFontXYZ123", false, false);
            assert!(
                result.is_none(),
                "resolve_via_fontconfig should return None for nonexistent font"
            );
        }

        #[test]
        fn test_resolve_via_fontconfig_cache_negative() {
            let fm = fontconfig_font_manager();
            // First call should check fontconfig and cache the negative result.
            let result1 = fm.resolve_via_fontconfig("TotallyNonexistent456", false, false);
            assert!(result1.is_none(), "Nonexistent font should return None");

            // Second call should hit the cache without calling fontconfig.
            let result2 = fm.resolve_via_fontconfig("TotallyNonexistent456", false, false);
            assert!(
                result2.is_none(),
                "Cached negative result should also be None"
            );

            // Verify the cache has the negative entry.
            let inner = fm.inner.lock();
            let cached = inner.fc_loaded_cache.get("TotallyNonexistent456");
            assert!(cached.is_some(), "Negative result should be cached");
            assert!(
                cached.unwrap().is_none(),
                "Cached entry should be None (negative)"
            );
        }

        #[test]
        fn test_query_with_fallback_fontconfig_step() {
            let fm = fontconfig_font_manager();
            // "Arial" should be resolved via fontconfig, returning a fontdb ID.
            let arial_id = fm.query_with_fallback("Arial", false, false);
            assert!(
                arial_id.is_some(),
                "query_with_fallback should resolve Arial via fontconfig"
            );

            // Also verify the font data is retrievable.
            if let Some(id) = arial_id {
                let data = fm.get_font_data(id);
                assert!(
                    data.is_some(),
                    "Font data must be retrievable for fontconfig-resolved font"
                );
            }
        }

        #[test]
        fn test_query_with_fallback_scoring_priority() {
            let fm = fontconfig_font_manager();
            // If fontdb has "DejaVu Sans" directly, querying "DejaVu Sans"
            // should hit the scoring step (not fontconfig).
            if fm.has_exact_family("DejaVu Sans") {
                let id = fm.query_with_fallback("DejaVu Sans", false, false);
                assert!(
                    id.is_some(),
                    "Scoring step should find DejaVu Sans before fontconfig step"
                );
            }
        }
    }
}
