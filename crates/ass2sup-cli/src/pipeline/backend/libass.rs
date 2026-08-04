//! Libass rendering backend (C library via FFI).
//!
//! Wraps the [`subtitle_renderer_libass`] crate to render ASS content
//! through libass, then crops, quantises, and returns [`QuantizedFrame`]s.

use std::collections::{BinaryHeap, HashMap};

use ass_core::SubtitleDocument;
use color_quantizer::QuantizedFrame;
use tracing::debug;

use crate::cli::args::Args;
use crate::cli::progress::ProgressReporter;
use crate::config::Config;
use crate::error::CliError;

/// Sweep-line active-event tracker.
///
/// Replaces the previous O(n×m) `events.iter().any(...)` scan per timestamp
/// with a single sort plus a min-heap of active end times. Semantics match the
/// original half-open interval `start <= ts < start + duration`.
struct ActiveEventTracker {
    /// `(start, end)` pairs, sorted by start.
    starts: Vec<(u64, u64)>,
    /// Min-heap of active end times, stored negated (`u64::MAX - end`).
    active_ends: BinaryHeap<u64>,
    /// Next unprocessed index into `starts`.
    next: usize,
}

impl ActiveEventTracker {
    fn new(events: impl IntoIterator<Item = (u64, u64)>) -> Self {
        let mut starts: Vec<(u64, u64)> = events.into_iter().collect();
        starts.sort_unstable_by_key(|(start, _)| *start);
        Self {
            starts,
            active_ends: BinaryHeap::new(),
            next: 0,
        }
    }

    /// Advance the sweep line to `ts` and report whether any event is active
    /// in the half-open interval `[ts, next_ts)` — i.e. `start <= ts < end`.
    fn advance(&mut self, ts: u64) -> bool {
        // Add events that start at or before this timestamp.
        while self.next < self.starts.len() && self.starts[self.next].0 <= ts {
            // Min-heap via negated end time.
            self.active_ends.push(u64::MAX - self.starts[self.next].1);
            self.next += 1;
        }
        // Drop events that have already ended (end <= ts).
        while let Some(&neg_end) = self.active_ends.peek() {
            if u64::MAX - neg_end <= ts {
                self.active_ends.pop();
            } else {
                break;
            }
        }
        !self.active_ends.is_empty()
    }
}

/// Render and quantize using the libass C library.
pub fn render_and_quantize(
    content: &str,
    _doc: &SubtitleDocument,
    config: &Config,
    args: &Args,
) -> Result<Vec<QuantizedFrame>, CliError> {
    let libass_config = build_libass_config(config);
    let frames = process_libass(content, libass_config, args)
        .map_err(|e| CliError::Conversion(format!("libass rendering failed: {e}")))?;
    Ok(frames)
}

/// Bridge config from the unified CLI format to libass-native format.
fn build_libass_config(config: &Config) -> subtitle_renderer_libass::ConversionConfig {
    use color_quantizer::DitherMethod;

    let dither = match config.dither {
        DitherMethod::None => "none".to_string(),
        DitherMethod::FloydSteinberg => "floyd-steinberg".to_string(),
        DitherMethod::Ordered => "ordered".to_string(),
    };

    subtitle_renderer_libass::ConversionConfig {
        fps: config.fps,
        width: config.resolution.width,
        height: config.resolution.height,
        max_colors: config.max_colors,
        dither,
        default_font: Some(config.font.default_font.clone()),
        fonts_dirs: config
            .font
            .font_dirs
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect(),
        font_fallback_map: HashMap::new(),
        check_fonts: false,
    }
}

