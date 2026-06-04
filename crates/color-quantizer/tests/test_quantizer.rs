use color_quantizer::{quantize, DitherMethod, QuantizedFrame, Quantizer, Rgba};

#[test]
fn test_rgba_new() {
    let c = Rgba::new(10, 20, 30, 40);
    assert_eq!(c.r, 10);
    assert_eq!(c.g, 20);
    assert_eq!(c.b, 30);
    assert_eq!(c.a, 40);
}

#[test]
fn test_rgba_distance_same() {
    let a = Rgba::new(100, 100, 100, 100);
    assert_eq!(a.distance_sq(&a), 0);
}

#[test]
fn test_rgba_distance_different() {
    let a = Rgba::new(0, 0, 0, 0);
    let b = Rgba::new(255, 255, 255, 255);
    let d = a.distance_sq(&b);
    assert_eq!(d, 255 * 255 * 4);
}

#[test]
fn test_rgba_copy_eq() {
    let a = Rgba::new(1, 2, 3, 4);
    let b = a;
    assert_eq!(a, b);
}

#[test]
fn test_quantized_frame_palette_size() {
    let frame = QuantizedFrame {
        width: 2,
        height: 2,
        palette: vec![Rgba::new(0, 0, 0, 0), Rgba::new(255, 255, 255, 255)],
        indices: vec![0, 1, 0, 1],
        transparent_index: 0,
    };
    assert_eq!(frame.palette_size(), 2);
}

#[test]
fn test_dither_method_default() {
    let d = DitherMethod::default();
    assert!(matches!(d, DitherMethod::FloydSteinberg));
}

#[test]
fn test_quantizer_new() {
    let q = Quantizer::new(128);
    let frame = q.quantize(&[0u8; 4], 1, 1);
    assert!(frame.palette_size() <= 128);
}

#[test]
fn test_quantizer_max_colors_capped() {
    let q = Quantizer::new(500);
    assert!(q.quantize(&[0u8; 4], 1, 1).palette_size() <= 255);
}

#[test]
fn test_quantizer_with_dither() {
    let q = Quantizer::new(255).with_dither(DitherMethod::None);
    let rgba = vec![128u8, 64, 32, 255];
    let frame = q.quantize(&rgba, 1, 1);
    assert!(!frame.palette.is_empty());
}

#[test]
fn test_quantize_single_pixel_opaque() {
    let rgba = vec![100u8, 150, 200, 255];
    let frame = quantize(&rgba, 1, 1);
    assert_eq!(frame.width, 1);
    assert_eq!(frame.height, 1);
    assert!(!frame.palette.is_empty());
    assert_eq!(frame.indices.len(), 1);
}

#[test]
fn test_quantize_single_pixel_transparent() {
    let rgba = vec![0u8, 0, 0, 0];
    let frame = quantize(&rgba, 1, 1);
    assert!(frame.palette.iter().any(|c| c.a == 0));
}

#[test]
fn test_quantize_2x2_uniform() {
    let rgba = vec![
        255u8, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 255,
    ];
    let frame = quantize(&rgba, 2, 2);
    assert_eq!(frame.indices.len(), 4);
    assert!(frame
        .palette
        .iter()
        .any(|c| c.r == 255 && c.g == 0 && c.b == 0));
}

#[test]
fn test_quantize_2x2_mixed() {
    let rgba = vec![
        255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
    ];
    let frame = quantize(&rgba, 2, 2);
    assert!(frame.palette_size() >= 2);
}

#[test]
fn test_quantize_ordered_dither() {
    let q = Quantizer::new(16).with_dither(DitherMethod::Ordered);
    let mut rgba = Vec::with_capacity(64);
    for _ in 0..16 {
        rgba.extend_from_slice(&[128u8, 128, 128, 255]);
    }
    let frame = q.quantize(&rgba, 4, 4);
    assert_eq!(frame.indices.len(), 16);
}

#[test]
fn test_quantize_floyd_steinberg() {
    let q = Quantizer::new(16).with_dither(DitherMethod::FloydSteinberg);
    let mut rgba = Vec::with_capacity(64);
    for _ in 0..16 {
        rgba.extend_from_slice(&[128u8, 128, 128, 255]);
    }
    let frame = q.quantize(&rgba, 4, 4);
    assert_eq!(frame.indices.len(), 16);
}

#[test]
fn test_quantize_transparent_index() {
    let rgba = vec![0u8, 0, 0, 0, 255, 255, 255, 255];
    let frame = quantize(&rgba, 2, 1);
    let ti = frame.transparent_index as usize;
    assert!(ti < frame.palette.len());
    assert_eq!(frame.palette[ti].a, 0);
}
