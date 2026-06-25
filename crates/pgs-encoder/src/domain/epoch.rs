use crate::types::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Display set kind for epoch management.
///
/// Determines which segments are included in a display set and which
/// composition state is used, per PGS spec (ISO 14496-6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplaySetKind {
    /// First frame of a new epoch — full display set with all segments.
    EpochStart,
    /// Object content changed — full display set with updated ODS.
    NormalCase,
    /// No object or palette changes — minimal PCS + END to keep showing.
    EpochContinue,
    /// Only palette changed — PCS + PDS + END, no ODS/WDS.
    PaletteOnly,
}

/// Tracks epoch state across consecutive frames.
///
/// Maintains palette and RLE hashes from the previous frame and
/// determines the [`DisplaySetKind`] for each new frame.
pub struct EpochManager {
    pub prev_palette_hash: Option<u64>,
    pub prev_object_rle_hash: Option<u64>,
    pub frame_count: u32,
    pub object_version: u8,
    pub max_frames_per_epoch: u32,
}

impl EpochManager {
    pub fn new() -> Self {
        Self {
            prev_palette_hash: None,
            prev_object_rle_hash: None,
            frame_count: 0,
            object_version: 0,
            max_frames_per_epoch: 0,
        }
    }

    /// Set the maximum frames per epoch before forcing a restart.
    /// `0` (default) disables the limit.
    pub fn with_max_frames(mut self, max: u32) -> Self {
        self.max_frames_per_epoch = max;
        self
    }

    /// Decide the display set kind based on hash comparisons and epoch duration.
    ///
    /// Returns [`DisplaySetKind::EpochStart`] when there is no previous frame
    /// (first frame or explicit reset) or when the max-frames-per-epoch limit
    /// has been reached.
    pub fn decide_kind(&self, palette_hash: u64, rle_hash: u64) -> DisplaySetKind {
        if self.max_frames_per_epoch > 0 && self.frame_count >= self.max_frames_per_epoch {
            return DisplaySetKind::EpochStart;
        }
        if self.prev_object_rle_hash.is_none() {
            DisplaySetKind::EpochStart
        } else if rle_hash
            != self
                .prev_object_rle_hash
                .unwrap_or(rle_hash.wrapping_add(1))
        {
            DisplaySetKind::NormalCase
        } else if palette_hash
            != self
                .prev_palette_hash
                .unwrap_or(palette_hash.wrapping_add(1))
        {
            DisplaySetKind::PaletteOnly
        } else {
            DisplaySetKind::EpochContinue
        }
    }

    /// Store hashes and advance counters after a frame is processed.
    ///
    /// When the max-frames-per-epoch limit is reached, resets epoch state
    /// so the next call to [`decide_kind`] returns [`DisplaySetKind::EpochStart`].
    pub fn update(&mut self, palette_hash: u64, rle_hash: u64) {
        self.prev_palette_hash = Some(palette_hash);
        self.prev_object_rle_hash = Some(rle_hash);
        self.frame_count += 1;
        self.object_version = self.object_version.wrapping_add(1);
        if self.max_frames_per_epoch > 0 && self.frame_count >= self.max_frames_per_epoch {
            self.frame_count = 0;
            self.prev_palette_hash = None;
            self.prev_object_rle_hash = None;
            self.object_version = 0;
        }
    }
}

impl Default for EpochManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute a hash of palette entries for palette-change detection.
pub fn hash_palette(entries: &[PaletteEntry]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for entry in entries {
        entry.index.hash(&mut hasher);
        entry.y.hash(&mut hasher);
        entry.cb.hash(&mut hasher);
        entry.cr.hash(&mut hasher);
        entry.alpha.hash(&mut hasher);
    }
    hasher.finish()
}

/// Compute a hash of RLE data for object change detection.
pub fn hash_bytes(data: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    data.hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_start_on_first_frame() {
        let mgr = EpochManager::new();
        assert_eq!(mgr.decide_kind(0, 0), DisplaySetKind::EpochStart);
    }

    #[test]
    fn test_epoch_continue_unchanged() {
        let mut mgr = EpochManager::new();
        mgr.update(100, 200);
        assert_eq!(mgr.decide_kind(100, 200), DisplaySetKind::EpochContinue);
    }

    #[test]
    fn test_normal_case_rle_changed() {
        let mut mgr = EpochManager::new();
        mgr.update(100, 200);
        assert_eq!(mgr.decide_kind(100, 201), DisplaySetKind::NormalCase);
    }

    #[test]
    fn test_palette_only_palette_changed() {
        let mut mgr = EpochManager::new();
        mgr.update(100, 200);
        assert_eq!(mgr.decide_kind(101, 200), DisplaySetKind::PaletteOnly);
    }

    #[test]
    fn test_max_frames_forces_epoch_start_with_unchanged_hashes() {
        let mut mgr = EpochManager::new().with_max_frames(3);
        // First frame always epoch start
        assert_eq!(mgr.decide_kind(100, 200), DisplaySetKind::EpochStart);
        mgr.update(100, 200);
        // Second frame — unchanged hashes → EpochContinue
        assert_eq!(mgr.decide_kind(100, 200), DisplaySetKind::EpochContinue);
        mgr.update(100, 200);
        // Third frame — unchanged, but frame_count=2 < 3 → still EpochContinue
        assert_eq!(mgr.decide_kind(100, 200), DisplaySetKind::EpochContinue);
        mgr.update(100, 200);
        // Fourth call: frame_count=3 >= 3 → EpochStart despite unchanged hashes
        assert_eq!(mgr.decide_kind(100, 200), DisplaySetKind::EpochStart);
    }

    #[test]
    fn test_max_frames_disabled_zero_still_continues() {
        let mut mgr = EpochManager::new().with_max_frames(0);
        mgr.update(100, 200);
        // Run past 100 frames — still EpochContinue
        for _ in 0..100 {
            mgr.update(100, 200);
        }
        assert_eq!(mgr.decide_kind(100, 200), DisplaySetKind::EpochContinue);
    }

    #[test]
    fn test_max_frames_update_resets_counters() {
        let mut mgr = EpochManager::new().with_max_frames(2);
        mgr.update(100, 200); // frame_count → 1
        mgr.update(100, 200); // frame_count → 2, then reset to 0
                              // After reset, hashes should be None → EpochStart
        assert_eq!(mgr.frame_count, 0);
        assert!(mgr.prev_palette_hash.is_none());
        assert!(mgr.prev_object_rle_hash.is_none());
        assert_eq!(mgr.object_version, 0);
    }
}