/// Run the libass rendering pipeline.
///
/// This re-exports and calls the libass-core equivalent of the original
/// `Ass2Sup::process_events()` pipeline.
fn process_libass(
    content: &str,
    config: subtitle_renderer_libass::ConversionConfig,
    args: &Args,
) -> Result<Vec<QuantizedFrame>, subtitle_renderer_libass::AssError> {
    use subtitle_renderer_libass::AssRenderer;
    let t_start = std::time::Instant::now();

    let needed_families = subtitle_renderer_libass::extract_font_families(content);
    tracing::info!(
        "Font families needed: {}",
        if needed_families.is_empty() {
            "all".to_string()
        } else {
            needed_families
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        }
    );

    let mut renderer = AssRenderer::new(config.width, config.height)?;
    renderer.load_ass(content)?;
    renderer.configure_fonts(
        config.default_font.as_deref(),
        &config.fonts_dirs,
        &needed_families,
    )?;

    let events = renderer.events();
    if events.is_empty() {
        return Err(subtitle_renderer_libass::AssError::NoEvents);
    }

    let timestamps = subtitle_renderer_libass::generate_timestamps(&events, config.fps);
    if timestamps.is_empty() {
        return Err(subtitle_renderer_libass::AssError::NoEvents);
    }

    let pipeline =
        subtitle_renderer_libass::create_pipeline(config.max_colors, &config.dither, config.height);
    let mut output_frames: Vec<QuantizedFrame> = Vec::new();
    let mut prev_data_hash: Option<u64> = None;

    let total_frames = timestamps.len() as u64;
    tracing::info!(
        "Rendering {total_frames} frames ({} events, font setup {:.2}s)...",
        events.len(),
        t_start.elapsed().as_secs_f64(),
    );

    let mut progress =
        ProgressReporter::new(total_frames, "Rendering", args.quiet || args.parallel);

    let last_event_end = events
        .iter()
        .map(|e| e.start_ms + e.duration_ms)
        .max()
        .unwrap_or(0) as u64;

    // Sweep-line active-event tracker: sort events by start once, then walk
    // the timeline maintaining a min-heap of active end times. Replaces the
    // previous O(n×m) `events.iter().any(...)` scan per timestamp (n events ×
    // m timestamps ≈ 15.5M comparisons for a 2h movie).
    let mut tracker = ActiveEventTracker::new(
        events
            .iter()
            .map(|e| (e.start_ms as u64, (e.start_ms + e.duration_ms) as u64)),
    );
    let timestamps_iter = timestamps.windows(2);

    for window in timestamps_iter {
        let ts = window[0];
        let next_ts = window[1];

        let has_active = tracker.advance(ts);
        if !has_active {
            progress.inc();
            continue;
        }

        let images = match renderer.render_frame(ts as i64)? {
            Some(imgs) if !imgs.is_empty() => imgs,
            _ => {
                progress.inc();
                continue;
            }
        };

        let (rgba, bbox_x, bbox_y) = match subtitle_renderer_libass::compose_frame_bbox(
            &images,
            config.width,
            config.height,
        ) {
            Some(v) => v,
            None => {
                progress.inc();
                continue;
            }
        };

        // Crop to the tight non-transparent bbox. The compose step already
        // limited the buffer to the images' union bbox, so this scan is over a
        // small region (not the full 1920×1080 frame); the returned offsets
        // are relative to the bbox buffer and must be shifted back to
        // full-frame coordinates.
        let cropped =
            match subtitle_renderer_libass::crop_to_tight_bbox(&rgba.data, rgba.width, rgba.height)
            {
                Some((data, x, y, w, h)) => (data, x + bbox_x, y + bbox_y, w, h),
                None => {
                    progress.inc();
                    continue;
                }
            };

        let cropped_frame = subtitle_renderer_libass::CroppedFrame {
            data: cropped.0,
            x: cropped.1,
            y: cropped.2,
            width: cropped.3,
            height: cropped.4,
        };

        let prev_frame = output_frames.last();
        let mut q = pipeline.quantize_with_prev(
            &cropped_frame.data,
            cropped_frame.width,
            cropped_frame.height,
            prev_frame,
        );
        q.x = cropped_frame.x as u16;
        q.y = cropped_frame.y as u16;
        q.pts_ms = ts;
        q.duration_ms = next_ts.saturating_sub(ts).max(1);

        // Duplicate detection
        let hash = subtitle_renderer_libass::hash_quantized(&q);
        if prev_data_hash == Some(hash) {
            if let Some(last) = output_frames.last_mut() {
                last.duration_ms = ts + q.duration_ms - last.pts_ms;
            }
            progress.inc();
            continue;
        }

        prev_data_hash = Some(hash);
        output_frames.push(q);
        progress.inc();
    }

    progress.finish_and_clear();

    if output_frames.is_empty() {
        return Err(subtitle_renderer_libass::AssError::Ass(
            "libass rendered 0 frames — no fonts available or no visible events; \
             check that fonts are installed and the ASS file contains visible dialogue"
                .into(),
        ));
    }

    // Fix up last frame duration
    if let Some(last) = output_frames.last_mut()
        && last.pts_ms + last.duration_ms < last_event_end
    {
        last.duration_ms = last_event_end.saturating_sub(last.pts_ms);
    }

    let elapsed = t_start.elapsed();
    tracing::info!(
        "{}",
        crate::cli::progress::RenderSummary {
            elapsed_secs: elapsed.as_secs_f64(),
            rendered: 0, // libass pipeline dedups upstream; CLI sees only unique frames
            empty_skipped: None,
            duplicate_skipped: None,
            unique_frames: output_frames.len(),
        }
        .summary_line()
    );
    debug!(rendered = output_frames.len(), "libass rendering complete");
    Ok(output_frames)
}

