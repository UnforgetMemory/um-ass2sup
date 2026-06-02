use subtitle_renderer::{FrameCache, FrameCacheKey, RenderedFrame, make_frame_key};
use std::sync::Arc;
use std::thread;

fn test_frame(pts_ms: u64) -> RenderedFrame {
    RenderedFrame {
        pts_ms,
        duration_ms: 1000,
        width: 1920,
        height: 1080,
        bitmap: vec![0u8; 1920 * 1080 * 4],
    }
}

fn key(event: usize, ts: u64) -> FrameCacheKey {
    FrameCacheKey { event_index: event, timestamp_ms: ts }
}

#[test]
fn integration_cache_insert_and_retrieve() {
    let cache = FrameCache::new(16);
    let k = key(0, 1000);
    assert!(cache.get(&k).is_none());

    cache.insert(k.clone(), test_frame(1000));
    assert!(cache.contains(&k));
    let frame = cache.get(&k).unwrap();
    assert_eq!(frame.pts_ms, 1000);
    assert_eq!(frame.width, 1920);
}

#[test]
fn integration_cache_fifo_eviction_boundary() {
    let cache = FrameCache::new(4);
    let keys: Vec<_> = (0..8usize).map(|i| key(i, i as u64 * 1000)).collect();
    for (i, k) in keys.iter().enumerate() {
        cache.insert(k.clone(), test_frame(i as u64 * 1000));
    }

    assert_eq!(cache.len(), 4);
    let evicted_count = keys.iter().filter(|k| !cache.contains(k)).count();
    assert_eq!(evicted_count, 4, "half the entries should be evicted");
}

#[test]
fn integration_cache_clear_resets_state() {
    let cache = FrameCache::new(100);
    for i in 0..50usize {
        cache.insert(key(i, i as u64 * 100), test_frame(i as u64 * 100));
    }
    assert_eq!(cache.len(), 50);

    cache.clear();
    assert!(cache.is_empty());
    assert!(!cache.contains(&key(0, 0)));
}

#[test]
fn integration_cache_overwrite_preserves_capacity() {
    let cache = FrameCache::new(5);
    let shared_key = key(0, 1000);
    for i in 0..10u64 {
        let k = FrameCacheKey { event_index: 0, timestamp_ms: 1000 };
        cache.insert(k, test_frame(i * 1000));
    }
    assert_eq!(cache.len(), 1, "overwriting same key should not grow cache");
    assert_eq!(cache.get(&shared_key).unwrap().pts_ms, 9000);
}

#[test]
fn integration_cache_thread_safety_stress() {
    let cache = Arc::new(FrameCache::new(256));
    let mut handles = vec![];

    for thread_id in 0..8 {
        let c = Arc::clone(&cache);
        handles.push(thread::spawn(move || {
            for i in 0..100 {
                let k = key(thread_id * 100 + i, (thread_id * 100 + i) as u64 * 10);
                c.insert(k, test_frame((thread_id * 100 + i) as u64 * 10));
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    assert!(cache.len() <= 256, "cache should never exceed max_entries");
}

#[test]
fn integration_make_frame_key_deterministic() {
    let k1 = make_frame_key(7, 3500);
    let k2 = make_frame_key(7, 3500);
    assert_eq!(k1, k2);
    assert_eq!(k1.event_index, 7);
    assert_eq!(k1.timestamp_ms, 3500);
}

#[test]
fn integration_cache_different_events_same_time() {
    let cache = FrameCache::new(10);
    let k1 = key(0, 5000);
    let k2 = key(1, 5000);
    cache.insert(k1.clone(), test_frame(5000));
    cache.insert(k2.clone(), test_frame(5000));
    assert!(cache.contains(&k1));
    assert!(cache.contains(&k2));
    assert_eq!(cache.len(), 2);
}

#[test]
fn integration_cache_same_event_different_times() {
    let cache = FrameCache::new(10);
    let k1 = key(0, 1000);
    let k2 = key(0, 2000);
    cache.insert(k1.clone(), test_frame(1000));
    cache.insert(k2.clone(), test_frame(2000));
    assert!(cache.contains(&k1));
    assert!(cache.contains(&k2));
    assert_eq!(cache.len(), 2);
    assert_eq!(cache.get(&k1).unwrap().pts_ms, 1000);
    assert_eq!(cache.get(&k2).unwrap().pts_ms, 2000);
}
