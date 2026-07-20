//! Font file cache — persists font data to disk so subsequent runs can skip
//! the expensive directory scan + parallel read phase.
//!
//! ## Format
//!
//! ```text
//! [magic: 4 bytes] "ASFC"
//! [version: 4 bytes LE] 1
//! [entry_count: 4 bytes LE]
//! entries[]:
//!   [name_len: 4 bytes LE][name: name_len bytes]
//!   [path_len: 4 bytes LE][path: path_len bytes]
//!   [mtime_sec: 8 bytes LE][mtime_nsec: 4 bytes LE]
//!   [data_len: 4 bytes LE][data: data_len bytes]
//! ```

use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Normalize a font family name for comparison: lowercase, strip spaces/hyphens.
fn normalize_font_name(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .collect()
}

/// Font file cache persisted to disk.
pub struct FontCache;

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
    pub fn scan_fonts(
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
    pub fn load() -> Option<Vec<(String, Vec<u8>)>> {
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
    pub fn update_with_data(
        fonts: &[(String, PathBuf, SystemTime)],
        font_data: &[(String, Vec<u8>)],
    ) {
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
