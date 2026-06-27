//! Utility functions shared across the conversion pipeline.

use std::collections::BTreeSet;

use ass_core::Event;

/// Crop a rendered RGBA bitmap to the tight bounding box of non-transparent pixels.
///
/// Returns `(cropped_rgba, x, y, w, h)` or `None` if the frame is entirely transparent.
pub fn crop_to_tight_bbox(
    bitmap: &[u8],
    width: u32,
    height: u32,
) -> Option<(Vec<u8>, u32, u32, u32, u32)> {
    if bitmap.len() != (width as usize) * (height as usize) * 4 {
        return None;
    }
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any = false;

    for y in 0..height {
        for x in 0..width {
            let off = ((y * width + x) * 4) as usize;
            if bitmap[off + 3] > 0 {
                any = true;
                if x < min_x {
                    min_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if x > max_x {
                    max_x = x;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }

    if !any {
        return None;
    }

    let w = max_x - min_x + 1;
    let h = max_y - min_y + 1;
    let mut out = Vec::with_capacity((w as usize) * (h as usize) * 4);

    for y in min_y..=max_y {
        let row_start = ((y * width + min_x) * 4) as usize;
        let row_end = row_start + (w as usize) * 4;
        out.extend_from_slice(&bitmap[row_start..row_end]);
    }

    Some((out, min_x, min_y, w, h))
}

/// Generate frame timeline timestamps from events and fps.
///
/// Returns a sorted `Vec<u64>` of millisecond timestamps only for time ranges
/// where at least one event is active. Skips gaps between events to avoid
/// generating frames for empty periods.
/// Uses float-based per-frame computation to avoid drift at non-integer fps.
pub fn generate_frame_timeline(events: &[Event], fps: f64) -> Vec<u64> {
    if events.is_empty() || fps <= 0.0 {
        return Vec::new();
    }

    let ms_per_frame = 1000.0 / fps;

    // Merge overlapping event time ranges
    let mut ranges: Vec<(u64, u64)> = events.iter().map(|e| (e.start_ms, e.end_ms)).collect();
    ranges.sort_by_key(|r| r.0);

    let mut merged: Vec<(u64, u64)> = Vec::new();
    for (start, end) in ranges {
        if let Some(last) = merged.last_mut() {
            if start <= last.1 {
                last.1 = last.1.max(end);
                continue;
            }
        }
        merged.push((start, end));
    }

    // Generate frames only for merged ranges
    let mut timestamps: BTreeSet<u64> = BTreeSet::new();
    for (range_start, range_end) in merged {
        let mut frame_index = 0u64;
        loop {
            let pts = range_start as f64 + frame_index as f64 * ms_per_frame;
            if pts >= range_end as f64 {
                break;
            }
            timestamps.insert(pts as u64);
            frame_index += 1;
        }
    }

    timestamps.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ass_core::{Effect, Event, EventType, StyleRef};

    fn make_event(start_ms: u64, end_ms: u64) -> Event {
        Event {
            source_line: 1,
            event_type: EventType::Dialogue,
            layer: 0,
            start_ms,
            end_ms,
            style: StyleRef::new("Default"),
            actor: String::new(),
            margin_l: None,
            margin_r: None,
            margin_v: None,
            effect: Effect::None,
            text_raw: "Hello".into(),
            override_tags: Vec::new(),
            karaoke: Vec::new(),
        }
    }

    #[test]
    fn test_empty_events() {
        let result = generate_frame_timeline(&[], 23.976);
        assert!(
            result.is_empty(),
            "empty events should produce empty timeline"
        );
    }

    #[test]
    fn test_single_event() {
        let events = vec![make_event(0, 1000)];
        let result = generate_frame_timeline(&events, 23.976);
        assert!(!result.is_empty());
        assert_eq!(result.len(), 24);
        assert_eq!(result[0], 0);
    }

    #[test]
    fn test_ntsc_frame_rate() {
        let events = vec![make_event(0, 2002)];
        let result = generate_frame_timeline(&events, 24000.0 / 1001.0);
        // ~48 frames expected; float boundary may produce 48 or 49
        assert!(!result.is_empty());
        assert!(result.len() == 48 || result.len() == 49);
        assert_eq!(result[0], 0);
    }

    #[test]
    fn test_gap_between_events() {
        let events = vec![make_event(0, 500), make_event(2000, 3000)];
        let result = generate_frame_timeline(&events, 23.976);
        assert!(!result.is_empty());
        assert_eq!(result[0], 0);
        assert!(*result.last().unwrap() < 3000);
        assert!(result.len() > 24);
    }

    #[test]
    fn test_sorted_and_unique() {
        let events = vec![make_event(0, 5000)];
        let result = generate_frame_timeline(&events, 23.976);
        for w in result.windows(2) {
            assert!(
                w[0] < w[1],
                "timestamps must be strictly increasing and unique"
            );
        }
    }

    #[test]
    fn test_single_frame_duration() {
        let events = vec![make_event(0, 1)];
        let result = generate_frame_timeline(&events, 23.976);
        assert!(!result.is_empty());
        assert_eq!(result[0], 0);
    }

    #[test]
    fn test_zero_fps() {
        let events = vec![make_event(0, 1000)];
        let result = generate_frame_timeline(&events, 0.0);
        assert!(result.is_empty(), "zero fps should return empty timeline");
    }

    #[test]
    fn test_negative_fps() {
        let events = vec![make_event(0, 1000)];
        let result = generate_frame_timeline(&events, -1.0);
        assert!(
            result.is_empty(),
            "negative fps should return empty timeline"
        );
    }
}
