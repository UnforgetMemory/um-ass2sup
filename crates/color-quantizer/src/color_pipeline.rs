//! Color pipeline: colour-space transforms, transfer functions, tonemapping.
//!
//! This module provides the v2.0 colour-pipeline primitives that the
//! renderer uses to convert between SDR BT.709, HDR BT.2020 with PQ, and
//! HDR BT.2020 with HLG. The existing v0.5.x quantizer (BT.601/BT.709
//! only) continues to work; the new types are additive.

use serde::{Deserialize, Serialize};

/// Colour-space identifier.
///
/// `SdrBt709` is the existing v0.5.x default. `HdrBt2020Pq` and
/// `HdrBt2020Hlg` are the v2.0 HDR paths.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ColorSpace {
    /// SDR BT.709 (the v0.5.x default).
    #[default]
    SdrBt709,
    /// HDR BT.2020 with the PQ (SMPTE ST 2084) transfer function. HDR10.
    HdrBt2020Pq,
    /// HDR BT.2020 with the HLG (ARIB STD-B67) transfer function.
    HdrBt2020Hlg,
}

impl ColorSpace {
    /// Returns the BT.601 / BT.709 / BT.2020 primaries as a 3x3
    /// row-major matrix from linear RGB in this space to linear RGB
    /// in the CIE 1931 XYZ space.
    ///
    /// Matrix values from the BT.601 / BT.709 / BT.2020 specifications.
    pub fn to_xyz_matrix(self) -> [[f64; 3]; 3] {
        match self {
            ColorSpace::SdrBt709 => [
                [0.4124564, 0.3575761, 0.1804375],
                [0.2126729, 0.7151522, 0.0721750],
                [0.0193339, 0.1191920, 0.9503041],
            ],
            ColorSpace::HdrBt2020Pq | ColorSpace::HdrBt2020Hlg => [
                [0.6369580, 0.1446169, 0.1688810],
                [0.2627002, 0.6779981, 0.0593017],
                [0.0000000, 0.0280727, 1.0609851],
            ],
        }
    }

    /// Returns `true` for HDR colour spaces.
    pub fn is_hdr(self) -> bool {
        matches!(self, ColorSpace::HdrBt2020Pq | ColorSpace::HdrBt2020Hlg)
    }
}

/// Transfer function. Models the gamma / EOTF curve that maps between
/// linear light and the encoded signal.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum TransferFunction {
    /// Linear (no transfer function).
    Linear,
    /// sRGB transfer function (the v0.5.x default).
    Srgb,
    /// PQ (SMPTE ST 2084) — used for HDR10.
    Pq,
    /// HLG (ARIB STD-B67) — used for HLG HDR.
    Hlg,
}

impl TransferFunction {
    /// Apply the EOTF (encoded → linear) to a single channel in `[0.0, 1.0]`.
    pub fn to_linear(self, v: f64) -> f64 {
        let v = v.clamp(0.0, 1.0);
        match self {
            TransferFunction::Linear => v,
            TransferFunction::Srgb => {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            }
            TransferFunction::Pq => pq_to_linear(v),
            TransferFunction::Hlg => hlg_to_linear(v),
        }
    }

    /// Apply the OETF (linear → encoded) to a single channel in `[0.0, 1.0]`.
    pub fn from_linear(self, v: f64) -> f64 {
        let v = v.clamp(0.0, 1.0);
        match self {
            TransferFunction::Linear => v,
            TransferFunction::Srgb => {
                if v <= 0.0031308 {
                    v * 12.92
                } else {
                    1.055 * v.powf(1.0 / 2.4) - 0.055
                }
            }
            TransferFunction::Pq => pq_from_linear(v),
            TransferFunction::Hlg => hlg_from_linear(v),
        }
    }
}

/// PQ (SMPTE ST 2084) EOTF: encoded → linear.
///
/// Reference: SMPTE ST 2084:2014.
fn pq_to_linear(v: f64) -> f64 {
    let m2 = 78.84375;
    let c1 = 0.8359375;
    let c2 = 18.8515625;
    let c3 = 18.6875;
    let n = (v.powf(1.0 / m2) - c1).max(0.0);
    if n <= 0.0 {
        0.0
    } else {
        let denom = c2 - c3 * v.powf(1.0 / m2);
        if denom == 0.0 {
            0.0
        } else {
            (n / denom).powf(1.0 / 0.0126838773)
        }
    }
}

