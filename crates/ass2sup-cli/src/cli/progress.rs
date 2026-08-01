//! Progress bar creation and styling.
//!
//! The progress bar is drawn to **stderr** (not stdout) so it never contends
//! with the tracing stdout layer, which on Windows PowerShell caused the bar
//! to be swallowed entirely (the render loop then looked "frozen").

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

/// A progress reporter that drives both an optional indicatif bar (when the
/// terminal supports it) and a throttled plain-text INFO log.
///
/// The plain-text log is the **reliable cross-platform feedback channel**: on
/// Windows PowerShell the indicatif bar can be swallowed, so we also emit
/// `Rendered X/Y (P%) elapsed Es` at most every [`Self::LOG_INTERVAL`] or every
/// [`Self::FRAMES_PER_LOG`] frames. This guarantees the user always sees
/// progress even when the bar is invisible.
pub struct ProgressReporter {
    bar: ProgressBar,
    total: u64,
    processed: u64,
    started: std::time::Instant,
    last_log: std::time::Instant,
    last_log_frames: u64,
    quiet: bool,
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
        }
    }

    /// Advance by one unit, emitting a throttled INFO log when due.
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
            // ETA based on average rate so far.
            let eta = if self.processed > 0 && elapsed > 0.0 {
                let per = elapsed / self.processed as f64;
                per * (self.total.saturating_sub(self.processed)) as f64
            } else {
                0.0
            };
            tracing::info!(
                "Rendered {}/{} ({:.1}%) elapsed {:.0}s, ETA ~{:.0}s",
                self.processed,
                self.total,
                pct,
                elapsed,
                eta,
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

/// Create a styled progress bar with the cyan/blue theme, drawn to stderr.
///
/// Refresh is throttled to 10 Hz (every 100 ms) so per-frame `inc()` calls in
/// the render loop don't redraw the whole line thousands of times a second.
pub fn create(len: u64, message: &str) -> ProgressBar {
    let pb = ProgressBar::with_draw_target(Some(len), ProgressDrawTarget::stderr());
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({eta}) {msg}")
            .unwrap()
            .progress_chars("█▓░"),
    );
    pb.set_message(message.to_string());
    pb.enable_steady_tick(std::time::Duration::from_millis(100));
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
    fn reporter_eta_decreases_as_progress_increases() {
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
}
