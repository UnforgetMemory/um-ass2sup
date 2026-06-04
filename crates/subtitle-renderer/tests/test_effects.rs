use subtitle_renderer::{apply_gaussian_blur, apply_shadow, composite_over};
use tiny_skia::Pixmap;

fn make_pixmap(w: u32, h: u32, fill: [u8; 4]) -> Pixmap {
    let mut pm = Pixmap::new(w, h).unwrap();
    let data = pm.data_mut();
    for i in 0..(w * h) as usize {
        let idx = i * 4;
        data[idx] = fill[0];
        data[idx + 1] = fill[1];
        data[idx + 2] = fill[2];
        data[idx + 3] = fill[3];
    }
    pm
}

fn set_pixel(pm: &mut Pixmap, x: u32, y: u32, color: [u8; 4]) {
    let w = pm.width();
    let idx = ((y * w + x) * 4) as usize;
    let data = pm.data_mut();
    data[idx] = color[0];
    data[idx + 1] = color[1];
    data[idx + 2] = color[2];
    data[idx + 3] = color[3];
}

fn get_pixel(pm: &Pixmap, x: u32, y: u32) -> [u8; 4] {
    let w = pm.width();
    let idx = ((y * w + x) * 4) as usize;
    let data = pm.data();
    [data[idx], data[idx + 1], data[idx + 2], data[idx + 3]]
}

#[test]
fn test_blur_radius_zero_does_nothing() {
    let mut pm = make_pixmap(10, 10, [0, 0, 0, 0]);
    set_pixel(&mut pm, 5, 5, [255, 255, 255, 255]);
    let before = pm.data().to_vec();
    apply_gaussian_blur(&mut pm, 0.0);
    assert_eq!(pm.data(), before.as_slice());
}

#[test]
fn test_blur_negative_radius_does_nothing() {
    let mut pm = make_pixmap(10, 10, [0, 0, 0, 0]);
    set_pixel(&mut pm, 5, 5, [255, 255, 255, 255]);
    let before = pm.data().to_vec();
    apply_gaussian_blur(&mut pm, -1.0);
    assert_eq!(pm.data(), before.as_slice());
}

#[test]
fn test_blur_radius_one_spreads_pixel() {
    let mut pm = make_pixmap(10, 10, [0, 0, 0, 0]);
    set_pixel(&mut pm, 5, 5, [255, 255, 255, 255]);
    apply_gaussian_blur(&mut pm, 1.0);
    let center = get_pixel(&pm, 5, 5);
    assert!(center[3] > 0, "center pixel should still have alpha");
    let neighbor = get_pixel(&pm, 6, 5);
    assert!(neighbor[3] > 0, "neighbor pixel should receive blur spread");
    let far = get_pixel(&pm, 0, 0);
    assert_eq!(far[3], 0, "distant pixel should remain transparent");
}

#[test]
fn test_blur_radius_five_spreads_wider() {
    let mut pm = make_pixmap(20, 20, [0, 0, 0, 0]);
    set_pixel(&mut pm, 10, 10, [255, 255, 255, 255]);
    apply_gaussian_blur(&mut pm, 5.0);
    let close = get_pixel(&pm, 12, 10);
    let mid = get_pixel(&pm, 8, 10);
    assert!(close[3] > 0, "close pixel should have blur");
    assert!(mid[3] > 0, "mid-range pixel should have blur");
}

#[test]
fn test_blur_preserves_opaque_area() {
    let mut pm = make_pixmap(10, 10, [128, 128, 128, 255]);
    apply_gaussian_blur(&mut pm, 1.0);
    let center = get_pixel(&pm, 5, 5);
    assert_eq!(center[3], 255, "fully opaque area should stay opaque");
    assert_eq!(center[0], 128);
}

#[test]
fn test_shadow_basic_offset() {
    let src = vec![0u8; 10 * 10 * 4];
    let mut src_mut = src.clone();
    let w = 10u32;
    let h = 10u32;
    let idx = ((5 * w + 5) * 4) as usize;
    src_mut[idx] = 255;
    src_mut[idx + 1] = 255;
    src_mut[idx + 2] = 255;
    src_mut[idx + 3] = 255;
    let shadow_color = [0, 0, 0, 200];
    let result = apply_shadow(&src_mut, w, h, 2.0, 2.0, 0.0, shadow_color);
    let dst_idx = ((7 * w + 7) * 4) as usize;
    assert!(
        result[dst_idx + 3] > 0,
        "shadow should appear at offset position"
    );
    assert_eq!(result[dst_idx], 0);
    assert_eq!(result[dst_idx + 1], 0);
    assert_eq!(result[dst_idx + 2], 0);
}