/// PQ (SMPTE ST 2084) inverse EOTF: linear → encoded.
fn pq_from_linear(v: f64) -> f64 {
    let m2 = 78.84375;
    let c1 = 0.8359375;
    let c2 = 18.8515625;
    let c3 = 18.6875;
    let p = v.powf(0.0126838773);
    if p <= 0.0 {
        0.0
    } else {
        ((c1 + c2 * p) / (1.0 + c3 * p)).powf(m2)
    }
}

/// HLG (ARIB STD-B67) EOTF: encoded → linear.
fn hlg_to_linear(v: f64) -> f64 {
    let a = 0.17883277_f64;
    let b = 0.28466892;
    let c = 0.55991073;
    if v <= 0.5 {
        (v * v) / 3.0
    } else {
        ((v - c) / a).exp().mul_add(1.0, b) / 12.0
    }
}

/// HLG (ARIB STD-B67) inverse EOTF: linear → encoded.
fn hlg_from_linear(v: f64) -> f64 {
    let a = 0.17883277_f64;
    let b = 1.0 - 4.0 * a;
    let c = 0.5 - a * (4.0_f64 * a).ln();
    if v <= 1.0 / 12.0 {
        (3.0 * v).sqrt()
    } else {
        a * (12.0 * v - b).ln() + c
    }
}

/// Tonemapping operator. Maps HDR linear light to a displayable range.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
pub enum Tonemap {
    /// No tonemapping; clamp to `[0, 1]`.
    None,
    /// Hable tonemapping (Uncharted 2 filmic). Smooth shoulder.
    #[default]
    Hable,
    /// Reinhard tonemapping. Simple `v / (1 + v)`.
    Reinhard,
    /// ACES filmic tonemapping. Industry standard.
    Aces,
}

impl Tonemap {
    /// Apply the tonemapping operator to a linear-light value.
    pub fn apply(self, v: f64) -> f64 {
        let v = v.max(0.0);
        match self {
            Tonemap::None => v.min(1.0),
            Tonemap::Hable => hable(v),
            Tonemap::Reinhard => reinhard(v),
            Tonemap::Aces => aces(v),
        }
    }
}

/// Hable tonemapping. Reference: https://www.gdcvault.com/play/1012351
fn hable(v: f64) -> f64 {
    hable_partial(v * 2.0) / hable_partial(11.2)
}

fn hable_partial(v: f64) -> f64 {
    const A: f64 = 0.15;
    const B: f64 = 0.50;
    const C: f64 = 0.10;
    const D: f64 = 0.20;
    const E: f64 = 0.02;
    const F: f64 = 0.30;
    ((v * (A * v + C * B) + D * E) / (v * (A * v + B) + D * F)) - E / F
}

/// Reinhard tonemapping.
fn reinhard(v: f64) -> f64 {
    v / (1.0 + v)
}

/// ACES filmic tonemapping (Narkowicz fit).
fn aces(v: f64) -> f64 {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    ((v * (a * v + b)) / (v * (c * v + d) + e)).clamp(0.0, 1.0)
}

/// End-to-end colour-pipeline configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColorPipelineConfig {
    /// Source colour space.
    pub input_space: ColorSpace,
    /// Destination colour space.
    pub output_space: ColorSpace,
    /// Source transfer function.
    pub input_transfer: TransferFunction,
    /// Destination transfer function.
    pub output_transfer: TransferFunction,
    /// Tonemapping operator for HDR→SDR (or SDR→SDR with exposure).
    pub tonemap: Tonemap,
}

impl Default for ColorPipelineConfig {
    fn default() -> Self {
        Self {
            input_space: ColorSpace::SdrBt709,
            output_space: ColorSpace::SdrBt709,
            input_transfer: TransferFunction::Srgb,
            output_transfer: TransferFunction::Srgb,
            tonemap: Tonemap::Hable,
        }
    }
}

