#![allow(missing_docs)]

//! Color-space conversion primitives.
//!
//! Provides conversion between sRGB, BT.709, BT.2020, and linear-light
//! representations. All conversion matrices are constant-evaluated.

use super::ColorSpace;

/// sRGB ↔ linear conversion (gamma ≈ 2.2 with a linear toe at 0.04045).
pub fn srgb_to_linear(c: f32) -> f32 {
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(c: f32) -> f32 {
    if c <= 0.0031308 {
        c * 12.92
    } else {
        c.powf(1.0 / 2.4) * 1.055 - 0.055
    }
}

/// BT.709 → linear-light using ITU-R BT.709 matrix.
pub fn bt709_to_linear(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let x = 0.412_391 * r + 0.357_584 * g + 0.180_481 * b;
    let y = 0.212_639 * r + 0.715_169 * g + 0.072_192 * b;
    let z = 0.019_331 * r + 0.119_195 * g + 0.950_532 * b;
    (x, y, z)
}

/// Linear-light → BT.709 using ITU-R BT.709 matrix.
pub fn linear_to_bt709(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    let r = 3.240_97 * x - 1.537_383 * y - 0.498_611 * z;
    let g = -0.969_229 * x + 1.875_928 * y + 0.041_555 * z;
    let b = 0.055_643 * x - 0.204_043 * y + 1.057_067 * z;
    (r, g, b)
}

/// BT.2020 → linear-light using ITU-R BT.2020 matrix.
pub fn bt2020_to_linear(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let x = 0.262_679 * r + 0.677_998 * g + 0.059_323 * b;
    let y = 0.559_998 * r + 0.336_001 * g + 0.104_001 * b;
    let z = 0.000_000 * r + 0.009_999 * g + 0.990_001 * b;
    (x, y, z)
}

/// Linear-light → BT.2020 using ITU-R BT.2020 matrix.
pub fn linear_to_bt2020(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    let r = 1.716_651 * x - 0.355_671 * y - 0.253_341 * z;
    let g = -0.666_684 * x + 1.616_481 * y + 0.015_768 * z;
    let b = 0.017_640 * x - 0.042_771 * y + 1.025_531 * z;
    (r, g, b)
}

/// Convert RGB in the given color space to CIE XYZ D65.
pub fn rgb_to_xyz(r: f32, g: f32, b: f32, space: ColorSpace) -> (f32, f32, f32) {
    match space {
        ColorSpace::Linear => (r, g, b),
        ColorSpace::Srgb => bt709_to_linear(r, g, b),
        ColorSpace::Bt709 => bt709_to_linear(r, g, b),
        ColorSpace::Bt2020 => bt2020_to_linear(r, g, b),
    }
}

/// Convert CIE XYZ D65 to RGB in the given color space.
pub fn xyz_to_rgb(x: f32, y: f32, z: f32, space: ColorSpace) -> (f32, f32, f32) {
    match space {
        ColorSpace::Linear => (x, y, z),
        ColorSpace::Srgb => linear_to_bt709(x, y, z),
        ColorSpace::Bt709 => linear_to_bt709(x, y, z),
        ColorSpace::Bt2020 => linear_to_bt2020(x, y, z),
    }
}

/// Convert CIE XYZ D65 to CIE L*a*b*.
pub fn xyz_to_lab(x: f32, y: f32, z: f32) -> (f32, f32, f32) {
    const XN: f32 = 0.95047;
    const YN: f32 = 1.0;
    const ZN: f32 = 1.08883;
    const DELTA: f32 = 6.0 / 29.0;
    const EPSILON: f32 = 216.0 / 24389.0;

    fn lab_f(t: f32) -> f32 {
        if t > EPSILON {
            t.powf(1.0 / 3.0)
        } else {
            t / (3.0 * DELTA * DELTA) + 4.0 / 29.0
        }
    }

    let fx = lab_f(x / XN);
    let fy = lab_f(y / YN);
    let fz = lab_f(z / ZN);

    let l = 116.0 * fy - 16.0;
    let a = 500.0 * (fx - fy);
    let b = 200.0 * (fy - fz);

    (l, a, b)
}

/// Convert RGB between two color spaces via linear-light and XYZ D65.
pub fn convert(r: f32, g: f32, b: f32, from: ColorSpace, to: ColorSpace) -> (f32, f32, f32) {
    if from == to {
        return (r, g, b);
    }

    let (x, y, z) = rgb_to_xyz(r, g, b, from);
    xyz_to_rgb(x, y, z, to)
}
