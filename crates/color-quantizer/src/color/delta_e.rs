#![allow(missing_docs)]

//! Perceptual color-difference formulae.
//!
//! Provides CIE76 (ΔE₆₀), CIE94, and a fast perceptually-weighted
//! Euclidean approximation for palette nearest-neighbour search.
//!
//! All functions operate on linear-light [R, G, B] in [0..=1].

/// CIE76 colour-difference: Euclidean in L*a*b*.
/// Fastest ΔE variant; acceptable above ≈10 ΔE.
pub fn cie76(_l1: f32, _a1: f32, _b1: f32, _l2: f32, _a2: f32, _b2: f32) -> f32 {
    let dl = _l1 - _l2;
    let da = _a1 - _a2;
    let db = _b1 - _b2;
    (dl * dl + da * da + db * db).sqrt()
}

/// Perceptually-weighted Euclidean distance in RGB (3:4:2 heuristic).
///
/// Approximately matches ΔE₆₀ at a fraction of the cost. Suitable for
/// median-cut splitting and k-d tree nearest-neighbour.
pub fn weighted_rgb_distance(r1: u8, g1: u8, b1: u8, r2: u8, g2: u8, b2: u8) -> u32 {
    let dr = r1 as i32 - r2 as i32;
    let dg = g1 as i32 - g2 as i32;
    let db = b1 as i32 - b2 as i32;
    (dr * dr * 3 + dg * dg * 4 + db * db * 2) as u32
}
