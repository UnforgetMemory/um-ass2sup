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
}

impl EpochManager {
    pub fn new() -> Self {
        Self {
            prev_palette_hash: None,
            prev_object_rle_hash: None,
            frame_count: 0,
            object_version: 0,
        }
    }

    /// Decide the display set kind based on hash comparisons.
    ///
    /// Returns [`DisplaySetKind::EpochStart`] when there is no previous frame
    /// (first frame or explicit reset).
    pub fn decide_kind(&self, palette_hash: u64, rle_hash: u64) -> DisplaySetKind {
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
    pub fn update(&mut self, palette_hash: u64, rle_hash: u64) {
        self.prev_palette_hash = Some(palette_hash);
        self.prev_object_rle_hash = Some(rle_hash);
        self.frame_count += 1;
        self.object_version = self.object_version.wrapping_add(1);
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
}
