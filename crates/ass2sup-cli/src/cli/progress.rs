//! Progress bar creation and styling.
//!
//! The progress bar is drawn to **stderr** (not stdout) so it never contends
//! with the tracing stdout layer, which on Windows PowerShell caused the bar
//! to be swallowed entirely (the render loop then looked "frozen").

use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use std::cell::RefCell;
use std::time::Duration;

thread_local! {
    static BATCH_MULTI: RefCell<Option<MultiProgress>> = const { RefCell::new(None) };
}

/// Attach reporter bars on this thread to a shared [`MultiProgress`]
/// (called by sequential batch mode before the loop).
pub fn set_batch_multi(mp: MultiProgress) {
    BATCH_MULTI.with(|cell| *cell.borrow_mut() = Some(mp));
}

/// Detach reporter bars from the shared [`MultiProgress`] after the loop.
pub fn clear_batch_multi() {
    BATCH_MULTI.with(|cell| *cell.borrow_mut() = None);
}

/// Exponential moving average of render throughput (frames/second).
///
/// ETA derived from a *cumulative* average (`elapsed / processed`) jitters
/// wildly on subtitle workloads because per-unit cost is heterogeneous:
/// cheap "no active event" skips, expensive animation-burst frames and
/// duplicate/empty skips all share the same counter. A windowed/EMA rate
/// smooths those swings into a stable, professional-looking ETA.
struct SmoothRate {
    ema: Option<f64>,
    alpha: f64,
}

impl SmoothRate {
    fn new() -> Self {
        Self {
            ema: None,
            alpha: 0.3,
        }
    }

    /// Feed one instantaneous rate sample (fps); returns the smoothed rate.
    /// The first sample seeds the EMA directly.
    fn sample(&mut self, inst: f64) -> f64 {
        let ema = match self.ema {
            None => inst,
            Some(prev) => self.alpha * inst + (1.0 - self.alpha) * prev,
        };
        self.ema = Some(ema);
        ema
    }
}

/// ETA in seconds given a smoothed rate (fps) and remaining work.
fn eta_from_rate(rate: f64, remaining: u64) -> f64 {
    if rate > 0.0 {
        remaining as f64 / rate
    } else {
        0.0
    }
}

/// A progress reporter that drives both an optional indicatif bar (when the
/// terminal supports it) and a throttled plain-text log.
///
/// The plain-text log is the **reliable cross-platform feedback channel**: on
/// Windows PowerShell the indicatif bar can be swallowed, so we also emit
/// `Rendered X/Y (P%) elapsed Es` at most every [`Self::LOG_INTERVAL`] or every
/// [`Self::FRAMES_PER_LOG`] frames. This guarantees the user always sees
/// progress even when the bar is invisible. The plain-text lines are logged at
/// `debug!` level so interactive terminals default to the smooth bar as the
/// primary display and only see the lines with `-v`.
pub struct ProgressReporter {
    bar: ProgressBar,
    total: u64,
    processed: u64,
    started: std::time::Instant,
    last_log: std::time::Instant,
    last_log_frames: u64,
    quiet: bool,
    rate: SmoothRate,
    message: String,
}

impl ProgressReporter {
    /// Throttle plain-text logs to at most one per 3 seconds.
    const LOG_INTERVAL: std::time::Duration = std::time::Duration::from_secs(3);
    /// ... or at most one per 500 frames.
    const FRAMES_PER_LOG: u64 = 500;

    /// Create a reporter for `total` units of work.
    pub fn new(total: u64, message: &str, quiet: bool) -> Self {
        let bar = if quiet {
            indicatif::ProgressBar::hidden()
        } else {
            create(total, message)
        };
        Self {
            bar,
            total,
            processed: 0,
            started: std::time::Instant::now(),
            last_log: std::time::Instant::now(),
            last_log_frames: 0,
            quiet,
            rate: SmoothRate::new(),
            message: message.to_string(),
        }
    }

