use crate::quantize::nearest::find_nearest_weighted;

/// Check if all pixels in `pixels` can map to `palette` within `threshold`.
pub fn all_mappable(pixels: &[[u8; 4]], palette: &[[u8; 4]], threshold: f32) -> bool {
    if palette.is_empty() {
        return false;
    }
    let threshold_sq = (threshold * threshold) as u64;
    pixels.iter().all(|p| {
        let idx = find_nearest_weighted(p, palette) as usize;
        if idx < palette.len() {
            let dr = i64::from(p[0]) - i64::from(palette[idx][0]);
            let dg = i64::from(p[1]) - i64::from(palette[idx][1]);
            let db = i64::from(p[2]) - i64::from(palette[idx][2]);
            let da = i64::from(p[3]) - i64::from(palette[idx][3]);
            let d_sq = (dr * dr * 3 + dg * dg * 4 + db * db * 2 + da * da) as u64;
            d_sq <= threshold_sq
        } else {
            false
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_mappable_identical() {
        let pixels = [[100, 100, 100, 255]];
        let palette = vec![[100, 100, 100, 255]];
        assert!(all_mappable(&pixels, &palette, 10.0));
    }

    #[test]
    fn all_mappable_empty_palette() {
        assert!(!all_mappable(&[[0, 0, 0, 255]], &[], 10.0));
    }
}
