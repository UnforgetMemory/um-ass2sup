//! Colour-space conversion functions for PGS encoding.
//!
//! Re-exports from `domain::palette` for backward compatibility.
//! New code should import directly from `crate::domain::palette`.

pub use crate::domain::palette::{
    build_palette, color_space_for_height, palette_to_rgba, rgba_to_ycbcr, swap, ycbcr_to_rgba,
};
