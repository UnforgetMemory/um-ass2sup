//! Rendering bridge: libass track management and frame rasterization.

use std::collections::HashSet;
use std::ffi::CString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::ptr;
use std::time::SystemTime;

use libass_sys;
use rayon::prelude::*;

use crate::domain::error::AssError;
use crate::domain::frame::{AssEventInfo, AssImageData, ImageType};

/// Cast any `*const T` to `*const i8` for libass FFI.
///
/// On x86_64/macOS `c_char = i8` → T is often already i8 (no-op at runtime).
/// On aarch64/Windows `c_char = u8` → required type conversion for `CString::as_ptr()` etc.
fn cast_ptr_to_i8<T>(p: *const T) -> *const i8 {
    p as *const i8
}

/// Normalize a font family name for comparison: lowercase, strip spaces/hyphens.
fn normalize_font_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Returns true if the string is plausibly a real font family name (contains at
/// least one alphabetic character).  Filters out hex color codes, override tags,
/// and other garbage that the \fn parser might pick up.
fn is_valid_font_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    name.chars().any(|c| c.is_ascii_alphabetic())
}

/// Extract all font family names referenced in an ASS subtitle file.
///
/// Parses `Style:` lines for `Fontname` and `Dialogue:` lines for `\fn` override
/// tags.  Returns a deduplicated set of normalized names.
pub fn extract_font_families(content: &str) -> HashSet<String> {
    let mut families = HashSet::new();
    let mut in_styles = false;
    let mut in_events = false;
    let mut fontname_idx: Option<usize> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("[V4+ Styles]")
            || trimmed.starts_with("[V4 Styles]")
            || trimmed.starts_with("[Styles]")
        {
            in_styles = true;
            in_events = false;
            fontname_idx = None;
            continue;
        }
        if trimmed.starts_with("[Events]") {
            in_styles = false;
            in_events = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_styles = false;
            in_events = false;
        }

        // --- Style section: find Fontname column index -----------------
        if in_styles && trimmed.starts_with("Format:") {
            for (i, field) in trimmed[7..].split(',').enumerate() {
                if field.trim().eq_ignore_ascii_case("Fontname") {
                    fontname_idx = Some(i);
                    break;
                }
            }
            continue;
        }

        // --- Style section: extract Fontname value ---------------------
        if in_styles && trimmed.starts_with("Style:") {
            if let Some(idx) = fontname_idx {
                let after_style = trimmed[6..].trim();
                let parts: Vec<&str> = after_style.splitn(idx + 2, ',').collect();
                if parts.len() > idx + 1 {
                    let fontname = parts[idx + 1].trim().trim_matches('"');
                    if is_valid_font_name(fontname) && !fontname.eq_ignore_ascii_case("Arial") {
                        families.insert(normalize_font_name(fontname));
                    }
                }
            }
            continue;
        }

        // --- Events section: find \fn override tags --------------------
        if in_events && trimmed.starts_with("Dialogue:") {
            // Text is after the 9th comma (0-indexed: field 9)
            let text = trimmed.split(',').skip(9).collect::<Vec<_>>().join(",");
            let mut pos = 0;
            let bytes = text.as_bytes();
            while pos < bytes.len() {
                if bytes[pos] == b'\\'
                    && pos + 2 < bytes.len()
                    && bytes[pos + 1] == b'f'
                    && bytes[pos + 2] == b'n'
                {
                    let start = pos + 3;
                    if start < bytes.len() && bytes[start] == b'{' {
                        // \fn{FontName}
                        if let Some(end) = text[start + 1..].find('}') {
                            let fn_name = text[start + 1..start + 1 + end].trim();
                            if is_valid_font_name(fn_name) && !fn_name.eq_ignore_ascii_case("Arial")
                            {
                                families.insert(normalize_font_name(fn_name));
                            }
                            pos = start + 1 + end + 1;
                            continue;
                        }
                    }
                    // \fnFontName (no braces)
                    let end = text[start..]
                        .find(['\\', '}', '{'])
                        .unwrap_or(text[start..].len());
                    let fn_name = text[start..start + end].trim();
                    if is_valid_font_name(fn_name) && !fn_name.eq_ignore_ascii_case("Arial") {
                        families.insert(normalize_font_name(fn_name));
                    }
                    pos = start + end;
                    continue;
                }
                pos += 1;
            }
        }
    }

    families
}

