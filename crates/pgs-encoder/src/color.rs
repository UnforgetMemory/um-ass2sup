use crate::types::PaletteEntry;
use color_quantizer::Rgba;

/// Convert YCbCrA (BT.601 full-range) to RGBA.
///
/// This is the exact inverse of [`rgba_to_ycbcr`]. The YCbCr color space
/// is defined by ITU-R BT.601 with full-range coefficients (as used by
/// Blu-ray PGS subtitle palettes).
///
/// # Arguments
///
/// * `y`     — Luma component (0–255)
/// * `cb`    — Blue-difference chroma (0–255, 128 = neutral)
/// * `cr`    — Red-difference chroma (0–255, 128 = neutral)
/// * `alpha` — Alpha channel (0 = fully transparent, 255 = fully opaque)
///
/// # Example
///
/// ```
/// use pgs_encoder::color::ycbcr_to_rgba;
///
/// // White: Y=255, Cb=128, Cr=128
/// let rgba = ycbcr_to_rgba(255, 128, 128, 255);
/// assert_eq!(rgba, [255, 255, 255, 255]);
/// ```
#[inline]
pub fn ycbcr_to_rgba(y: u8, cb: u8, cr: u8, alpha: u8) -> [u8; 4] {
    let y = f64::from(y);
    let cb = f64::from(cb);
    let cr = f64::from(cr);

    // BT.601 inverse color transformation (full-range)
    // R' = Y + 1.402 × (Cr − 128)
    // G' = Y − 0.344 × (Cb − 128) − 0.714 × (Cr − 128)
    // B' = Y + 1.772 × (Cb − 128)
    let r = (y + 1.402 * (cr - 128.0)).round().clamp(0.0, 255.0) as u8;
    let g = (y - 0.344 * (cb - 128.0) - 0.714 * (cr - 128.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    let b = (y + 1.772 * (cb - 128.0)).round().clamp(0.0, 255.0) as u8;

    [r, g, b, alpha]
}

/// Convert a PGS palette (YCbCrA entries) to flat RGBA bytes.
///
/// Each palette entry is converted via [`ycbcr_to_rgba`], producing a
/// flat `Vec<[u8; 4]>` with RGBA8888 entries in palette-index order.
///
/// # Arguments
///
/// * `entries` — Slice of [`PaletteEntry`] from a PDS segment
///
/// # Example
///
/// ```
/// use pgs_encoder::color::palette_to_rgba;
/// use pgs_encoder::types::PaletteEntry;
///
/// let entries = vec![
///     PaletteEntry { index: 0, y: 16,  cb: 126, cr: 124, alpha: 0   }, // transparent
///     PaletteEntry { index: 1, y: 255, cb: 128, cr: 128, alpha: 255 }, // white
/// ];
/// let rgba = palette_to_rgba(&entries);
/// assert_eq!(rgba.len(), 2);
/// assert_eq!(rgba[0][3], 0);   // alpha = 0 (transparent)
/// assert_eq!(rgba[1][3], 255); // alpha = 255 (opaque)
/// ```
pub fn palette_to_rgba(entries: &[PaletteEntry]) -> Vec<[u8; 4]> {
    entries
        .iter()
        .map(|e| ycbcr_to_rgba(e.y, e.cb, e.cr, e.alpha))
        .collect()
}

pub fn rgba_to_ycbcr(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
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

/// Swap palette index 0 with a pivot value.
///
/// Used by the encoder to remap `transparent_index → 0` before RLE encoding
/// (since the RLE format uses 0 as the transparent run marker) and by the
/// decoder to reverse this mapping after RLE decoding.
///
/// - `swap(0, pivot)` → `pivot`
/// - `swap(pivot, pivot)` → `0`
/// - `swap(other, pivot)` → `other`
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

pub fn build_palette(palette: &[Rgba]) -> Vec<PaletteEntry> {
    palette
        .iter()
        .enumerate()
        .map(|(i, rgba)| {
            let (y, cb, cr) = rgba_to_ycbcr(rgba.r, rgba.g, rgba.b);
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── ycbcr_to_rgba tests ────────────────────────────────────────────────

    #[test]
    fn test_ycbcr_to_rgba_black() {
        // Y=0, Cb=128, Cr=128 → RGB black
        let rgba = ycbcr_to_rgba(0, 128, 128, 255);
        assert_eq!(rgba[0], 0);
        assert_eq!(rgba[1], 0);
        assert_eq!(rgba[2], 0);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn test_ycbcr_to_rgba_white() {
        // Y=255, Cb=128, Cr=128 → RGB white
        let rgba = ycbcr_to_rgba(255, 128, 128, 255);
        assert_eq!(rgba[0], 255);
        assert_eq!(rgba[1], 255);
        assert_eq!(rgba[2], 255);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn test_ycbcr_to_rgba_red() {
        let rgba = ycbcr_to_rgba(76, 85, 255, 255);
        // BT.601 roundtrip introduces ±1 rounding error — verify approximate
        assert!(
            (rgba[0] as i16 - 255).abs() <= 1,
            "R={} expected ~255",
            rgba[0]
        );
        assert_eq!(rgba[1], 0);
        assert_eq!(rgba[2], 0);
    }

    #[test]
    fn test_ycbcr_to_rgba_green() {
        let rgba = ycbcr_to_rgba(150, 44, 21, 255);
        assert!((rgba[0] as i16).abs() <= 1, "R={}", rgba[0]);
        assert!((rgba[1] as i16 - 255).abs() <= 1, "G={}", rgba[1]);
        assert!((rgba[2] as i16).abs() <= 1, "B={}", rgba[2]);
    }

    #[test]
    fn test_ycbcr_to_rgba_blue() {
        let rgba = ycbcr_to_rgba(29, 255, 107, 255);
        assert_eq!(rgba[0], 0);
        assert_eq!(rgba[1], 0);
        assert!(
            (rgba[2] as i16 - 255).abs() <= 1,
            "B={} expected ~255",
            rgba[2]
        );
    }

    #[test]
    fn test_ycbcr_to_rgba_transparent() {
        // alpha passes through unchanged
        let rgba = ycbcr_to_rgba(128, 128, 128, 0);
        assert_eq!(rgba[3], 0);
    }

    #[test]
    fn test_ycbcr_roundtrip_gray() {
        // Roundtrip: gray colors should be near-lossless
        for gray in [0u8, 32, 64, 128, 192, 224, 255] {
            let (y, cb, cr) = rgba_to_ycbcr(gray, gray, gray);
            let [r, g, b, _a] = ycbcr_to_rgba(y, cb, cr, 255);
            assert_eq!(r, gray, "gray={gray}: R mismatch");
            assert_eq!(g, gray, "gray={gray}: G mismatch");
            assert_eq!(b, gray, "gray={gray}: B mismatch");
        }
    }

    #[test]
    fn test_ycbcr_roundtrip_primary_colors() {
        let primaries = [
            (255, 0, 0),
            (0, 255, 0),
            (0, 0, 255),
            (255, 255, 0),
            (255, 0, 255),
            (0, 255, 255),
            (255, 255, 255),
        ];
        for (r, g, b) in primaries {
            let (y, cb, cr) = rgba_to_ycbcr(r, g, b);
            let [ro, go, bo, _] = ycbcr_to_rgba(y, cb, cr, 255);
            // BT.601 roundtrip has ±1 per-component tolerance due to float rounding
            assert!((ro as i16 - r as i16).abs() <= 1, "({r},{g},{b}): R={ro}");
            assert!((go as i16 - g as i16).abs() <= 1, "({r},{g},{b}): G={go}");
            assert!((bo as i16 - b as i16).abs() <= 1, "({r},{g},{b}): B={bo}");
        }
    }

    // ── palette_to_rgba tests ───────────────────────────────────────────────

    #[test]
    fn test_palette_to_rgba_black_white() {
        let entries = vec![
            PaletteEntry {
                index: 0,
                y: 16,
                cb: 126,
                cr: 124,
                alpha: 0,
            }, // transparent
            PaletteEntry {
                index: 1,
                y: 255,
                cb: 128,
                cr: 128,
                alpha: 255,
            }, // white
        ];
        let rgba = palette_to_rgba(&entries);
        assert_eq!(rgba.len(), 2);
        assert_eq!(rgba[0][3], 0);
        assert_eq!(rgba[1][3], 255);
    }

    #[test]
    fn test_palette_to_rgba_256_entries() {
        let entries: Vec<_> = (0u8..=255)
            .map(|i| PaletteEntry {
                index: i,
                y: i,
                cb: 128,
                cr: 128,
                alpha: i,
            })
            .collect();
        let rgba = palette_to_rgba(&entries);
        assert_eq!(rgba.len(), 256);
        // Alpha should pass through
        for (i, pixel) in rgba.iter().enumerate() {
            assert_eq!(pixel[3], i as u8, "alpha mismatch at index {i}");
        }
    }

    // ── existing rgba_to_ycbcr tests ────────────────────────────────────────

    #[test]
    fn test_rgba_to_ycbcr_black() {
        let (y, cb, cr) = rgba_to_ycbcr(0, 0, 0);
        assert_eq!(y, 0);
        assert_eq!(cb, 128);
        assert_eq!(cr, 128);
    }

    #[test]
    fn test_rgba_to_ycbcr_white() {
        let (y, cb, cr) = rgba_to_ycbcr(255, 255, 255);
        assert_eq!(y, 255);
        assert_eq!(cb, 128);
        assert_eq!(cr, 128);
    }

    #[test]
    fn test_rgba_to_ycbcr_red() {
        let (y, cb, cr) = rgba_to_ycbcr(255, 0, 0);
        assert_eq!(y, 76);
        assert_eq!(cb, 85);
        assert_eq!(cr, 255);
    }

    #[test]
    fn test_rgba_to_ycbcr_green() {
        let (y, cb, cr) = rgba_to_ycbcr(0, 255, 0);
        assert_eq!(y, 150);
        assert_eq!(cb, 44);
        assert_eq!(cr, 21);
    }

    #[test]
    fn test_rgba_to_ycbcr_blue() {
        let (y, cb, cr) = rgba_to_ycbcr(0, 0, 255);
        assert_eq!(y, 29);
        assert_eq!(cb, 255);
        assert_eq!(cr, 107);
    }

    #[test]
    fn test_build_palette() {
        let palette = vec![Rgba::new(0, 0, 0, 0), Rgba::new(255, 255, 255, 255)];
        let entries = build_palette(&palette);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].alpha, 0);
        assert_eq!(entries[1].index, 1);
        assert_eq!(entries[1].y, 255);
        assert_eq!(entries[1].alpha, 255);
    }
}