#[test]
fn test_shadow_zero_offset() {
    let src = vec![0u8; 10 * 10 * 4];
    let mut src_mut = src.clone();
    let w = 10u32;
    let h = 10u32;
    let idx = ((5 * w + 5) * 4) as usize;
    src_mut[idx] = 255;
    src_mut[idx + 1] = 255;
    src_mut[idx + 2] = 255;
    src_mut[idx + 3] = 255;
    let shadow_color = [0, 0, 0, 128];
    let result = apply_shadow(&src_mut, w, h, 0.0, 0.0, 0.0, shadow_color);
    let dst_idx = ((5 * w + 5) * 4) as usize;
    assert_eq!(
        result[dst_idx + 3],
        128,
        "shadow at zero offset should use shadow alpha"
    );
}

#[test]
fn test_shadow_empty_source() {
    let src = vec![0u8; 10 * 10 * 4];
    let shadow_color = [0, 0, 0, 255];
    let result = apply_shadow(&src, 10, 10, 3.0, 3.0, 0.0, shadow_color);
    assert!(
        result.iter().all(|&b| b == 0),
        "shadow of empty source should be empty"
    );
}

#[test]
fn test_shadow_proportional_alpha() {
    let w = 10u32;
    let h = 10u32;
    let mut src = vec![0u8; (w * h * 4) as usize];
    let idx = ((5 * w + 5) * 4) as usize;
    src[idx] = 255;
    src[idx + 1] = 255;
    src[idx + 2] = 255;
    src[idx + 3] = 128;
    let shadow_color = [0, 0, 0, 255];
    let result = apply_shadow(&src, w, h, 1.0, 1.0, 0.0, shadow_color);
    let dst_idx = ((6 * w + 6) * 4) as usize;
    let expected_alpha = (255u32 * 128) / 255;
    assert_eq!(result[dst_idx + 3], expected_alpha as u8);
}

#[test]
fn test_composite_over_opaque_on_transparent() {
    let mut dst = vec![0u8; 10 * 10 * 4];
    let mut src = vec![0u8; 10 * 10 * 4];
    src[0] = 255;
    src[1] = 0;
    src[2] = 0;
    src[3] = 255;
    composite_over(&mut dst, &src, 10, 10);
    assert_eq!(dst[0], 255);
    assert_eq!(dst[1], 0);
    assert_eq!(dst[2], 0);
    assert_eq!(dst[3], 255);
}

#[test]
fn test_composite_over_transparent_on_opaque() {
    let mut dst = vec![0u8; 10 * 10 * 4];
    dst[0] = 128;
    dst[1] = 128;
    dst[2] = 128;
    dst[3] = 255;
    let src = vec![0u8; 10 * 10 * 4];
    composite_over(&mut dst, &src, 10, 10);
    assert_eq!(dst[0], 128);
    assert_eq!(dst[1], 128);
    assert_eq!(dst[2], 128);
    assert_eq!(dst[3], 255);
}

#[test]
fn test_composite_over_semi_transparent() {
    let mut dst = vec![0u8; 4];
    dst[0] = 0;
    dst[1] = 0;
    dst[2] = 0;
    dst[3] = 255;
    let mut src = vec![0u8; 4];
    src[0] = 255;
    src[1] = 255;
    src[2] = 255;
    src[3] = 128;
    composite_over(&mut dst, &src, 1, 1);
    assert!(
        dst[0] > 0,
        "semi-transparent white on black should lighten pixel"
    );
    assert!(dst[0] < 200, "semi-transparent should not fully replace");
    assert_eq!(dst[3], 255, "alpha should remain 255");
}

#[test]
fn test_composite_over_both_transparent() {
    let mut dst = vec![0u8; 4];
    let src = vec![0u8; 4];
    composite_over(&mut dst, &src, 1, 1);
    assert_eq!(dst, [0, 0, 0, 0]);
}

