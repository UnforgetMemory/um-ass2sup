#![allow(missing_docs)]

use crate::Rgba;

/// Convert a flat `[[u8; 4]]` palette to `Vec<Rgba>` for the unified output type.
pub fn palette_to_rgba(palette: &[[u8; 4]]) -> Vec<Rgba> {
    palette
        .iter()
        .map(|c| Rgba::new(c[0], c[1], c[2], c[3]))
        .collect()
}

/// Convert `Vec<Rgba>` to a flat `[[u8; 4]]` palette for internal processing.
pub fn rgba_to_palette(palette: &[Rgba]) -> Vec<[u8; 4]> {
    palette.iter().map(|c| [c.r, c.g, c.b, c.a]).collect()
}
