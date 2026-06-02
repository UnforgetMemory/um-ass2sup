use std::collections::HashMap;
use std::sync::RwLock;

use crate::context::RenderedFrame;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct FrameCacheKey {
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

pub fn make_frame_key(timestamp_ms: u64) -> FrameCacheKey {
    FrameCacheKey { timestamp_ms }
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
        let key = FrameCacheKey { timestamp_ms: 1000 };

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
            let key = FrameCacheKey { timestamp_ms: i as u64 * 1000 };
            cache.insert(key, test_frame((i as u64) * 1000));
        }

        assert_eq!(cache.len(), 2);
    }

    #[test]
    fn test_cache_clear() {
        let cache = FrameCache::new(10);
        let key = FrameCacheKey { timestamp_ms: 1000 };

        cache.insert(key.clone(), test_frame(1000));
        assert!(!cache.is_empty());

        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn test_make_frame_key() {
        let key = make_frame_key(5000);
        assert_eq!(key.timestamp_ms, 5000);
    }

    #[test]
    fn test_cache_eviction_removes_first_inserted() {
        let cache = FrameCache::new(3);
        let k1 = FrameCacheKey { timestamp_ms: 1000 };
        let k2 = FrameCacheKey { timestamp_ms: 2000 };
        let k3 = FrameCacheKey { timestamp_ms: 3000 };
        let k4 = FrameCacheKey { timestamp_ms: 4000 };

        cache.insert(k1.clone(), test_frame(1000));
        cache.insert(k2.clone(), test_frame(2000));
        cache.insert(k3.clone(), test_frame(3000));
        cache.insert(k4.clone(), test_frame(4000));

        assert_eq!(cache.len(), 3, "cache should stay at max capacity");
        let evicted_count = [&k1, &k2, &k3, &k4]
            .iter()
            .filter(|k| !cache.contains(k))
            .count();
        assert_eq!(evicted_count, 1, "exactly one entry should be evicted");
    }

    #[test]
    fn test_cache_get_missing_key_returns_none() {
        let cache = FrameCache::new(10);
        let key = FrameCacheKey { timestamp_ms: 9999 };
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn test_cache_overwrite_existing_key() {
        let cache = FrameCache::new(10);
        let key = FrameCacheKey { timestamp_ms: 1000 };

        cache.insert(key.clone(), test_frame(1000));
        assert_eq!(cache.get(&key).unwrap().pts_ms, 1000);

        // Overwrite with a different frame
        cache.insert(key.clone(), test_frame(2000));
        assert_eq!(cache.get(&key).unwrap().pts_ms, 2000);
        assert_eq!(cache.len(), 1, "overwrite should not increase count");
    }

    #[test]
    fn test_cache_insert_past_capacity_evicts_multiple() {
        let cache = FrameCache::new(2);
        for i in 0..6 {
            let key = FrameCacheKey { timestamp_ms: i as u64 * 1000 };
            cache.insert(key, test_frame(i as u64 * 1000));
        }
        assert_eq!(cache.len(), 2, "cache should never exceed max_entries");
        let present: Vec<_> = (0..6)
            .filter(|i| {
                let k = FrameCacheKey { timestamp_ms: *i as u64 * 1000 };
                cache.contains(&k)
            })
            .collect();
        assert_eq!(present.len(), 2, "exactly 2 entries should remain");
    }

    #[test]
    fn test_cache_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let cache = Arc::new(FrameCache::new(100));
        let mut handles = vec![];

        // Spawn writer threads
        for i in 0..10 {
            let c = Arc::clone(&cache);
            handles.push(thread::spawn(move || {
                for j in 0..50 {
                    let key = FrameCacheKey {
                        timestamp_ms: (i * 50 + j) as u64 * 100,
                    };
                    c.insert(key, test_frame((i * 50 + j) as u64 * 100));
                }
            }));
        }

        // Spawn reader threads
        for i in 0..5 {
            let c = Arc::clone(&cache);
            handles.push(thread::spawn(move || {
                for j in 0..50 {
                    let key = FrameCacheKey {
                        timestamp_ms: (i * 50 + j) as u64 * 100,
                    };
                    let _ = c.get(&key);
                    let _ = c.contains(&key);
                }
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        // Cache should not exceed capacity
        assert!(cache.len() <= 100);
    }

    #[test]
    fn test_frame_key_hash_consistency() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let k1 = FrameCacheKey { timestamp_ms: 3000 };
        let k2 = FrameCacheKey { timestamp_ms: 3000 };
        let k3 = FrameCacheKey { timestamp_ms: 3001 };

        let mut h1 = DefaultHasher::new();
        let mut h2 = DefaultHasher::new();
        let mut h3 = DefaultHasher::new();

        k1.hash(&mut h1);
        k2.hash(&mut h2);
        k3.hash(&mut h3);

        assert_eq!(h1.finish(), h2.finish(), "same keys should produce same hash");
        assert_ne!(h1.finish(), h3.finish(), "different keys should produce different hashes");
    }

    #[test]
    fn test_make_frame_key_uniqueness() {
        let k1 = make_frame_key(1000);
        let k2 = make_frame_key(2000);
        let k3 = make_frame_key(1000);
        assert_ne!(k1, k2, "different timestamps should be different keys");
        assert_eq!(k1, k3, "same timestamps should be equal keys");
    }
}
