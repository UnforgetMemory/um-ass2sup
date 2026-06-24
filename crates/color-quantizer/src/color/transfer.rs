#![allow(missing_docs)]

//! Transfer functions for HDR and SDR color pipelines.
//!
//! Defines the electro-optical transfer functions (EOTF / inverse of gamma)
//! and opto-electronic transfer functions (OETF / gamma) for:
//!
//! | Standard | EOTF (linear ← encoded) | OETF (encoded ← linear) |
//! |----------|--------------------------|--------------------------|
//! | sRGB     | IEC 61966-2-1            | sRGB γ ≈ 2.2             |
//! | BT.1886  | Pure power 2.4           | Pure power 1/2.4          |
//! | PQ       | SMPTE ST 2084            | Perceptual Quantizer     |
//! | HLG      | ARIB STD-B67             | Hybrid Log-Gamma         |
//!
//! All functions operate on normalised `f32` values in [0.0, 1.0].

/// Apply the sRGB opto-electronic transfer function (gamma encode).
///
/// Maps linear-light in [0.0, 1.0] to non-linear sRGB in [0.0, 1.0].
#[inline]
pub fn srgb_oetf(linear: f32) -> f32 {
    if linear <= 0.0031308 {
        linear * 12.92
    } else {
        linear.powf(1.0 / 2.4) * 1.055 - 0.055
    }
}