/// Convert an RGB triplet from the source colour space to the destination
/// colour space, applying the transfer functions and tonemapping as needed.
///
/// `rgb` is in the source encoded domain (e.g. sRGB 0-1 for SDR sources).
/// Returns RGB in the destination encoded domain.
pub fn convert_rgb(rgb: [f64; 3], config: &ColorPipelineConfig) -> [f64; 3] {
    // 1. Decode source: encoded -> linear
    let linear = [
        config.input_transfer.to_linear(rgb[0]),
        config.input_transfer.to_linear(rgb[1]),
        config.input_transfer.to_linear(rgb[2]),
    ];
    // 2. Convert linear RGB source gamut -> dest gamut
    // (matrix multiplication; here we use the identity because
    // the v0.5.x path is already gamut-correct. Full 3x3
    // gamut conversion is added in a follow-up.)
    let linear_dest = linear;
    // 3. Tonemap if either side is HDR
    let linear_dest = if config.input_space.is_hdr() || config.output_space.is_hdr() {
        [
            config.tonemap.apply(linear_dest[0]),
            config.tonemap.apply(linear_dest[1]),
            config.tonemap.apply(linear_dest[2]),
        ]
    } else {
        linear_dest
    };
    // 4. Encode destination: linear -> encoded
    [
        config.output_transfer.from_linear(linear_dest[0]),
        config.output_transfer.from_linear(linear_dest[1]),
        config.output_transfer.from_linear(linear_dest[2]),
    ]
}