/// libass log callback — third arg is a `va_list`, not a string, so we log
/// by level only.  The actual message content is lost, but level-based
/// filtering correctly suppresses INFO-level font-select noise while still
/// surfacing WARN/ERROR to the user.
extern "C" fn libass_log_callback(level: i32, _fmt: *const i8, _va: *mut i8, _data: *mut i8) {
    match level {
        0 | 1 => tracing::error!("[libass] libass error"),
        2 => tracing::warn!("[libass] libass warning"),
        3 => tracing::debug!("[libass] libass info"),
        _ => tracing::trace!("[libass] libass debug"),
    }
}

/// Font cache file format:
/// ```text
/// [magic: 4 bytes] "ASFC"
/// [version: 4 bytes LE] 1
/// [entry_count: 4 bytes LE]
/// entries[]:
///   [name_len: 4 bytes LE][name: name_len bytes]
///   [path_len: 4 bytes LE][path: path_len bytes]
///   [mtime_sec: 8 bytes LE][mtime_nsec: 4 bytes LE]
///   [data_len: 4 bytes LE][data: data_len bytes]
/// ```
struct FontCache;

impl FontCache {
    fn cache_dir() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            std::env::var("LOCALAPPDATA")
                .ok()
                .map(|d| PathBuf::from(d).join("ass2sup"))
        }
        #[cfg(target_os = "linux")]
        {
            std::env::var("XDG_CACHE_HOME")
                .ok()
                .map(|d| PathBuf::from(d).join("ass2sup"))
                .or_else(|| {
                    std::env::var("HOME")
                        .ok()
                        .map(|h| PathBuf::from(h).join(".cache").join("ass2sup"))
                })
        }
        #[cfg(target_os = "macos")]
        {
            std::env::var("HOME").ok().map(|h| {
                PathBuf::from(h)
                    .join("Library")
                    .join("Caches")
                    .join("ass2sup")
            })
        }
        #[cfg(not(any(target_os = "windows", target_os = "linux", target_os = "macos")))]
        {
            None
        }
    }

    fn cache_path() -> Option<PathBuf> {
        Self::cache_dir().map(|d| d.join("fonts.cache"))
    }

    /// Collect font files from scan directories, optionally filtered by needed families.
    /// When `needed` is non-empty, only fonts whose filename (stem, lowercased, alnum only)
    /// contains any needed family name are included.
    fn scan_fonts(
        scan_dirs: &[String],
        needed: &HashSet<String>,
    ) -> Vec<(String, PathBuf, SystemTime)> {
        let mut fonts = Vec::new();
        for dir_path in scan_dirs {
            let dir = std::path::Path::new(dir_path);
            if !dir.is_dir() {
                continue;
            }
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if !path.is_file() {
                        continue;
                    }
                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .map(|e| e.to_lowercase())
                        .unwrap_or_default();
                    if !matches!(
                        ext.as_str(),
                        "ttf" | "otf" | "ttc" | "otc" | "woff" | "woff2"
                    ) {
                        continue;
                    }
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("font")
                        .to_string();
                    // If needed families are specified, skip fonts whose filename
                    // doesn't contain any needed family name.
                    if !needed.is_empty() {
                        let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                        let stem_norm = normalize_font_name(stem);
                        let matches = needed.iter().any(|nf| stem_norm.contains(nf));
                        if !matches {
                            continue;
                        }
                    }
                    let mtime = std::fs::metadata(&path)
                        .and_then(|m| m.modified())
                        .unwrap_or(SystemTime::UNIX_EPOCH);
                    fonts.push((name, path, mtime));
                }
            }
        }
        fonts
    }

    /// Check if a cached entry is still valid by comparing mtime.
    fn entry_valid(path: &Path, stored_mtime: SystemTime) -> bool {
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .map(|m| m == stored_mtime)
            .unwrap_or(false)
    }

    /// Try to load font data from cache. Returns None if cache is missing or invalid.
    fn load() -> Option<Vec<(String, Vec<u8>)>> {
        let path = Self::cache_path()?;
        let mut file = std::fs::File::open(&path).ok()?;

        let mut magic = [0u8; 4];
        file.read_exact(&mut magic).ok()?;
        if &magic != b"ASFC" {
            return None;
        }

        let mut version = [0u8; 4];
        file.read_exact(&mut version).ok()?;
        if u32::from_le_bytes(version) != 1 {
            return None;
        }

        let mut count_buf = [0u8; 4];
        file.read_exact(&mut count_buf).ok()?;
        let entry_count = u32::from_le_bytes(count_buf) as usize;

        let mut entries = Vec::with_capacity(entry_count);
        for _ in 0..entry_count {
            // name
            let mut nl = [0u8; 4];
            file.read_exact(&mut nl).ok()?;
            let name_len = u32::from_le_bytes(nl) as usize;
            let mut name_bytes = vec![0u8; name_len];
            file.read_exact(&mut name_bytes).ok()?;
            let name = String::from_utf8(name_bytes).ok()?;

            // path
            let mut pl = [0u8; 4];
            file.read_exact(&mut pl).ok()?;
            let path_len = u32::from_le_bytes(pl) as usize;
            let mut path_bytes = vec![0u8; path_len];
            file.read_exact(&mut path_bytes).ok()?;
            let path_str = String::from_utf8(path_bytes).ok()?;

            // mtime
            let mut sec_buf = [0u8; 8];
            file.read_exact(&mut sec_buf).ok()?;
            let mut nsec_buf = [0u8; 4];
            file.read_exact(&mut nsec_buf).ok()?;
            let duration =
                std::time::Duration::new(u64::from_le_bytes(sec_buf), u32::from_le_bytes(nsec_buf));
            let stored_mtime = SystemTime::UNIX_EPOCH + duration;

            // data
            let mut dl = [0u8; 4];
            file.read_exact(&mut dl).ok()?;
            let data_len = u32::from_le_bytes(dl) as usize;
            let mut data = vec![0u8; data_len];
            file.read_exact(&mut data).ok()?;

            // Validate mtime — skip this entry if the file changed
            let path = std::path::Path::new(&path_str);
            if Self::entry_valid(path, stored_mtime) {
                entries.push((name, data));
            }
        }

        if entries.is_empty() {
            return None;
        }
        Some(entries)
    }

    /// Update cache with actual font data (after reading files).
    fn update_with_data(fonts: &[(String, PathBuf, SystemTime)], font_data: &[(String, Vec<u8>)]) {
        let path = match Self::cache_path() {
            Some(p) => p,
            None => return,
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }

        let mut buf = Vec::new();
        buf.extend_from_slice(b"ASFC");
        buf.extend_from_slice(&1u32.to_le_bytes());
        buf.extend_from_slice(&(fonts.len() as u32).to_le_bytes());

        for (name, fpath, mtime) in fonts {
            let name_bytes = name.as_bytes();
            buf.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(name_bytes);

            let path_str = fpath.to_string_lossy();
            let path_bytes = path_str.as_bytes();
            buf.extend_from_slice(&(path_bytes.len() as u32).to_le_bytes());
            buf.extend_from_slice(path_bytes);

            let duration = mtime
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default();
            buf.extend_from_slice(&duration.as_secs().to_le_bytes());
            buf.extend_from_slice(&duration.subsec_nanos().to_le_bytes());

            // Find the data for this font
            if let Some((_, data)) = font_data.iter().find(|(n, _)| n == name) {
                buf.extend_from_slice(&(data.len() as u32).to_le_bytes());
                buf.extend_from_slice(data);
            } else {
                buf.extend_from_slice(&0u32.to_le_bytes());
            }
        }

        let _ = std::fs::write(&path, &buf);
    }
}

