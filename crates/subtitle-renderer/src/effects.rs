use tiny_skia::Pixmap;

pub fn apply_gaussian_blur(pixmap: &mut Pixmap, radius: f32) {
    if radius <= 0.0 {
        return;
    }
    let r = radius.ceil() as u32;
    let w = pixmap.width();
    let h = pixmap.height();
    let data = pixmap.data_mut();

    let mut temp = vec![0u8; data.len()];

    for y in 0..h {
        for x in 0..w {
            let (mut sr, mut sg, mut sb, mut sa, mut count) = (0u32, 0u32, 0u32, 0u32, 0u32);
            for dx in -(r as i32)..=(r as i32) {
                let nx = x as i32 + dx;
                if nx >= 0 && nx < w as i32 {
                    let idx = (y * w + nx as u32) as usize * 4;
                    sr += data[idx] as u32;
                    sg += data[idx + 1] as u32;
                    sb += data[idx + 2] as u32;
                    sa += data[idx + 3] as u32;
                    count += 1;
                }
            }
            let idx = (y * w + x) as usize * 4;
            temp[idx] = (sr / count) as u8;
            temp[idx + 1] = (sg / count) as u8;
            temp[idx + 2] = (sb / count) as u8;
            temp[idx + 3] = (sa / count) as u8;
        }
    }

    data.copy_from_slice(&temp);

    for y in 0..h {
        for x in 0..w {
            let (mut sr, mut sg, mut sb, mut sa, mut count) = (0u32, 0u32, 0u32, 0u32, 0u32);
            for dy in -(r as i32)..=(r as i32) {
                let ny = y as i32 + dy;
                if ny >= 0 && ny < h as i32 {
                    let idx = (ny as u32 * w + x) as usize * 4;
                    sr += data[idx] as u32;
                    sg += data[idx + 1] as u32;
                    sb += data[idx + 2] as u32;
                    sa += data[idx + 3] as u32;
                    count += 1;
                }
            }
            let idx = (y * w + x) as usize * 4;
            data[idx] = (sr / count) as u8;
            data[idx + 1] = (sg / count) as u8;
            data[idx + 2] = (sb / count) as u8;
            data[idx + 3] = (sa / count) as u8;
        }
    }
}

pub fn apply_shadow(
    src: &[u8],
    width: u32,
    height: u32,
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    shadow_color: [u8; 4],
) -> Vec<u8> {
    let num_pixels = (width * height) as usize;

    // Step 1: Create shadow layer — replace non-transparent pixels with shadow_color
    let mut shadow_data = vec![0u8; num_pixels * 4];
    for i in 0..num_pixels {
        let idx = i * 4;
        let src_a = src[idx + 3] as u32;
        if src_a > 0 {
            shadow_data[idx] = shadow_color[0];
            shadow_data[idx + 1] = shadow_color[1];
            shadow_data[idx + 2] = shadow_color[2];
            shadow_data[idx + 3] = ((shadow_color[3] as u32 * src_a) / 255) as u8;
        }
    }

    // Step 2: Apply gaussian blur to the shadow layer
    if blur_radius > 0.0 {
        let mut shadow_pixmap = Pixmap::new(width, height).unwrap();
        shadow_pixmap
            .data_mut()
            .copy_from_slice(&shadow_data);
        apply_gaussian_blur(&mut shadow_pixmap, blur_radius);
        shadow_data = shadow_pixmap.data().to_vec();
    }

    // Step 3: Offset the shadow layer
    let mut result = vec![0u8; num_pixels * 4];
    let ox = offset_x.round() as i32;
    let oy = offset_y.round() as i32;

    for y in 0..height as i32 {
        for x in 0..width as i32 {
            let sx = x - ox;
            let sy = y - oy;
            if sx >= 0 && sx < width as i32 && sy >= 0 && sy < height as i32 {
                let src_idx = (sy as u32 * width + sx as u32) as usize * 4;
                let dst_idx = (y as u32 * width + x as u32) as usize * 4;
                result[dst_idx] = shadow_data[src_idx];
                result[dst_idx + 1] = shadow_data[src_idx + 1];
                result[dst_idx + 2] = shadow_data[src_idx + 2];
                result[dst_idx + 3] = shadow_data[src_idx + 3];
            }
        }
    }

    result
}

pub fn composite_over(dst: &mut [u8], src: &[u8], width: u32, height: u32) {
    assert_eq!(dst.len(), (width * height * 4) as usize);
    assert_eq!(src.len(), (width * height * 4) as usize);

    let n = (width * height) as usize;
    for i in 0..n {
        let idx = i * 4;
        let sa = src[idx + 3] as u32;
        if sa == 0 {
            continue;
        }
        let da = dst[idx + 3] as u32;
        let out_a = sa + da * (255 - sa) / 255;
        if out_a == 0 {
            continue;
        }
        for c in 0..3 {
            let sv = src[idx + c] as u32;
            let dv = dst[idx + c] as u32;
            dst[idx + c] = ((sv * sa + dv * da * (255 - sa) / 255) / out_a) as u8;
        }
        dst[idx + 3] = out_a as u8;
    }
}
