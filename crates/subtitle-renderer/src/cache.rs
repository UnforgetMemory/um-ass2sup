use std::collections::HashMap;
use std::sync::RwLock;

use crate::context::RenderedFrame;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct FrameCacheKey {
    pub event_index: usize,
    pub timestamp_ms: u64,
}

pub struct FrameCache {
    cache: RwLock<HashMap<FrameCacheKey, RenderedFrame>>,
    max_entries: usize,
}

impl FrameCache {
    pub fn new(max_entries: usize) -> Self {
        Self {
            cache: RwLock::new(HashMap::with_capacity(max_entries)),
            max_entries,
        }
    }

    pub fn get(&self, key: &FrameCacheKey) -> Option<RenderedFrame> {
        let cache = self.cache.read().ok()?;
        cache.get(key).cloned()
    }

    pub fn insert(&self, key: FrameCacheKey, frame: RenderedFrame) {
        let mut cache = match self.cache.write() {
            Ok(c) => c,
            Err(_) => return,
        };

        if cache.len() >= self.max_entries {
            if let Some(first_key) = cache.keys().next().cloned() {
                cache.remove(&first_key);
            }
        }

        cache.insert(key, frame);
    }

    pub fn contains(&self, key: &FrameCacheKey) -> bool {
        self.cache
            .read()
            .map(|c| c.contains_key(key))
            .unwrap_or(false)
    }

    pub fn len(&self) -> usize {
        self.cache.read().map(|c| c.len()).unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        if let Ok(mut cache) = self.cache.write() {
            cache.clear();
        }
    }
}

impl Default for FrameCache {
    fn default() -> Self {
        Self::new(1024)
    }
}

pub fn make_frame_key(event_index: usize, timestamp_ms: u64) -> FrameCacheKey {
    FrameCacheKey {
        event_index,
        timestamp_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_frame(pts_ms: u64) -> RenderedFrame {
        RenderedFrame {
            pts_ms,
            duration_ms: 1000,
            width: 1920,
            height: 1080,
            bitmap: vec![0u8; 1920 * 1080 * 4],
        }
    }

    #[test]
    fn test_cache_basic_ops() {
        let cache = FrameCache::new(10);
        let key = FrameCacheKey {
            event_index: 0,
            timestamp_ms: 1000,
        };

        assert!(cache.is_empty());
        assert!(!cache.contains(&key));

        cache.insert(key.clone(), test_frame(1000));
        assert!(cache.contains(&key));
        assert_eq!(cache.len(), 1);

        let frame = cache.get(&key);
        assert!(frame.is_some());
        assert_eq!(frame.unwrap().pts_ms, 1000);
    }

    #[test]
    fn test_cache_eviction() {
        let cache = FrameCache::new(2);

        for i in 0..4 {
            let key = FrameCacheKey {
                event_index: i,
                timestamp_ms: i as u64 * 1000,
            };
            cache.insert(key, test_frame((i as u64) * 1000));
        }

        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_cache_clear() {
        let cache = FrameCache::new(10);
        let key = FrameCacheKey {
            event_index: 0,
            timestamp_ms: 1000,
        };

        cache.insert(key.clone(), test_frame(1000));
        assert!(!cache.is_empty());

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_make_frame_key() {
        let key = make_frame_key(42, 5000);
        assert_eq!(key.event_index, 42);
        assert_eq!(key.timestamp_ms, 5000);
    }
}