/// Safe Rust wrapper around libass lifecycle.
///
/// Manages `ASS_Library`, `ASS_Renderer`, and `ASS_Track` handles with
/// correct Drop ordering (track → renderer → library).
pub struct AssRenderer {
    libass: &'static libass_sys::Libass,
    library: *mut libass_sys::ASS_Library,
    renderer: *mut libass_sys::ASS_Renderer,
    track: *mut libass_sys::ASS_Track,
    width: u32,
    height: u32,
    fonts_configured: bool,
}

// libass handles are thread-safe (internally mutex-protected)
unsafe impl Send for AssRenderer {}
unsafe impl Sync for AssRenderer {}

impl AssRenderer {
    /// Create a new libass renderer for the given frame dimensions.
    ///
    /// Initializes `ASS_Library` and `ASS_Renderer`, configures frame size
    /// and storage size, and enables font extraction.
    pub fn new(width: u32, height: u32) -> Result<Self, AssError> {
        let libass = libass_sys::Libass::global().map_err(|_| AssError::InitFailed)?;
        let library = unsafe { (libass.ass_library_init)() };
        if library.is_null() {
            return Err(AssError::InitFailed);
        }

        let renderer = unsafe { (libass.ass_renderer_init)(library) };
        if renderer.is_null() {
            unsafe { (libass.ass_library_done)(library) };
            return Err(AssError::InitFailed);
        }

        unsafe {
            (libass.ass_set_frame_size)(renderer, width as i32, height as i32);
            (libass.ass_set_storage_size)(renderer, width as i32, height as i32);
            (libass.ass_set_extract_fonts)(library, 1);
            (libass.ass_set_message_cb)(library, Some(libass_log_callback), std::ptr::null_mut());
        }

        Ok(Self {
            libass,
            library,
            renderer,
            track: ptr::null_mut(),
            width,
            height,
            fonts_configured: false,
        })
    }