    /// Advance by one unit, emitting a throttled plain-text log when due.
    pub fn inc(&mut self) {
        self.processed += 1;
        self.bar.inc(1);
        let frames_since = self.processed.saturating_sub(self.last_log_frames);
        let time_since = self.last_log.elapsed();
        if !self.quiet
            && (frames_since >= Self::FRAMES_PER_LOG
                || (self.processed > 0 && time_since >= Self::LOG_INTERVAL))
        {
            let elapsed = self.started.elapsed().as_secs_f64();
            let pct = self.processed as f64 / self.total.max(1) as f64 * 100.0;
            let inst_rate = if time_since.as_secs_f64() > 0.0 {
                frames_since as f64 / time_since.as_secs_f64()
            } else {
                0.0
            };
            let rate = self.rate.sample(inst_rate);
            let remaining = self.total.saturating_sub(self.processed);
            let mut eta = eta_from_rate(rate, remaining);
            // Fallback: never report 0 ETA while work remains and we have a
            // cumulative estimate to lean on.
            if eta <= 0.0 && self.processed > 0 && elapsed > 0.0 {
                eta = elapsed / self.processed as f64 * remaining as f64;
            }
            self.bar
                .set_message(format!("{} · {rate:.0} fps · ETA {eta:.0}s", self.message));
            tracing::debug!(
                "Rendered {}/{} ({:.1}%) elapsed {:.0}s, ETA ~{:.0}s, {:.0} fps",
                self.processed,
                self.total,
                pct,
                elapsed,
                eta,
                rate,
            );
            self.last_log = std::time::Instant::now();
            self.last_log_frames = self.processed;
        }
    }

    /// Mark the progress as finished and clear the bar.
    pub fn finish_and_clear(&mut self) {
        self.bar.finish_and_clear();
    }

    /// Current processed count (useful for final stats).
    pub fn processed(&self) -> u64 {
        self.processed
    }
}

/// Aggregate render statistics reported by each backend after the render loop.
///
/// Both backends produce a single, identically-structured completion line;
/// fields that a backend cannot measure are omitted from the formatted output
/// rather than shown as zero.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderSummary {
    /// Wall time spent in the render+quantize loop.
    pub elapsed_secs: f64,
    /// Number of frames actually rasterized (0 if the backend does not track it).
    pub rendered: u64,
    /// Frames skipped because the rendered bitmap was fully transparent.
    pub empty_skipped: Option<u64>,
    /// Frames skipped because they were byte-identical to the previous frame.
    pub duplicate_skipped: Option<u64>,
    /// Unique frames kept for encoding.
    pub unique_frames: usize,
}

impl RenderSummary {
    /// One-line professional completion summary, shared by both backends.
    pub fn summary_line(&self) -> String {
        let mut s = format!(
            "Render complete: {} unique frames in {:.2}s",
            self.unique_frames, self.elapsed_secs
        );
        if self.rendered > 0 {
            s.push_str(&format!(
                " (avg {:.1} ms/frame, rendered {})",
                self.elapsed_secs * 1000.0 / self.rendered as f64,
                self.rendered
            ));
        }
        let mut skipped: Vec<String> = Vec::new();
        if let Some(e) = self.empty_skipped {
            skipped.push(format!("empty {e}"));
        }
        if let Some(d) = self.duplicate_skipped {
            skipped.push(format!("duplicate {d}"));
        }
        if !skipped.is_empty() {
            s.push_str(&format!(" · {}", skipped.join(", ")));
        }
        s
    }
}

