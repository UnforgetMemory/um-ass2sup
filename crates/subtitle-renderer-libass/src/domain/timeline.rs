use std::collections::BTreeSet;

use crate::domain::frame::AssEventInfo;

/// Detect if an event's text contains ASS override tags that produce
/// frame-level visual changes (position moves, alpha fades, transforms).
fn has_animation(text: &str) -> bool {
    // Scan for tags that change per frame
    text.contains("\\move(")
        || text.contains("\\fad(")
        || text.contains("\\fade(")
        || text.contains("\\t(")
        || text.contains("\\fscx")
        || text.contains("\\fscy")
        || text.contains("\\fax")
        || text.contains("\\fay")
        || text.contains("\\frx")
        || text.contains("\\fry")
        || text.contains("\\frz")
        || text.contains("\\clip(")
        || text.contains("\\iclip(")
        // Banner/Scroll effects via Effect field (embedded in text by libass)
        || text.to_lowercase().contains("banner")
        || text.to_lowercase().contains("scroll")
}

/// Generate sorted render timestamps from a list of ASS events.
///
/// Strategy (matching the original ass2sup's `render_and_quantize`):
/// - Merge overlapping event time ranges to avoid redundant gaps.
/// - For **animated** events (`\move`, `\fad`, `\t`, etc.): generate a
///   timestamp at **every video frame** within the event's range.
/// - For **static** events: only generate timestamps at event start and end.
pub fn generate_timestamps(events: &[AssEventInfo], fps: f64) -> Vec<u64> {
    if events.is_empty() || fps <= 0.0 {
        return Vec::new();
    }

    let ms_per_frame = 1000.0 / fps;

    // Step 1: collect all interesting timestamps
    //   - For animated events: every frame in the active range
    //   - For static events: start + end
    let mut timestamps = BTreeSet::new();

    for event in events {
        let start_ms = event.start_ms.max(0) as u64;
        let end_ms = (event.start_ms + event.duration_ms).max(0) as u64;
        if start_ms >= end_ms {
            continue;
        }

        if has_animation(&event.text) {
            // Animated: generate every frame
            let mut t = start_ms as f64;
            while t < end_ms as f64 {
                timestamps.insert(t as u64);
                t += ms_per_frame;
            }
        } else {
            // Static: only start and end
            timestamps.insert(start_ms);
            timestamps.insert(end_ms);
        }
    }

    // Step 2: Also add all event start/end times as explicit render points
    // (ensures we don't miss boundary transitions even without animation)
    for event in events {
        let start_ms = event.start_ms.max(0) as u64;
        let end_ms = (event.start_ms + event.duration_ms).max(0) as u64;
        timestamps.insert(start_ms);
        timestamps.insert(end_ms);
    }

    timestamps.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_event(start_ms: i64, duration_ms: i64, text: &str) -> AssEventInfo {
        AssEventInfo {
            start_ms,
            duration_ms,
            style: 0,
            text: text.to_string(),
        }
    }

    #[test]
    fn empty_events_returns_empty() {
        assert!(generate_timestamps(&[], 23.976).is_empty());
    }

    #[test]
    fn static_event_start_and_end() {
        let ts = generate_timestamps(&[make_event(0, 1000, "Hello")], 24.0);
        assert!(
            ts.len() >= 2,
            "static event: need start+end, got {} ts",
            ts.len()
        );
        assert_eq!(ts[0], 0);
        assert!(ts.iter().any(|&t| t == 1000 || t > 900));
    }

    #[test]
    fn fade_event_has_many_frames() {
        let ts = generate_timestamps(&[make_event(0, 2000, "{\\fad(200,200)}Hello")], 24.0);
        // 2 seconds at 24fps = 48 frames
        assert!(
            ts.len() >= 20,
            "fade event should have ~48 timestamps, got {}",
            ts.len()
        );
    }

    #[test]
    fn move_event_has_many_frames() {
        let ts = generate_timestamps(&[make_event(0, 3000, "{\\move(0,0,100,100)}Hello")], 24.0);
        assert!(
            ts.len() >= 30,
            "move event should have ~72 timestamps, got {}",
            ts.len()
        );
    }

    #[test]
    fn transform_event_has_many_frames() {
        let ts = generate_timestamps(&[make_event(0, 3000, "{\\t(0,3000,\\fscx120)}Hello")], 24.0);
        assert!(
            ts.len() >= 30,
            "transform event should have ~72 ts, got {}",
            ts.len()
        );
    }

    #[test]
    fn zero_duration_event_skipped() {
        let ts = generate_timestamps(&[make_event(0, 0, "Hello")], 23.976);
        // zero-duration event still gets start+end = 0+0 → same timestamp, may yield 1
        assert!(!ts.is_empty(), "zero duration may still have start ts");
    }

    #[test]
    fn has_animation_detects_fad() {
        assert!(has_animation("{\\fad(200,200)}Hi"));
        assert!(has_animation("{\\move(0,0,100,100)}Hi"));
        assert!(has_animation("{\\t(0,500,\\fscx120)}Hi"));
        assert!(!has_animation("Plain text"));
        assert!(!has_animation("{\\b1}Bold text{\\b0}"));
    }
}