#[test]
fn test_composite_over_simd_batch_4px() {
    // Exercise the SIMD path: exactly 4 pixels (one u32x4 batch)
    let mut dst = vec![
        0, 0, 0, 0, // pixel 0: transparent black
        0, 0, 0, 255, // pixel 1: opaque black
        100, 150, 200, 255, // pixel 2: opaque gray-blue
        255, 0, 0, 128, // pixel 3: semi-transparent red
    ];
    let src = vec![
        255, 0, 0, 255, // pixel 0: opaque red
        255, 255, 255, 128, // pixel 1: semi-transparent white
        0, 0, 0, 0, // pixel 2: fully transparent
        0, 0, 255, 255, // pixel 3: opaque blue
    ];
    composite_over(&mut dst, &src, 4, 1);

    // Pixel 0: opaque red over transparent → src replaces
    assert_eq!(dst[0], 255, "pixel 0 R");
    assert_eq!(dst[1], 0, "pixel 0 G");
    assert_eq!(dst[2], 0, "pixel 0 B");
    assert_eq!(dst[3], 255, "pixel 0 A");

    // Pixel 1: semi-transparent white (a=128) over opaque black
    // out_a = 128 + 255*(255-128)/255 = 128 + 127 = 255
    // out_r = (255*128 + 0) / 255 = 128
    assert_eq!(dst[4], 128, "pixel 1 R");
    assert_eq!(dst[5], 128, "pixel 1 G");
    assert_eq!(dst[6], 128, "pixel 1 B");
    assert_eq!(dst[7], 255, "pixel 1 A");

    // Pixel 2: transparent over opaque → dst unchanged
    assert_eq!(dst[8], 100, "pixel 2 R");
    assert_eq!(dst[9], 150, "pixel 2 G");
    assert_eq!(dst[10], 200, "pixel 2 B");
    assert_eq!(dst[11], 255, "pixel 2 A");

    // Pixel 3: opaque blue over semi-transparent red
    // out_a = 255 + 128*0/255 = 255
    // out_b = (255*255 + 0) / 255 = 255
    assert_eq!(dst[12], 0, "pixel 3 R");
    assert_eq!(dst[13], 0, "pixel 3 G");
    assert_eq!(dst[14], 255, "pixel 3 B");
    assert_eq!(dst[15], 255, "pixel 3 A");
}

#[test]
fn test_composite_over_simd_with_remainder() {
    // 2 pixels: SIMD batch handles first 0 of 4, scalar handles remaining 2
    let mut dst = vec![
        0, 0, 0, 255, // pixel 0: opaque black
        128, 128, 128, 255, // pixel 1: opaque gray
    ];
    let src = vec![
        0, 0, 0, 0, // pixel 0: fully transparent
        255, 0, 0, 200, // pixel 1: semi-transparent red
    ];
    composite_over(&mut dst, &src, 2, 1);

    // Pixel 0: transparent over opaque → dst unchanged
    assert_eq!(dst[0], 0, "pixel 0 R unchanged");
    assert_eq!(dst[3], 255, "pixel 0 A unchanged");

    // Pixel 1: semi-transparent red (a=200) over opaque gray
    // out_a = 200 + 255*55/255 = 200 + 55 = 255
    // out_r = (255*200 + 128*255*55/255) / 255 = (51000 + 7040) / 255 = 227
    // out_g = (0*200 + 128*255*55/255) / 255 = 7040 / 255 = 27
    assert_eq!(dst[4], 227, "pixel 1 R");
    assert_eq!(dst[5], 27, "pixel 1 G");
    assert_eq!(dst[6], 27, "pixel 1 B");
    assert_eq!(dst[7], 255, "pixel 1 A");
}

#[test]
fn test_composite_over_multiple_pixels() {
    let w = 3u32;
    let h = 1u32;
    let mut dst = vec![0u8; (w * h * 4) as usize];
    let mut src = vec![0u8; (w * h * 4) as usize];
    src[3] = 255;
    src[4] = 255;
    src[7] = 128;
    composite_over(&mut dst, &src, w, h);
    assert_eq!(dst[3], 255, "first pixel fully applied");
    assert_eq!(dst[7], 128, "second pixel semi-applied");
    assert_eq!(dst[11], 0, "third pixel untouched");
}