#[cfg(test)]
mod tracker_tests {
    use super::ActiveEventTracker;

    /// Reference O(n×m) implementation of the original loop condition:
    /// event is active at `ts` iff `start <= ts && ts < start + duration`.
    fn reference_active(events: &[(u64, u64)], ts: u64) -> bool {
        events.iter().any(|(start, end)| *start <= ts && ts < *end)
    }

    #[test]
    fn matches_reference_scan_on_battleship_timeline() {
        // Property test across a realistic spread of timestamps: the sweep
        // tracker must agree with the original linear scan at every point.
        let events = [
            (0, 1000),
            (500, 600),
            (1000, 2000),
            (1000, 1000), // zero-duration: never active
            (2500, 3000),
            (9000, 10000),
        ];
        let mut tracker = ActiveEventTracker::new(events.iter().copied());
        for ts in (0..=10000).step_by(37) {
            assert_eq!(
                tracker.advance(ts),
                reference_active(&events, ts),
                "mismatch at ts={ts}"
            );
        }
    }

    #[test]
    fn event_starting_exactly_at_ts_is_active() {
        let events = [(100, 200)];
        let mut tracker = ActiveEventTracker::new(events.iter().copied());
        assert!(tracker.advance(100)); // start <= ts
        assert!(tracker.advance(199)); // still inside [start, end)
        assert!(!tracker.advance(200)); // end <= ts → not active
    }

    #[test]
    fn zero_duration_event_never_active() {
        let events = [(100, 100)];
        let mut tracker = ActiveEventTracker::new(events.iter().copied());
        assert!(!tracker.advance(100));
    }

    #[test]
    fn overlapping_events_share_activity() {
        let events = [(0, 100), (50, 150), (120, 130)];
        let mut tracker = ActiveEventTracker::new(events.iter().copied());
        assert!(tracker.advance(0));
        assert!(tracker.advance(60)); // inside first two
        assert!(tracker.advance(125)); // only the middle one
        assert!(!tracker.advance(200)); // all ended
    }

    #[test]
    fn no_events_never_active() {
        let mut tracker = ActiveEventTracker::new(std::iter::empty());
        assert!(!tracker.advance(0));
        assert!(!tracker.advance(1_000_000));
    }

    #[test]
    fn unsorted_input_is_handled() {
        let events = [(500, 600), (0, 100), (200, 300)];
        let mut tracker = ActiveEventTracker::new(events.iter().copied());
        assert!(tracker.advance(0));
        assert!(tracker.advance(250));
        assert!(tracker.advance(550));
        assert!(!tracker.advance(1000));
    }
}
