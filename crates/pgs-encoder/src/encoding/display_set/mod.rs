//! PGS display-set builders.
//!
//! Each module builds one family of display sets (PCS/WDS/PDS/ODS segment
//! groups): basic kinds, windowed single/multi-object sets, and epoch-split
//! sets for PotPlayer compatibility.

mod basic;
mod epoch_split;
mod window;

pub use basic::{
    build_continue_display_set, build_palette_clear_display_set, build_palette_only_display_set,
};
pub use epoch_split::build_epoch_split_display_set;
pub use window::{build_multi_window_display_set, build_single_window_display_set, find_split_row};

use crate::domain::palette::PaletteEntry;
use crate::domain::rle::rle_encode;

const MAX_ODS_CHUNK: usize = 0xFFE0;

/// Configuration for building a single PGS display set (PCS/WDS/PDS/ODS).
pub struct DisplaySetConfig {
    pub display_width: u16,
    pub display_height: u16,
    pub frame_rate: u8,
    pub composition_number: u16,
    pub object_id: u16,
    pub palette_id: u8,
    pub window_id: u8,
    pub potplayer_compat: bool,
}

impl DisplaySetConfig {
    /// Number of objects to reference in a palette_clear display set.
    pub fn palette_clear_num_objects(&self) -> u8 {
        1
    }
}

/// Prepare RLE data and compute its hash for ODS encoding.
pub fn prepare_rle_and_hash(
    palette_entries: &mut [PaletteEntry],
    indices: &[u8],
    width: u32,
    height: u32,
    transparent_index: u8,
) -> (Vec<u8>, u64) {
    let ti = transparent_index;
    if ti != 0 && (ti as usize) < palette_entries.len() {
        palette_entries.swap(0, ti as usize);
        let mut swapped_indices = indices.to_vec();
        for idx in swapped_indices.iter_mut() {
            if *idx == 0 {
                *idx = ti;
            } else if *idx == ti {
                *idx = 0;
            }
        }
        let rle = rle_encode(&swapped_indices, width, height, 0);
        let rle_hash = crate::domain::epoch::hash_bytes(&rle);
        (rle, rle_hash)
    } else {
        let rle = rle_encode(indices, width, height, transparent_index);
        let rle_hash = crate::domain::epoch::hash_bytes(&rle);
        (rle, rle_hash)
    }
}