/// Apply the sRGB electro-optical transfer function (gamma decode).
///
/// Maps non-linear sRGB in [0.0, 1.0] to linear-light in [0.0, 1.0].
#[inline]
pub fn srgb_eotf(encoded: f32) -> f32 {
    if encoded <= 0.04045 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

/// Apply pure power (BT.1886) EOTF with γ = 2.4.
#[inline]
pub fn bt1886_eotf(encoded: f32) -> f32 {
    encoded.powf(2.4)
}

/// Apply pure power OETF with γ = 1/2.4.
#[inline]
pub fn bt1886_oetf(linear: f32) -> f32 {
    linear.powf(1.0 / 2.4)
}

// ---------------------------------------------------------------------------
// Perceptual Quantizer (PQ) — SMPTE ST 2084
// ---------------------------------------------------------------------------
//
// PQ maps absolute luminance (0–10 000 cd/m²) to 12-bit integer codes using
// a non-linear transfer function optimised for human contrast sensitivity.
//
// Constants from SMPTE ST 2084:2014 / ITU-R BT.2100-2.
//
// EOTF:  linear luminance  ←  non-linear code
// OETF:  non-linear code   ←  linear luminance

/// Maximum luminance PQ can represent (cd/m²).
pub const PQ_PEAK_LUMINANCE: f32 = 10_000.0;

const PQ_M1: f32 = 2610.0 / 16384.0; // 0.1593017578125
const PQ_M2: f32 = 2523.0 * 128.0 / 4096.0; // 78.84375
const PQ_C1: f32 = 3424.0 / 4096.0; // 0.8359375
const PQ_C2: f32 = 2413.0 * 32.0 / 4096.0; // 18.8515625
const PQ_C3: f32 = 2392.0 * 32.0 / 4096.0; // 18.6875

/// Apply PQ electro-optical transfer function (ST 2084 EOTF).
///
/// Converts non-linear PQ code in [0.0, 1.0] to linear luminance in
/// [0.0, 1.0] (normalised to `PQ_PEAK_LUMINANCE`).
///
/// Equivalent to SMPTE ST 2084 EOTF: V → Y.
#[inline]
pub fn pq_eotf(encoded: f32) -> f32 {
    let v = encoded.clamp(0.0, 1.0);
    let v_pow = v.powf(1.0 / PQ_M2);
    let num = (v_pow - PQ_C1).max(0.0);
    let den = (PQ_C2 - PQ_C3 * v_pow).max(f32::MIN_POSITIVE);
    (num / den).powf(1.0 / PQ_M1)
}

/// Apply PQ opto-electronic transfer function (ST 2084 OETF).
///
/// Converts linear luminance in [0.0, 1.0] (normalised) to non-linear PQ code
/// in [0.0, 1.0].
///
/// Equivalent to SMPTE ST 2084 OETF: Y → V.
#[inline]
pub fn pq_oetf(linear: f32) -> f32 {
    let y = linear.clamp(0.0, 1.0);
    let y_pow = y.powf(PQ_M1);
    let num = PQ_C1 + PQ_C2 * y_pow;
    let den = 1.0 + PQ_C3 * y_pow;
    (num / den).powf(PQ_M2)
}

/// Apply PQ EOTF to absolute luminance (cd/m²).
#[inline]
pub fn pq_eotf_absolute(encoded: f32) -> f32 {
    pq_eotf(encoded) * PQ_PEAK_LUMINANCE
}

/// Apply PQ OETF from absolute luminance (cd/m²).
#[inline]
pub fn pq_oetf_absolute(luminance_cd_m2: f32) -> f32 {
    pq_oetf(luminance_cd_m2 / PQ_PEAK_LUMINANCE)
}

// ---------------------------------------------------------------------------
// Hybrid Log-Gamma (HLG) — ARIB STD-B67 / ITU-R BT.2100-2
// ---------------------------------------------------------------------------
//
// HLG uses a hybrid curve: a logarithmic segment for highlights and a gamma
// segment for the lower range. Unlike PQ it is scene-referred and relative.
//
// Constants: a = 0.17883277, b = 0.28466892, c = 0.55991073

const HLG_A: f32 = 0.17883277;
const HLG_B: f32 = 0.28466892;
const HLG_C: f32 = 0.559_910_7;

/// Apply HLG OETF (scene-light → non-linear code).
///
/// Input `linear` is normalised scene-referred linear light in [0.0, 1.0].
/// Output is HLG non-linear code in [0.0, 1.0].
#[inline]
pub fn hlg_oetf(linear: f32) -> f32 {
    let l = linear.clamp(0.0, 1.0);
    if l <= 1.0 / 12.0 {
        (3.0 * l).sqrt()
    } else {
        HLG_A * (12.0 * l - HLG_B).ln() + HLG_C
    }
}

/// Apply HLG EOTF (non-linear code → display-referred linear light).
///
/// Inverse of `hlg_oetf`. The output is normalised linear light in [0.0, 1.0].
#[inline]
pub fn hlg_eotf(encoded: f32) -> f32 {
    let e = encoded.clamp(0.0, 1.0);
    if e <= 0.5 {
        (e * e) / 3.0
    } else {
        (((e - HLG_C) / HLG_A).exp() + HLG_B) / 12.0
    }
}

// ---------------------------------------------------------------------------
// High-level dispatch
// ---------------------------------------------------------------------------

/// Identifies the transfer function standard.
#[derive(Copy, Clone, Debug, PartialEq, Default)]
pub enum TransferFunction {
    /// sRGB (IEC 61966-2-1), the default for SDR content
    #[default]
    Srgb,
    /// Pure power 2.4 (BT.1886), common in video
    Bt1886,
    /// Perceptual Quantizer (SMPTE ST 2084) for HDR10
    Pq,
    /// Hybrid Log-Gamma (ARIB STD-B67) for broadcast HDR
    Hlg,
}

/// Apply the EOTF (decode: non-linear code → linear-light) for the given
/// transfer function.
#[inline]
pub fn eotf(encoded: f32, tf: TransferFunction) -> f32 {
    match tf {
        TransferFunction::Srgb => srgb_eotf(encoded),
        TransferFunction::Bt1886 => bt1886_eotf(encoded),
        TransferFunction::Pq => pq_eotf(encoded),
        TransferFunction::Hlg => hlg_eotf(encoded),
    }
}

/// Apply the OETF (encode: linear-light → non-linear code) for the given
/// transfer function.
#[inline]
pub fn oetf(linear: f32, tf: TransferFunction) -> f32 {
    match tf {
        TransferFunction::Srgb => srgb_oetf(linear),
        TransferFunction::Bt1886 => bt1886_oetf(linear),
        TransferFunction::Pq => pq_oetf(linear),
        TransferFunction::Hlg => hlg_oetf(linear),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() < eps
    }

    #[test]
    fn srgb_roundtrip() {
        for linear in [0.0, 0.0031308, 0.04045, 0.2, 0.5, 0.8, 1.0] {
            let encoded = srgb_oetf(linear);
            let decoded = srgb_eotf(encoded);
            assert!(
                approx_eq(linear, decoded, 1e-4),
                "sRGB roundtrip failed at {linear}"
            );
        }
    }

    #[test]
    fn srgb_oetf_dc_bias() {
        // At exactly 0.0031308 the two segments should be continuous.
        let at_knee = 0.0031308f32;
        let encoded = srgb_oetf(at_knee);
        assert!(encoded > 0.0, "sRGB OETF at knee should be > 0");
        assert!(encoded < 0.1, "sRGB OETF at knee should be < 0.1");
    }

    #[test]
    fn pq_symmetry() {
        // PQ EOTF's minimum non-zero output starts at code ~0.894 (64/4095).
        // Only test codes > 0.9 where the EOTF produces measurable luminance.
        for v in [0.91, 0.92, 0.95, 0.99, 1.0] {
            let linear = pq_eotf(v);
            let re_encoded = pq_oetf(linear);
            assert!(
                approx_eq(v, re_encoded, 2e-3),
                "PQ roundtrip failed at {v}: {linear} → {re_encoded}"
            );
        }
    }

    #[test]
    fn pq_two_thresholds() {
        // PQ EOTF requires code > ~0.894 before producing non-zero output
        // (the minimum non-zero 12-bit code is 64/4095). Test at 0.95 and 0.99.
        // ST 2084 reference: code 0.95 → ~8000 cd/m²; code 0.99 → ~9800 cd/m²
        let cd_at_95 = pq_eotf_absolute(0.95);
        assert!(
            cd_at_95 > 5000.0,
            "PQ 0.95 should be > 5000 cd/m², got {cd_at_95}"
        );
        assert!(
            cd_at_95 < 9800.0,
            "PQ 0.95 should be < 9800 cd/m², got {cd_at_95}"
        );
        let cd_at_99 = pq_eotf_absolute(0.99);
        assert!(
            cd_at_99 > 9000.0,
            "PQ 0.99 should be > 9000 cd/m², got {cd_at_99}"
        );
        assert!(
            cd_at_99 <= 10000.0,
            "PQ 0.99 should be <= 10000 cd/m², got {cd_at_99}"
        );
    }

    #[test]
    fn hlg_roundtrip() {
        for linear in [0.0, 0.01, 0.05, 0.1, 0.25, 0.5, 0.75, 1.0] {
            let encoded = hlg_oetf(linear);
            let decoded = hlg_eotf(encoded);
            assert!(
                approx_eq(linear, decoded, 1e-4),
                "HLG roundtrip failed at {linear}: {encoded} → {decoded}"
            );
        }
    }

    #[test]
    fn hlg_knee_continuity() {
        // The HLG knee is at 1/12 ≈ 0.0833. Both segments should meet.
        let below = hlg_oetf(1.0 / 12.0 - 0.001);
        let above = hlg_oetf(1.0 / 12.0 + 0.001);
        let diff = (below - above).abs();
        assert!(
            diff < 0.01,
            "HLG knee discontinuity: {below} vs {above}, diff={diff}"
        );
    }

    #[test]
    fn transfer_function_dispatch_srgb() {
        let tf = TransferFunction::Srgb;
        let encoded = oetf(0.5, tf);
        let decoded = eotf(encoded, tf);
        assert!(approx_eq(0.5, decoded, 1e-4));
    }

    #[test]
    fn transfer_function_dispatch_pq() {
        let tf = TransferFunction::Pq;
        let encoded = oetf(0.5, tf);
        let decoded = eotf(encoded, tf);
        assert!(approx_eq(0.5, decoded, 2e-3));
    }

    #[test]
    fn transfer_function_dispatch_hlg() {
        let tf = TransferFunction::Hlg;
        let encoded = oetf(0.5, tf);
        let decoded = eotf(encoded, tf);
        assert!(approx_eq(0.5, decoded, 1e-4));
    }

    #[test]
    fn bt1886_roundtrip() {
        let tf = TransferFunction::Bt1886;
        let encoded = oetf(0.5, tf);
        let decoded = eotf(encoded, tf);
        assert!(approx_eq(0.5, decoded, 1e-4));
    }

    #[test]
    fn pq_clamp_negative() {
        // PQ EOTF should handle out-of-range input gracefully.
        let result = pq_eotf(-0.1);
        assert!(result >= 0.0);
        let result = pq_eotf(1.5);
        assert!(result <= 1.0);
    }

    #[test]
    fn hlg_clamp_negative() {
        let result = hlg_oetf(-0.1);
        assert!(result >= 0.0, "hlg_oetf(-0.1) should be >= 0, got {result}");
        let result = hlg_eotf(1.5);
        assert!(
            result <= 1.0 + 1e-6,
            "hlg_eotf(1.5) should be approx <= 1, got {result}"
        );
    }
}
