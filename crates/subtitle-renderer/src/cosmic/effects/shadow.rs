use crate::cosmic::effects::blur::apply_gaussian_blur;
use crate::cosmic::effects::composite::composite_over;
use tiny_skia::Pixmap;

/// Render a drop shadow behind subtitle text.
pub fn apply_shadow(
    src_data: &[u8],
    w: u32,
    h: u32,
    offset_x: f32,
    offset_y: f32,
    blur_radius: f32,
    shadow_color: [u8; 4],
) -> Vec<u8> {
    let wu = w as usize;
    let hu = h as usize;
    let ox = offset_x.round() as i32;
    let oy = offset_y.round() as i32;
    let mut shadow = vec![0u8; wu * hu * 4];

    // Tint non-transparent src pixels with shadow_color at reduced alpha
    let sa = shadow_color[3] as u32;
    for y in 0..hu {
        for x in 0..wu {
            let si = (y * wu + x) * 4;
            let src_a = src_data[si + 3] as u32;
            if src_a == 0 {
                continue;
            }
            let sx = x as i32 + ox;
            let sy = y as i32 + oy;
            if sx < 0 || sy < 0 || sx >= wu as i32 || sy >= hu as i32 {
                continue;
            }
            let di = (sy as usize * wu + sx as usize) * 4;
            let alpha = (src_a * sa / 255) as u8;
            shadow[di] = shadow_color[0];
            shadow[di + 1] = shadow_color[1];
            shadow[di + 2] = shadow_color[2];
            shadow[di + 3] = alpha;
        }
    }

    // Apply blur to shadow
    if blur_radius > 0.0 {
        let size = match tiny_skia::IntSize::from_wh(w, h) {
            Some(s) => s,
            None => return src_data.to_vec(),
        };
        let mut sp = match Pixmap::from_vec(shadow, size) {
            Some(p) => p,
            None => return src_data.to_vec(),
        };
        apply_gaussian_blur(&mut sp, blur_radius);
        shadow = sp.take();
    }

    // Composite shadow under original
    let mut result = shadow;
    composite_over(&mut result, src_data, w, h);
    result
}