    /// Load ASS content from a string.
    ///
    /// Parses the ASS script using `ass_read_memory`. Any previously loaded
    /// track is freed first.
    #[allow(clippy::unnecessary_cast, reason = "c_char differs per platform")]
    pub fn load_ass(&mut self, content: &str) -> Result<(), AssError> {
        // Free any existing track
        if !self.track.is_null() {
            unsafe { (self.libass.ass_free_track)(self.track) };
            self.track = ptr::null_mut();
        }

        let cstr = CString::new(content)
            .map_err(|_| AssError::Ass("ASS content contains null byte".into()))?;

        let track = unsafe {
            (self.libass.ass_read_memory)(
                self.library,
                cast_ptr_to_i8(cstr.as_ptr()),
                content.len(),
                ptr::null(),
            )
        };

        if track.is_null() {
            return Err(AssError::Ass("ass_read_memory returned null".into()));
        }

        self.track = track;
        Ok(())
    }

    /// Core fallback fonts that should always be available regardless of the ASS
    /// file's font requirements.  These are the most common fonts libass resorts
    /// to when the requested font is missing or doesn't cover certain glyphs.
    fn fallback_fonts() -> HashSet<String> {
        let mut fb = HashSet::new();
        fb.insert(normalize_font_name("Arial"));
        fb.insert(normalize_font_name("Times New Roman"));
        fb.insert(normalize_font_name("Microsoft YaHei"));
        fb.insert(normalize_font_name("Segoe UI"));
        fb.insert(normalize_font_name("Tahoma"));
        fb.insert(normalize_font_name("DejaVu Sans"));
        fb.insert(normalize_font_name("Helvetica"));
        fb
    }

