#![allow(missing_docs)]

use crate::Rgba;

/// Convert `Vec<Rgba>` to a flat `[[u8; 4]]` palette for internal processing.
pub fn rgba_to_palette(palette: &[Rgba]) -> Vec<[u8; 4]> {
    palette.iter().map(|c| [c.r, c.g, c.b, c.a]).collect()
}
