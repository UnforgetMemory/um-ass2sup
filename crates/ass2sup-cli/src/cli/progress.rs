//! Progress bar creation and styling.
//!
//! The progress bar is drawn to **stderr** (not stdout) so it never contends
//! with the tracing stdout layer, which on Windows PowerShell caused the bar
//! to be swallowed entirely (the render loop then looked "frozen").

use indicatif::{ProgressBar, ProgressDrawTarget, ProgressStyle};

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