    /// Configure font lookup.
    ///
    /// Font provider selection uses `ASS_FONTPROVIDER_AUTODETECT=0` so that libass
    /// picks the platform-native provider (DirectWrite on Windows, fontconfig on
    /// Linux, CoreText on macOS).
    ///
    /// System font directories are scanned automatically based on the platform:
    ///
    /// - **Windows**: `C:\Windows\Fonts` and `%LOCALAPPDATA%\Microsoft\Windows\Fonts`
    /// - **Linux**: `/usr/share/fonts`, `/usr/local/share/fonts`, `~/.local/share/fonts`, `~/.fonts`
    /// - **macOS**: `/System/Library/Fonts`, `/Library/Fonts`, `~/Library/Fonts`
    ///
    /// In addition, all `font_dirs` provided by the user are scanned. Every font
    /// file (`.ttf`, `.otf`, `.ttc`, `.otc`, `.woff`, `.woff2`) found in any of
    /// these directories is registered with libass via [`ass_add_font`] **before**
    /// [`ass_set_fonts`] is called, so they are available to every font provider.
    /// This gives true system + user two-level font matching, regardless of the
    /// font provider in use.
    ///
    /// `font_dirs` — user-provided font directories. The first directory is also
    /// passed to [`ass_set_fonts_dir`] for embedded font extraction.
    #[allow(clippy::unnecessary_cast, reason = "c_char differs per platform")]
    pub fn configure_fonts(
        &mut self,
        default_family: Option<&str>,
        font_dirs: &[String],
        needed_families: &HashSet<String>,
    ) -> Result<(), AssError> {
        // --- 0) Build list of font directories to scan ------------------------
        let mut scan_dirs: Vec<String> = Vec::new();

        #[cfg(target_os = "windows")]
        {
            scan_dirs.push("C:\\Windows\\Fonts".to_string());
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                scan_dirs.push(format!("{}\\Microsoft\\Windows\\Fonts", local));
            }
        }

        #[cfg(target_os = "linux")]
        {
            scan_dirs.push("/usr/share/fonts".to_string());
            scan_dirs.push("/usr/local/share/fonts".to_string());
            if let Ok(home) = std::env::var("HOME") {
                scan_dirs.push(format!("{}/.local/share/fonts", home));
                scan_dirs.push(format!("{}/.fonts", home));
            }
        }

        #[cfg(target_os = "macos")]
        {
            scan_dirs.push("/System/Library/Fonts".to_string());
            scan_dirs.push("/Library/Fonts".to_string());
            if let Ok(home) = std::env::var("HOME") {
                scan_dirs.push(format!("{}/Library/Fonts", home));
            }
        }

        // Add user-provided font directories
        scan_dirs.extend(font_dirs.iter().cloned());

        // Merge ASS-needed families with the global fallback set so that
        // libass's font fallback chain (e.g. CJK → Microsoft YaHei) has
        // fonts available even when the ASS file doesn't reference them.
        let mut all_needed = needed_families.clone();
        all_needed.extend(Self::fallback_fonts());