/// Create a styled progress bar with the cyan/blue theme, drawn to stderr.
///
/// Refresh is throttled to 10 Hz (every 100 ms) so per-frame `inc()` calls in
/// the render loop don't redraw the whole line thousands of times a second.
pub fn create(len: u64, message: &str) -> ProgressBar {
    // In sequential batch mode the bar joins the shared MultiProgress so it
    // coexists with the per-file bars on separate lines.
    let pb = BATCH_MULTI.with(|cell| match cell.borrow().as_ref() {
        Some(mp) => mp.add(ProgressBar::new(len)),
        None => ProgressBar::with_draw_target(Some(len), ProgressDrawTarget::stderr()),
    });
    let style = ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}")
        .unwrap_or_else(|e| {
            // Static template: failure is practically impossible, but degrade
            // gracefully instead of panicking on a format-string typo.
            tracing::warn!("Failed to build progress-bar template: {e}");
            ProgressStyle::default_bar()
        });
    pb.set_style(style.progress_chars("█▓░"));
    pb.set_message(message.to_string());
    pb.enable_steady_tick(Duration::from_millis(100));
    pb
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reporter_tracks_processed_count() {
        let mut r = ProgressReporter::new(1000, "Rendering", true);
        for _ in 0..250 {
            r.inc();
        }
        assert_eq!(r.processed(), 250);
    }

    #[test]
    fn reporter_quiet_suppresses_log_but_counts() {
        // quiet mode must never emit logs, but still count units.
        let mut r = ProgressReporter::new(100, "Rendering", true);
        for _ in 0..100 {
            r.inc();
        }
        assert_eq!(r.processed(), 100);
    }

    #[test]
    fn reporter_full_run_counts_and_clears() {
        // Deterministic: FRAMES_PER_LOG throttle means 500+ increments emit.
        // We can't easily capture tracing output here, but we can assert the
        // reporter never panics and counts stay sane across a full run.
        let mut r = ProgressReporter::new(2000, "Rendering", false);
        for _ in 0..2000 {
            r.inc();
        }
        r.finish_and_clear();
        assert_eq!(r.processed(), 2000);
    }

    #[test]
    fn smooth_rate_seeds_with_first_sample() {
        let mut r = SmoothRate::new();
        assert_eq!(r.sample(10.0), 10.0);
    }

    #[test]
    fn smooth_rate_blends_subsequent_samples() {
        let mut r = SmoothRate::new();
        r.sample(10.0);
        // alpha = 0.3: 0.3*20 + 0.7*10 = 13.0
        assert!((r.sample(20.0) - 13.0).abs() < 1e-9);
    }

    #[test]
    fn smooth_rate_converges_toward_steady_state() {
        let mut r = SmoothRate::new();
        r.sample(100.0);
        let mut last = 100.0;
        for _ in 0..80 {
            last = r.sample(30.0);
        }
        // Converges asymptotically toward 30 fps (steady-state input);
        // after 80 steps the residual is 70·0.7^80 ≈ 4e-11.
        assert!(
            (last - 30.0).abs() < 1e-6,
            "rate {last} did not converge to 30"
        );
    }

    #[test]
    fn smooth_rate_recovers_from_extreme_outlier() {
        // The whole point: one 500-fps burst (cheap skip region) must not
        // dominate the estimate for long; EMA damps the outlier.
        let mut r = SmoothRate::new();
        r.sample(500.0);
        let first = r.sample(10.0); // realistic render rate
        // First blend: 0.3·10 + 0.7·500 = 353 — damped, not equal to the spike.
        assert!(
            first < 400.0 && first > 300.0,
            "outlier not damped: {first}"
        );
        let mut last = first;
        for _ in 0..60 {
            last = r.sample(10.0);
        }
        assert!((last - 10.0).abs() < 1e-6);
    }

    #[test]
    fn eta_decreases_with_constant_rate() {
        // At a fixed 50 fps, ETA must be exactly remaining/50 for any total.
        let mut r = SmoothRate::new();
        r.sample(50.0);
        assert_eq!(eta_from_rate(50.0, 1000), 20.0);
        assert_eq!(eta_from_rate(50.0, 500), 10.0);
        assert_eq!(eta_from_rate(50.0, 0), 0.0);
        assert_eq!(eta_from_rate(0.0, 1000), 0.0);
    }

    #[test]
    fn eta_stable_across_mixed_samples() {
        // Alternating cheap (1000 fps) and expensive (10 fps) regions: the
        // cumulative average would swing between 10s and 1000s of ETA; the EMA
        // stays within the physically-meaningful band.
        let mut r = SmoothRate::new();
        r.sample(10.0);
        let mut rate = 10.0;
        for i in 0..100 {
            let inst = if i % 2 == 0 { 1000.0 } else { 10.0 };
            rate = r.sample(inst);
        }
        assert!(rate > 5.0 && rate < 500.0, "EMA rate out of band: {rate}");
    }

    #[test]
    fn render_summary_full_native_shape() {
        let s = RenderSummary {
            elapsed_secs: 287.16,
            rendered: 5974,
            empty_skipped: Some(35),
            duplicate_skipped: Some(2659),
            unique_frames: 3280,
        };
        let line = s.summary_line();
        assert!(line.starts_with("Render complete: 3280 unique frames in 287.16s"));
        assert!(line.contains("avg 48.1 ms/frame, rendered 5974"));
        assert!(line.contains("empty 35"));
        assert!(line.contains("duplicate 2659"));
    }

    #[test]
    fn render_summary_omits_unknown_fields() {
        // libass backend cannot measure rendered/empty/dup at the CLI level.
        let s = RenderSummary {
            elapsed_secs: 165.8,
            rendered: 0,
            empty_skipped: None,
            duplicate_skipped: None,
            unique_frames: 3193,
        };
        let line = s.summary_line();
        assert_eq!(line, "Render complete: 3193 unique frames in 165.80s");
        assert!(!line.contains("avg"));
        assert!(!line.contains("duplicate"));
    }

    #[test]
    fn render_summary_zero_elapsed_no_panic() {
        let s = RenderSummary::default();
        assert_eq!(
            s.summary_line(),
            "Render complete: 0 unique frames in 0.00s"
        );
    }
}
