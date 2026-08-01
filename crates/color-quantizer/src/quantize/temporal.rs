use crate::quantize::nearest::find_nearest_weighted_kd;

/// Check if all pixels in an RGBA byte buffer can map to `palette` within
/// `threshold`.
///
/// Uses the cached weighted k-d tree (exact parity with a linear weighted
/// scan) so this gate is O(pixels · log palette) instead of O(pixels · palette),
/// and iterates the raw RGBA bytes directly — no intermediate `[[u8; 4]]` copy.
pub fn all_mappable(rgba: &[u8], palette: &[[u8; 4]], threshold: f32) -> bool {
    if palette.is_empty() {
        return false;
    }
    let threshold_sq = (threshold * threshold) as u64;
    rgba.chunks_exact(4).all(|c| {
        let p = [c[0], c[1], c[2], c[3]];
        let idx = find_nearest_weighted_kd(&p, palette) as usize;
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
        let rgba = [100u8, 100, 100, 255];
        let palette = vec![[100, 100, 100, 255]];
        assert!(all_mappable(&rgba, &palette, 10.0));
    }

    #[test]
    fn all_mappable_empty_palette() {
        assert!(!all_mappable(&[0, 0, 0, 255], &[], 10.0));
    }

    #[test]
    fn all_mappable_bytes_equivalent_to_pixels() {
        // Same decision whether fed raw bytes or pre-collected pixels.
        let pixels: Vec<[u8; 4]> = vec![[100, 100, 100, 255], [200, 50, 50, 255], [0, 0, 0, 0]];
        let mut rgba = Vec::with_capacity(pixels.len() * 4);
        for p in &pixels {
            rgba.extend_from_slice(p);
        }
        let palette = vec![[100, 100, 100, 255], [200, 50, 50, 255], [0, 0, 0, 0]];
        assert!(all_mappable(&rgba, &palette, 10.0));
        // A far-away colour fails the tight threshold.
        let mut far = rgba.clone();
        far[..4].copy_from_slice(&[255, 0, 0, 255]);
        assert!(!all_mappable(&far, &palette, 1.0));
    }
}