        // --- 1) Try font cache first ---
        if let Some(cached) = FontCache::load() {
            let filtered: Vec<&(String, Vec<u8>)> = cached.iter().collect();
            let filtered: Vec<&&(String, Vec<u8>)> = if all_needed.is_empty() {
                filtered.iter().collect()
            } else {
                filtered
                    .iter()
                    .filter(|(name, _)| {
                        let stem = std::path::Path::new(name)
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or(name);
                        let stem_norm = normalize_font_name(stem);
                        all_needed.iter().any(|nf| stem_norm.contains(nf))
                    })
                    .collect()
            };
            tracing::info!("Font cache hit — {} font(s) from cache", filtered.len());
            for (name, data) in &filtered {
                if let Ok(cname) = CString::new(name.as_str()) {
                    unsafe {
                        (self.libass.ass_add_font)(
                            self.library,
                            cast_ptr_to_i8(cname.as_ptr()),
                            cast_ptr_to_i8(data.as_ptr()),
                            data.len() as i32,
                        );
                    }
                }
            }
        } else {
            // --- 2) Cache miss — scan, read, register, cache -------------
            tracing::info!("Registering fonts from {} director(ies)", scan_dirs.len());
            let mut all_needed = needed_families.clone();
            all_needed.extend(Self::fallback_fonts());
            let fonts_meta = FontCache::scan_fonts(&scan_dirs, &all_needed);
            let font_count = fonts_meta.len();
            if font_count == 0 {
                tracing::info!("  no font files found");
            } else {
                tracing::info!("  found {font_count} font file(s), reading in parallel...");

                let font_data: Vec<(String, Vec<u8>)> = fonts_meta
                    .par_iter()
                    .filter_map(|(name, path, _mtime)| {
                        std::fs::read(path).ok().map(|data| (name.clone(), data))
                    })
                    .collect();

                let loaded = font_data.len();
                tracing::info!("  read {loaded}/{font_count} font file(s)");

                for (i, (name, data)) in font_data.iter().enumerate() {
                    if let Ok(cname) = CString::new(name.as_str()) {
                        unsafe {
                            (self.libass.ass_add_font)(
                                self.library,
                                cast_ptr_to_i8(cname.as_ptr()),
                                cast_ptr_to_i8(data.as_ptr()),
                                data.len() as i32,
                            );
                        }
                    }
                    if (i + 1).is_multiple_of(50) || i + 1 == loaded {
                        tracing::info!("  registered font {}/{}", i + 1, loaded);
                    }
                }

                FontCache::update_with_data(&fonts_meta, &font_data);
                tracing::info!("  font cache written");
            }
        }

        // --- 2) Set fonts_dir for embedded font extraction (first user dir) ------
        if let Some(dir) = font_dirs.first() {
            if let Ok(cdir) = CString::new(dir.as_str()) {
                unsafe {
                    (self.libass.ass_set_fonts_dir)(self.library, cast_ptr_to_i8(cdir.as_ptr()));
                }
            }
        }

        // --- 3) Select font provider and initialize -------------------------
        let provider: i32 = 0; // ASS_FONTPROVIDER_AUTODETECT

        let family_cstr = default_family.and_then(|f| CString::new(f).ok());

        let family_ptr = family_cstr
            .as_ref()
            .map(|c| c.as_ptr())
            .unwrap_or(ptr::null());

        unsafe {
            (self.libass.ass_set_fonts)(
                self.renderer,
                ptr::null(),
                cast_ptr_to_i8(family_ptr),
                provider,
                ptr::null(),
                0,
            );
        }

