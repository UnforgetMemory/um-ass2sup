#![allow(missing_docs)]

//! Frame-to-frame temporal palette optimisation.
//!
//! Analyses consecutive frames to decide whether to reuse the previous
//! palette (saving PGS bandwidth), build a delta palette, or trigger a
//! full requantisation.

use crate::quantize::nearest::find_nearest_weighted;

/// Decision from temporal palette analysis.
#[derive(Copy, Clone, Debug, PartialEq)]
pub enum PaletteDecision {
    /// Reuse previous palette unchanged.
    Reuse,
    /// Build a delta palette — partial update.
    Delta,
    /// Full requantisation needed — frame content changed significantly.
    Refresh,
}

/// Frame-to-frame palette reuse analyser.
pub struct TemporalAnalyzer {
    /// Palette from the previous frame.
    prev_palette: Option<Vec<[u8; 4]>>,
}

impl TemporalAnalyzer {
    pub fn new() -> Self {
        Self { prev_palette: None }
    }

    /// Analyse the current frame and decide on palette strategy.
    ///
    /// `current_pixels` is the set of unique colours in the current frame
    /// (deduplicated). `palette` is the colour table produced for the frame.
    /// `threshold` is the maximum weighted colour distance for a pixel to
    /// be considered "mappable" to the previous palette.
    pub fn analyze(
        &mut self,
        current_pixels: &[[u8; 4]],
        palette: &[[u8; 4]],
        threshold: f32,
    ) -> PaletteDecision {
        let prev = match &self.prev_palette {
            Some(p) => p,
            None => {
                self.prev_palette = Some(palette.to_vec());
                return PaletteDecision::Refresh;
            }
        };

        let threshold_sq = (threshold * threshold) as u64;

        // Count pixels that CAN be mapped to the previous palette.
        let mappable = current_pixels
            .iter()
            .filter(|p| {
                let idx = find_nearest_weighted(p, prev) as usize;
                if idx < prev.len() {
                    let dr = i64::from(p[0]) - i64::from(prev[idx][0]);
                    let dg = i64::from(p[1]) - i64::from(prev[idx][1]);
                    let db = i64::from(p[2]) - i64::from(prev[idx][2]);
                    let da = i64::from(p[3]) - i64::from(prev[idx][3]);
                    let d_sq = (dr * dr * 3 + dg * dg * 4 + db * db * 2 + da * da) as u64;
                    d_sq <= threshold_sq
                } else {
                    false
                }
            })
            .count();

        let total = current_pixels.len();
        let mappable_ratio = if total > 0 {
            mappable as f32 / total as f32
        } else {
            1.0
        };

        self.prev_palette = Some(palette.to_vec());

        if mappable_ratio >= 0.95 {
            PaletteDecision::Reuse
        } else if mappable_ratio >= 0.70 {
            PaletteDecision::Delta
        } else {
            PaletteDecision::Refresh
        }
    }

    /// Reset the analyser state (e.g. on a scene cut).
    pub fn reset(&mut self) {
        self.prev_palette = None;
    }
}

/// Check if all pixels in `pixels` can map to `palette` within `threshold`.
pub fn all_mappable(pixels: &[[u8; 4]], palette: &[[u8; 4]], threshold: f32) -> bool {
    if palette.is_empty() {
        return false;
    }
    let threshold_sq = (threshold * threshold) as u64;
    pixels.iter().all(|p| {
        let idx = find_nearest_weighted(p, palette) as usize;
        if idx < palette.len() {
            let dr = i64::from(p[0]) - i64::from(palette[idx][0]);
            let dg = i64::from(p[1]) - i64::from(palette[idx][1]);
            let db = i64::from(p[2]) - i64::from(palette[idx][2]);
            let da = i64::from(p[3]) - i64::from(palette[idx][3]);
            let d_sq = (dr * dr * 3 + dg * dg * 4 + db * db * 2 + da * da) as u64;
            d_sq <= threshold_sq
        } else {
            false
        }
    })
}

impl Default for TemporalAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_frame_is_refresh() {
        let mut ta = TemporalAnalyzer::new();
        let decision = ta.analyze(&[[255, 0, 0, 255]], &[[255, 0, 0, 255]], 30.0);
        assert_eq!(decision, PaletteDecision::Refresh);
    }

    #[test]
    fn identical_frame_is_reuse() {
        let mut ta = TemporalAnalyzer::new();
        let pixels = [[255, 0, 0, 255], [0, 255, 0, 255]];
        let palette = vec![[255, 0, 0, 255], [0, 255, 0, 255]];
        ta.analyze(&pixels, &palette, 30.0); // first → refresh
        let decision = ta.analyze(&pixels, &palette, 30.0); // second
        assert_eq!(decision, PaletteDecision::Reuse);
    }

    #[test]
    fn reset_clears_state() {
        let mut ta = TemporalAnalyzer::new();
        ta.analyze(&[[0, 0, 0, 255]], &[[0, 0, 0, 255]], 30.0);
        ta.reset();
        let decision = ta.analyze(&[[0, 0, 0, 255]], &[[0, 0, 0, 255]], 30.0);
        assert_eq!(decision, PaletteDecision::Refresh);
    }

    #[test]
    fn different_frame_is_refresh() {
        let mut ta = TemporalAnalyzer::new();
        ta.analyze(&[[0, 0, 0, 255]], &[[0, 0, 0, 255]], 30.0);
        let decision = ta.analyze(&[[255, 255, 255, 255]], &[[255, 255, 255, 255]], 30.0);
        assert_eq!(decision, PaletteDecision::Refresh);
    }

    #[test]
    fn all_mappable_identical() {
        let pixels = [[100, 100, 100, 255]];
        let palette = vec![[100, 100, 100, 255]];
        assert!(all_mappable(&pixels, &palette, 10.0));
    }

    #[test]
    fn all_mappable_empty_palette() {
        assert!(!all_mappable(&[[0, 0, 0, 255]], &[], 10.0));
    }
}
