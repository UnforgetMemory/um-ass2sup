#![allow(missing_docs)]

//! Tone-mapping operators for HDR → SDR conversion.
//!
//! When the source content uses HDR primaries (BT.2020) or transfer functions
//! (PQ, HLG) but the output is SDR (BT.709 / sRGB), the colour values must be
//! tone-mapped to fit within the SDR luminance range (~0–100 cd/m²).
//!
//! | Operator | Character        | Best for           |
//! |----------|------------------|--------------------|
//! | Hable    | Preserves detail | Gaming / CGI       |
//! | Reinhard | Simple, fast     | General-purpose    |
//! | ACES     | Film-grade       | Cinematic content  |
//!
//! All operators work on individual `f32` channels in **linear-light** space.
//! Callers are responsible for converting to/from linear-light via the
//! transfer functions in [`super::transfer`].

/// Apply the Hable (Uncharted 2) tone-mapping curve to a linear channel.
///
/// Preserves detail in both shadows and highlights. The default exposure bias
/// is 1.0 (no adjustment).
#[inline]
pub fn hable(x: f32, exposure_bias: f32) -> f32 {
    let x = x * exposure_bias;
    // Uncharted 2 filmic curve constants
    let a = 0.15;
    let b = 0.50;
    let c = 0.10;
    let d = 0.20;
    let e = 0.02;
    let f = 0.30;
    ((x * (a * x + c * b) + d * e) / (x * (a * x + b) + d * f)) - e / f
}

/// Apply the Reinhard global tone-mapping operator to a linear channel.
///
/// Uses the basic `x / (1 + x)` formula which always maps [0, ∞) to [0, 1).
/// This preserves highlight detail but does not include a white-point
/// adjustment (use [`hable`] for that).
#[inline]
pub fn reinhard(x: f32) -> f32 {
    x / (1.0 + x)
}

/// Apply the ACES filmic tone-mapping (approximate curve).
///
/// A simplified version of the ACES RRT+ODT transform suitable for real-time
/// use.
#[inline]
pub fn aces_approx(x: f32) -> f32 {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    (x * (a * x + b)) / (x * (c * x + d) + e)
}

/// Tone-mapping operators that can be selected at runtime.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub enum ToneMapOperator {
    /// Hable (Uncharted 2) filmic curve
    #[default]
    Hable,
    /// Reinhard global
    Reinhard,
    /// ACES filmic approximation
    Aces,
}

/// Apply the selected tone-mapping operator to a single linear-light channel.
#[inline]
pub fn tone_map(channel: f32, operator: ToneMapOperator) -> f32 {
    match operator {
        ToneMapOperator::Hable => {
            let mapped = hable(channel, 1.0);
            // Normalise so that input=1.0 maps to output≈1.0
            let white = hable(1.0, 1.0);
            (mapped / white).clamp(0.0, 1.0)
        }
        ToneMapOperator::Reinhard => reinhard(channel).clamp(0.0, 1.0),
        ToneMapOperator::Aces => aces_approx(channel).clamp(0.0, 1.0),
    }
}

/// Apply tone mapping to an RGBA tuple (r, g, b, a) in linear-light space.
///
/// Only the RGB channels are tone-mapped; alpha is passed through unchanged.
#[inline]
pub fn tone_map_rgba(
    r: f32,
    g: f32,
    b: f32,
    a: f32,
    operator: ToneMapOperator,
) -> (f32, f32, f32, f32) {
    (
        tone_map(r, operator),
        tone_map(g, operator),
        tone_map(b, operator),
        a,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn hable_monotonic() {
        let mut prev = 0.0f32;
        for i in 0..100 {
            let x = i as f32 / 100.0;
            let y = hable(x, 1.0);
            assert!(y >= prev, "Hable not monotonic at {x}: {y} < {prev}");
            prev = y;
        }
    }

    #[test]
    fn reinhard_zero() {
        assert_eq!(reinhard(0.0), 0.0);
    }

    #[test]
    fn reinhard_asymptotic() {
        // For large x, reinhard(x) → 1.0 (since x/(1+x) → 1)
        let result = reinhard(100.0);
        assert!(result < 1.0, "reinhard(100) = {result} should be < 1");
        assert!(result > 0.99, "reinhard(100) = {result} should be > 0.99");
    }

    #[test]
    fn aces_approx_range() {
        for i in 0..100 {
            let x = i as f32 / 100.0;
            let y = aces_approx(x);
            assert!(y >= 0.0, "ACES negative at {x}: {y}");
            assert!(y <= 1.0, "ACES > 1.0 at {x}: {y}");
        }
    }

    #[test]
    fn tone_map_dispatch() {
        let x = 0.5f32;
        for op in &[
            ToneMapOperator::Hable,
            ToneMapOperator::Reinhard,
            ToneMapOperator::Aces,
        ] {
            let result = tone_map(x, *op);
            assert!(result >= 0.0);
            assert!(result <= 1.0);
        }
    }

    #[test]
    fn tone_map_rgba_alpha_passthrough() {
        let (r, g, b, a) = tone_map_rgba(0.5, 0.3, 0.7, 0.25, ToneMapOperator::Hable);
        assert_eq!(a, 0.25);
        assert!(r >= 0.0);
        assert!(g >= 0.0);
        assert!(b >= 0.0);
    }

    #[test]
    fn hable_normalisation_white() {
        // Input 1.0 should map close to 1.0 after normalisation
        let white = hable(1.0, 1.0);
        let mapped = hable(1.0, 1.0);
        let normalised = mapped / white;
        assert!(approx_eq(normalised, 1.0, 1e-6));
    }
}
