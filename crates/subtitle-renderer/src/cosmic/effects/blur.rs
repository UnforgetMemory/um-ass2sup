use tiny_skia::Pixmap;

/// Apply an approximated box blur to a pixmap in-place.
#[allow(clippy::manual_checked_ops)]
pub fn apply_gaussian_blur(pixmap: &mut Pixmap, radius: f32) {
    if radius <= 0.0 || pixmap.width() < 3 || pixmap.height() < 3 {
        return;
    }
    let w = pixmap.width() as usize;
    let h = pixmap.height() as usize;
    let data = pixmap.data_mut();
    let r = radius.ceil() as usize;
    // Horizontal pass
    let mut row = vec![0u8; w * 4];
    for y in 0..h {
        let off = y * w * 4;
        for x in 0..w {
            let (mut ra, mut ga, mut ba, mut aa, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32);
            for kx in x.saturating_sub(r)..=(x + r).min(w - 1) {
                let pi = off + kx * 4;
                ra += data[pi] as u32;
                ga += data[pi + 1] as u32;
                ba += data[pi + 2] as u32;
                aa += data[pi + 3] as u32;
                n += 1;
            }
            if n > 0 {
                let di = x * 4;
                row[di] = (ra / n) as u8;
                row[di + 1] = (ga / n) as u8;
                row[di + 2] = (ba / n) as u8;
                row[di + 3] = (aa / n) as u8;
            }
        }
        data[off..off + w * 4].copy_from_slice(&row[..w * 4]);
    }
    // Vertical pass
    let mut col = vec![0u8; h * 4];
    for x in 0..w {
        for y in 0..h {
            let (mut ra, mut ga, mut ba, mut aa, mut n) = (0u32, 0u32, 0u32, 0u32, 0u32);
            for ky in y.saturating_sub(r)..=(y + r).min(h - 1) {
                let pi = (ky * w + x) * 4;
                ra += data[pi] as u32;
                ga += data[pi + 1] as u32;
                ba += data[pi + 2] as u32;
                aa += data[pi + 3] as u32;
                n += 1;
            }
            if n > 0 {
                col[y * 4] = (ra / n) as u8;
                col[y * 4 + 1] = (ga / n) as u8;
                col[y * 4 + 2] = (ba / n) as u8;
                col[y * 4 + 3] = (aa / n) as u8;
            }
        }
        for y in 0..h {
            let di = (y * w + x) * 4;
            data[di..di + 4].copy_from_slice(&col[y * 4..y * 4 + 4]);
        }
    }
}
