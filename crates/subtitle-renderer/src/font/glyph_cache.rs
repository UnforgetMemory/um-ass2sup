//! Cross-frame glyph rasterization cache.
//!
//! Without this cache, every active subtitle event re-rasterizes every glyph on
//! every frame via swash (`FontRef::from_index` + `ScaleContext` + `Render`),
//! even when the same glyph at the same size is rendered identically across
//! dozens of consecutive frames (static dialogue, fade holds, credits stacks).
//!
//! The key is `(font-data identity, glyph id, exact size bits)`. The font-data
//! identity is the heap address of the shared `Arc<[u8]>` font payload — stable
//! for the lifetime of the process because the persistent font-data cache holds
//! one `Arc` per resolved font. Glyph bitmaps are stored as `Arc` so the hot
//! path pays one refcount bump per hit, not a copy.
//!
//! Eviction is a simple LRU (recency list + hash map) capped by total bytes,
//! not entry count — CJK glyph bitmaps are much larger than Latin ones, so a
//! byte budget bounds memory correctly.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

use super::types::RasterizedGlyph;

/// Cache key: font data allocation pointer + glyph id + exact size bits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    /// `Arc::as_ptr()` of the shared font data (stable per resolved font).
    pub font: usize,
    /// Swash glyph id.
    pub glyph: u16,
    /// `font_size.to_bits()` — exact f32 bits so distinct sizes never collide.
    pub size: u32,
}

/// LRU glyph bitmap cache with a byte-budgeted memory cap.
pub struct GlyphCache {
    map: HashMap<GlyphKey, Arc<RasterizedGlyph>>,
    order: VecDeque<GlyphKey>,
    bytes: usize,
    cap: usize,
}

impl GlyphCache {
    /// Create a cache with the given memory budget in bytes.
    pub fn new(cap: usize) -> Self {
        Self {
            map: HashMap::new(),
            order: VecDeque::new(),
            bytes: 0,
            cap,
        }
    }

    /// Look up a glyph; on hit, returns a shared `Arc` and refreshes recency.
    pub fn get(&mut self, key: &GlyphKey) -> Option<Arc<RasterizedGlyph>> {
        let hit = self.map.get(key)?;
        // Refresh recency: move the key to the back of the recency list.
        if self.order.back() != Some(key) {
            if let Some(pos) = self.order.iter().position(|k| k == key) {
                self.order.remove(pos);
                self.order.push_back(*key);
            }
        }
        Some(Arc::clone(hit))
    }

    /// Insert a glyph, evicting least-recently-used entries while over budget.
    /// Returns the shared `Arc` (may be the freshly inserted one).
    pub fn insert(&mut self, key: GlyphKey, glyph: RasterizedGlyph) -> Arc<RasterizedGlyph> {
        let bytes = glyph_size(&glyph);
        // Replace an existing entry for the same key (budget stays stable).
        if let Some(prev) = self.map.get(&key) {
            self.bytes = self.bytes.saturating_sub(glyph_size(prev));
        }
        let arc = Arc::new(glyph);
        self.map.insert(key, Arc::clone(&arc));
        if self.order.back() != Some(&key) {
            if let Some(pos) = self.order.iter().position(|k| k == &key) {
                self.order.remove(pos);
            }
            self.order.push_back(key);
        }
        self.bytes += bytes;
        self.evict();
        arc
    }

    /// Current estimated memory usage in bytes.
    pub fn bytes_used(&self) -> usize {
        self.bytes
    }

    /// Number of cached glyphs.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether the cache holds no glyphs.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    fn evict(&mut self) {
        // Never evict the last entry, even if a single glyph alone exceeds the
        // budget (with a 64 MiB cap this only matters in tests / pathological
        // fonts); eviction frees room for the *next* insert.
        while self.bytes > self.cap && self.order.len() > 1 {
            let oldest = self
                .order
                .pop_front()
                .expect("order non-empty while evicting");
            if let Some(evicted) = self.map.remove(&oldest) {
                self.bytes = self.bytes.saturating_sub(glyph_size(&evicted));
            }
        }
    }
}

