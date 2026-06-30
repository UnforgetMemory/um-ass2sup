use color_quantizer::color::ColorSpace;
use color_quantizer::Rgba;

/// A single entry in a PGS palette (YCbCrA format).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PaletteEntry {
    pub index: u8,
    pub y: u8,
    pub cb: u8,
    pub cr: u8,
    pub alpha: u8,
}

/// Choose the PGS colour space based on display height (legacy heuristic).
pub fn color_space_for_height(display_height: u16) -> ColorSpace {
    if display_height > 576 {
        ColorSpace::Bt709
    } else {
        ColorSpace::Srgb
    }
}

/// Swap palette index 0 with a pivot value.
///
/// Used by the encoder to remap `transparent_index → 0` before RLE encoding
/// (since the RLE format uses 0 as the transparent run marker) and by the
/// decoder to reverse this mapping after RLE decoding.
#[inline]
pub fn swap(val: u8, pivot: u8) -> u8 {
    if val == 0 {
        pivot
    } else if val == pivot {
        0
    } else {
        val
    }
}

/// Convert YCbCrA (BT.601 full-range) to RGBA.
///
/// This is the exact inverse of `rgba_to_ycbcr_bt601`.
#[inline]
pub fn ycbcr_to_rgba(y: u8, cb: u8, cr: u8, alpha: u8) -> [u8; 4] {
    let y = f64::from(y);
    let cb = f64::from(cb);
    let cr = f64::from(cr);

    let r = (y + 1.402 * (cr - 128.0)).round().clamp(0.0, 255.0) as u8;
    let g = (y - 0.344 * (cb - 128.0) - 0.714 * (cr - 128.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    let b = (y + 1.772 * (cb - 128.0)).round().clamp(0.0, 255.0) as u8;

    [r, g, b, alpha]
}

/// Convert a PGS palette (YCbCrA entries) to flat RGBA bytes.
pub fn palette_to_rgba(entries: &[PaletteEntry]) -> Vec<[u8; 4]> {
    entries
        .iter()
        .map(|e| ycbcr_to_rgba(e.y, e.cb, e.cr, e.alpha))
        .collect()
}

/// Convert RGBA to YCbCr using BT.601 coefficients.
#[inline]
fn rgba_to_ycbcr_bt601(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r = f64::from(r);
    let g = f64::from(g);
    let b = f64::from(b);

    let y = (0.299 * r + 0.587 * g + 0.114 * b)
        .round()
        .clamp(0.0, 255.0) as u8;
    let cb = (-0.169 * r - 0.331 * g + 0.500 * b + 128.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let cr = (0.500 * r - 0.419 * g - 0.081 * b + 128.0)
        .round()
        .clamp(0.0, 255.0) as u8;

    (y, cb, cr)
}

/// Convert RGBA to YCbCr using BT.709 coefficients.
#[inline]
fn rgba_to_ycbcr_bt709(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r = f64::from(r);
    let g = f64::from(g);
    let b = f64::from(b);

    let y = (0.2126 * r + 0.7152 * g + 0.0722 * b)
        .round()
        .clamp(0.0, 255.0) as u8;
    let cb = (-0.1146 * r - 0.3854 * g + 0.500 * b + 128.0)
        .round()
        .clamp(0.0, 255.0) as u8;
    let cr = (0.500 * r - 0.4542 * g - 0.0458 * b + 128.0)
        .round()
        .clamp(0.0, 255.0) as u8;

    (y, cb, cr)
}

fn rgba_to_ycbcr_cs(r: u8, g: u8, b: u8, cs: ColorSpace) -> (u8, u8, u8) {
    match cs {
        ColorSpace::Bt709 => rgba_to_ycbcr_bt709(r, g, b),
        _ => rgba_to_ycbcr_bt601(r, g, b),
    }
}

/// Convert RGBA color to YCbCr using BT.601 colour space.
pub fn rgba_to_ycbcr(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    rgba_to_ycbcr_bt601(r, g, b)
}

/// Build a PGS palette from RGBA pixels using the specified colour space.
pub fn build_palette(palette: &[Rgba], color_space: ColorSpace) -> Vec<PaletteEntry> {
    palette
        .iter()
        .enumerate()
        .map(|(i, rgba)| {
            let (y, cb, cr) = rgba_to_ycbcr_cs(rgba.r, rgba.g, rgba.b, color_space);
            PaletteEntry {
                index: i as u8,
                y,
                cb,
                cr,
                alpha: rgba.a,
            }
        })
        .collect()
}
