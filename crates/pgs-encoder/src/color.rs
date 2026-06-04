use color_quantizer::Rgba;
use crate::types::PaletteEntry;

pub fn rgba_to_ycbcr(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    let r = f64::from(r);
    let g = f64::from(g);
    let b = f64::from(b);

    let y  = ( 0.299 * r + 0.587 * g + 0.114 * b).round().clamp(0.0, 255.0) as u8;
    let cb = (-0.169 * r - 0.331 * g + 0.500 * b + 128.0).round().clamp(0.0, 255.0) as u8;
    let cr = ( 0.500 * r - 0.419 * g - 0.081 * b + 128.0).round().clamp(0.0, 255.0) as u8;

    (y, cb, cr)
}

pub fn build_palette(palette: &[Rgba]) -> Vec<PaletteEntry> {
    palette.iter().enumerate().map(|(i, rgba)| {
        let (y, cb, cr) = rgba_to_ycbcr(rgba.r, rgba.g, rgba.b);
        PaletteEntry {
            index: i as u8,
            y, cb, cr,
            alpha: rgba.a,
        }
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let palette = vec![
            Rgba::new(0, 0, 0, 0),
            Rgba::new(255, 255, 255, 255),
        ];
        let entries = build_palette(&palette);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[0].alpha, 0);
        assert_eq!(entries[1].index, 1);
        assert_eq!(entries[1].y, 255);
        assert_eq!(entries[1].alpha, 255);
    }
}