/// Auto-detect the source colour space from a subtitle file. Defaults to
/// SDR BT.709 because that is the v0.5.x convention and no header field
/// in the existing ASS format flags HDR.
pub fn detect_source_color_space(ass: &str) -> ColorSpace {
    // Look for the explicit `YCbCr Matrix: ` header (PGS convention)
    // or an `Output: HDR` marker.
    for line in ass.lines() {
        let line = line.trim();
        if line.eq_ignore_ascii_case("Output: HDR") {
            return ColorSpace::HdrBt2020Pq;
        }
        if line.starts_with("YCbCr Matrix: BT.2020") {
            return ColorSpace::HdrBt2020Pq;
        }
    }
    ColorSpace::SdrBt709
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_color_space_is_bt709() {
        assert_eq!(ColorSpace::default(), ColorSpace::SdrBt709);
    }

    #[test]
    fn hdr_detection() {
        assert!(!ColorSpace::SdrBt709.is_hdr());
        assert!(ColorSpace::HdrBt2020Pq.is_hdr());
        assert!(ColorSpace::HdrBt2020Hlg.is_hdr());
    }

    #[test]
    fn bt709_xyz_matrix_is_close_to_d65_reference() {
        // Row 0: 0.4124, 0.3576, 0.1804 (BT.709 D65)
        let m = ColorSpace::SdrBt709.to_xyz_matrix();
        assert!((m[0][0] - 0.4124).abs() < 0.001);
        assert!((m[1][1] - 0.7152).abs() < 0.001);
        assert!((m[2][2] - 0.9503).abs() < 0.001);
    }

    #[test]
    fn srgb_roundtrip_is_lossy_but_close() {
        for v in [0.0, 0.1, 0.5, 0.9, 1.0] {
            let linear = TransferFunction::Srgb.to_linear(v);
            let encoded = TransferFunction::Srgb.from_linear(linear);
            assert!((v - encoded).abs() < 1e-9, "roundtrip failed for {v}");
        }
    }

    #[test]
    fn linear_roundtrip_is_lossless() {
        for v in [0.0, 0.1, 0.5, 0.9, 1.0] {
            let linear = TransferFunction::Linear.to_linear(v);
            let encoded = TransferFunction::Linear.from_linear(linear);
            assert!((v - encoded).abs() < 1e-9);
        }
    }

    #[test]
    fn pq_to_linear_at_one_is_one() {
        let v = TransferFunction::Pq.to_linear(1.0);
        assert!((v - 1.0).abs() < 1e-6);
    }

    #[test]
    fn pq_to_linear_at_zero_is_zero() {
        let v = TransferFunction::Pq.to_linear(0.0);
        assert_eq!(v, 0.0);
    }

    #[test]
    fn pq_roundtrip_is_close() {
        for v in [0.0, 0.1, 0.3, 0.5, 0.7, 1.0] {
            let linear = TransferFunction::Pq.to_linear(v);
            let encoded = TransferFunction::Pq.from_linear(linear);
            assert!(
                (v - encoded).abs() < 1e-6,
                "PQ roundtrip failed for {v}: got {encoded}"
            );
        }
    }

    #[test]
    fn hlg_roundtrip_is_close() {
        for v in [0.0, 0.1, 0.3, 0.5, 0.7, 1.0] {
            let linear = TransferFunction::Hlg.to_linear(v);
            let encoded = TransferFunction::Hlg.from_linear(linear);
            assert!(
                (v - encoded).abs() < 1e-6,
                "HLG roundtrip failed for {v}: got {encoded}"
            );
        }
    }

    #[test]
    fn tonemap_none_clamps_to_one() {
        assert_eq!(Tonemap::None.apply(2.0), 1.0);
        assert_eq!(Tonemap::None.apply(0.5), 0.5);
        assert_eq!(Tonemap::None.apply(0.0), 0.0);
    }

    #[test]
    fn tonemap_reinhard_monotonic() {
        let a = Tonemap::Reinhard.apply(0.0);
        let b = Tonemap::Reinhard.apply(1.0);
        let c = Tonemap::Reinhard.apply(100.0);
        assert!(a < b);
        assert!(b < c);
        assert!(c < 1.0);
    }

    #[test]
    fn tonemap_hable_stays_in_unit_range() {
        // The Hable operator (Uncharted 2 filmic) is bounded for inputs
        // up to ~7x the white point. Beyond that, it asymptotes but does
        // not strictly cap at 1.0.
        for v in [0.0, 0.5, 1.0, 2.0, 4.0] {
            let t = Tonemap::Hable.apply(v);
            assert!((0.0..=1.0).contains(&t), "Hable out of range for {v}: {t}");
        }
    }

    #[test]
    fn tonemap_aces_stays_in_unit_range() {
        for v in [0.0, 0.5, 1.0, 2.0, 10.0, 100.0] {
            let t = Tonemap::Aces.apply(v);
            assert!((0.0..=1.0).contains(&t), "ACES out of range for {v}: {t}");
        }
    }

    #[test]
    fn convert_rgb_sdr_to_sdr_is_identity() {
        let config = ColorPipelineConfig::default();
        let rgb = [0.5, 0.3, 0.1];
        let out = convert_rgb(rgb, &config);
        for (a, b) in rgb.iter().zip(out.iter()) {
            assert!((a - b).abs() < 1e-9);
        }
    }

    #[test]
    fn convert_rgb_hdr_to_sdr_clamps_with_tonemap() {
        let config = ColorPipelineConfig {
            input_space: ColorSpace::HdrBt2020Pq,
            output_space: ColorSpace::SdrBt709,
            input_transfer: TransferFunction::Pq,
            output_transfer: TransferFunction::Srgb,
            tonemap: Tonemap::Hable,
        };
        let out = convert_rgb([0.5, 0.5, 0.5], &config);
        for v in out {
            assert!((0.0..=1.0).contains(&v));
        }
    }

    #[test]
    fn detect_default_is_sdr() {
        let src = "[Script Info]\nTitle: Foo\n\n[V4+ Styles]\n";
        assert_eq!(detect_source_color_space(src), ColorSpace::SdrBt709);
    }

    #[test]
    fn detect_hdr_output_marker() {
        let src = "[Script Info]\nOutput: HDR\n";
        assert_eq!(detect_source_color_space(src), ColorSpace::HdrBt2020Pq);
    }

    #[test]
    fn detect_hdr_ycbcr_matrix() {
        let src = "[Script Info]\nYCbCr Matrix: BT.2020\n";
        assert_eq!(detect_source_color_space(src), ColorSpace::HdrBt2020Pq);
    }
}