fn glyph_size(g: &RasterizedGlyph) -> usize {
    g.data.len() + std::mem::size_of::<RasterizedGlyph>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn glyph(w: u32, h: u32) -> RasterizedGlyph {
        RasterizedGlyph {
            data: vec![0u8; (w * h) as usize],
            width: w,
            height: h,
            left: 0,
            top: 0,
        }
    }

    fn key(font: usize, glyph: u16, size: f32) -> GlyphKey {
        GlyphKey {
            font,
            glyph,
            size: size.to_bits(),
        }
    }

    #[test]
    fn insert_and_hit_returns_identical_data() {
        let mut c = GlyphCache::new(1024 * 1024);
        let k = key(0x10, 5, 48.0);
        let g = glyph(32, 32);
        let arc = c.insert(k, g);
        assert_eq!(arc.data.len(), 32 * 32);
        let hit = c.get(&k).expect("hit after insert");
        assert_eq!(hit.data, arc.data);
        assert_eq!(c.len(), 1);
    }

    #[test]
    fn miss_returns_none() {
        let mut c = GlyphCache::new(1024 * 1024);
        assert!(c.get(&key(0x10, 5, 48.0)).is_none());
    }

    #[test]
    fn distinct_sizes_are_distinct_entries() {
        let mut c = GlyphCache::new(1024 * 1024);
        let k16 = key(0x10, 5, 16.0);
        let k48 = key(0x10, 5, 48.0);
        c.insert(k16, glyph(8, 8));
        c.insert(k48, glyph(32, 32));
        assert_eq!(c.len(), 2);
        assert!(c.get(&k16).is_some() && c.get(&k48).is_some());
    }

    #[test]
    fn evicts_oldest_when_over_budget() {
        // Cap fits 2 glyphs (~10.3 KB each incl. header) but not 3.
        let cap = 25_000;
        let mut c = GlyphCache::new(cap);
        let k1 = key(0x10, 1, 48.0);
        let k2 = key(0x10, 2, 48.0);
        let k3 = key(0x10, 3, 48.0);
        c.insert(k1, glyph(80, 128));
        c.insert(k2, glyph(80, 128));
        assert!(c.get(&k1).is_some() && c.get(&k2).is_some());
        c.insert(k3, glyph(80, 128));
        assert!(c.get(&k1).is_none(), "oldest must be evicted");
        assert!(c.get(&k2).is_some());
        assert!(c.get(&k3).is_some());
        assert!(c.bytes_used() <= cap);
    }

    #[test]
    fn get_refreshes_recency() {
        let cap = 25_000;
        let mut c = GlyphCache::new(cap);
        let k1 = key(0x10, 1, 48.0);
        let k2 = key(0x10, 2, 48.0);
        let k3 = key(0x10, 3, 48.0);
        c.insert(k1, glyph(80, 128));
        c.insert(k2, glyph(80, 128));
        // Touch k1 → k2 becomes the LRU.
        let _ = c.get(&k1);
        c.insert(k3, glyph(80, 128));
        assert!(c.get(&k1).is_some(), "touched entry must survive");
        assert!(c.get(&k2).is_none(), "untouched entry must be evicted");
        assert!(c.get(&k3).is_some());
    }

    #[test]
    fn oversized_single_glyph_does_not_panic() {
        // A single glyph larger than the budget still gets inserted (it simply
        // exceeds the cap; eviction only kicks in when more entries exist).
        let mut c = GlyphCache::new(1024);
        c.insert(key(0x10, 1, 48.0), glyph(200, 200)); // 40000 bytes > cap
        assert_eq!(c.len(), 1);
        assert!(c.get(&key(0x10, 1, 48.0)).is_some());
    }
}