        self.fonts_configured = true;
        Ok(())
    }

    /// Enable or disable hinting.
    ///
    /// `ASS_HINTING_LIGHT = 1`, `ASS_HINTING_NONE = 0`
    pub fn set_hinting(&self, hinting: i32) {
        unsafe { (self.libass.ass_set_hinting)(self.renderer, hinting) }
    }

    /// Set font scale factor.
    pub fn set_font_scale(&self, scale: f64) {
        unsafe { (self.libass.ass_set_font_scale)(self.renderer, scale) }
    }

    /// Render a single frame at the given timestamp.
    ///
    /// Returns `None` if no images were rendered (empty/transparent frame).
    /// The images are returned as safe `Vec<AssImageData>` — each image's
    /// alpha buffer is copied from libass's internal memory (which is only
    /// valid until the next `render_frame` call).
    pub fn render_frame(&self, timestamp_ms: i64) -> Result<Option<Vec<AssImageData>>, AssError> {
        if self.track.is_null() {
            return Err(AssError::Ass("No track loaded".into()));
        }

        let mut detect_change: i32 = 0;
        let image = unsafe {
            (self.libass.ass_render_frame)(
                self.renderer,
                self.track,
                timestamp_ms,
                &mut detect_change,
            )
        };

        if image.is_null() {
            return Ok(None);
        }

        let mut images = Vec::new();
        let mut current = image;

        while !current.is_null() {
            let img = unsafe { &*current };

            let w = img.w.max(0) as u32;
            let h = img.h.max(0) as u32;
            let stride = img.stride.max(0) as u32;

            // Copy alpha buffer from libass's internal memory
            let bitmap = if w > 0 && h > 0 && !img.bitmap.is_null() {
                let mut buf = Vec::with_capacity((stride * h) as usize);
                unsafe {
                    std::ptr::copy_nonoverlapping(img.bitmap, buf.as_mut_ptr(), buf.capacity());
                    buf.set_len(buf.capacity());
                }
                buf
            } else {
                Vec::new()
            };

            images.push(AssImageData {
                w,
                h,
                stride,
                bitmap,
                color: img.color,
                dst_x: img.dst_x.max(0) as u32,
                dst_y: img.dst_y.max(0) as u32,
                image_type: ImageType::from(img.image_type),
            });

            current = img.next;
        }

        Ok(Some(images))
    }

    /// Return parsed event metadata from the loaded track.
    ///
    /// Reads `n_events` and the `events` array from `ASS_Track`.
    /// Returns an empty vec if no track is loaded or no events exist.
    pub fn events(&self) -> Vec<AssEventInfo> {
        if self.track.is_null() {
            return Vec::new();
        }

        let track = unsafe { &*self.track };

        let n_events = track.n_events.max(0) as usize;
        if n_events == 0 || track.events.is_null() {
            return Vec::new();
        }

        let mut events = Vec::with_capacity(n_events);
        for i in 0..n_events {
            let event = unsafe { &*(track.events.add(i)) };
            let text = if !event.text.is_null() {
                unsafe { std::ffi::CStr::from_ptr(event.text as *const std::os::raw::c_char) }
                    .to_string_lossy()
                    .into_owned()
            } else {
                String::new()
            };

            events.push(AssEventInfo {
                start_ms: event.start,
                duration_ms: event.duration,
                style: event.style,
                text,
            });
        }

        events
    }

    /// Returns the PlayResX from the loaded track, or the configured width.
    pub fn play_res_x(&self) -> u32 {
        if self.track.is_null() {
            return self.width;
        }
        let track = unsafe { &*self.track };
        let res = track.play_res_x.max(0) as u32;
        if res == 0 {
            self.width
        } else {
            res
        }
    }

    /// Returns the PlayResY from the loaded track, or the configured height.
    pub fn play_res_y(&self) -> u32 {
        if self.track.is_null() {
            return self.height;
        }
        let track = unsafe { &*self.track };
        let res = track.play_res_y.max(0) as u32;
        if res == 0 {
            self.height
        } else {
            res
        }
    }

    /// Returns the number of events in the loaded track.
    pub fn num_events(&self) -> usize {
        if self.track.is_null() {
            return 0;
        }
        let track = unsafe { &*self.track };
        track.n_events.max(0) as usize
    }
}

impl Drop for AssRenderer {
    fn drop(&mut self) {
        if !self.track.is_null() {
            unsafe { (self.libass.ass_free_track)(self.track) };
        }
        if !self.renderer.is_null() {
            unsafe { (self.libass.ass_renderer_done)(self.renderer) };
        }
        if !self.library.is_null() {
            unsafe { (self.libass.ass_library_done)(self.library) };
        }
    }
}

impl std::fmt::Debug for AssRenderer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssRenderer")
            .field("width", &self.width)
            .field("height", &self.height)
            .field("fonts_configured", &self.fonts_configured)
            .field("track_loaded", &(!self.track.is_null()))
            .field("num_events", &self.num_events())
            .finish()
    }
}
