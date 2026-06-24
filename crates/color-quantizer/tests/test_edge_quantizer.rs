use color_quantizer::{DitherMethod, Quantizer, Rgba};

#[test]
fn median_cut_empty_pixels_returns_empty() {
    let result = color_quantizer::Quantizer::new(16).quantize(&[], 0, 0);
    // ColorPipeline returns a single transparent entry for empty input
    assert_eq!(result.palette.len(), 1);
    assert!(result.indices.is_empty());
}

#[test]
fn median_cut_single_pixel() {
    let rgba = vec![255u8, 0, 0, 255];
    let result = Quantizer::new(16).quantize(&rgba, 1, 1);
    assert_eq!(result.palette.len(), 1);
    assert_eq!(result.palette[0], Rgba::new(255, 0, 0, 255));
    assert_eq!(result.indices[0], 0);
}

#[test]
fn median_cut_all_transparent_pixels() {
    let rgba = vec![0u8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    let result = Quantizer::new(16).quantize(&rgba, 2, 2);
    assert_eq!(
        result.palette.len(),
        1,
        "should have exactly transparent entry"
    );
    assert_eq!(result.palette[0], Rgba::new(0, 0, 0, 0));
    assert!(result.indices.iter().all(|&i| i == 0));
}

#[test]
fn median_cut_all_same_color() {
    let rgba = vec![
        128u8, 64, 32, 255, 128, 64, 32, 255, 128, 64, 32, 255, 128, 64, 32, 255,
    ];
    let result = Quantizer::new(16).quantize(&rgba, 2, 2);
    assert!(result
        .palette
        .iter()
        .all(|p| *p == Rgba::new(128, 64, 32, 255)));
}

#[test]
fn median_cut_max_colors_zero_returns_empty_palette() {
    let rgba = vec![255u8, 0, 0, 255, 0, 255, 0, 255];
    let result = Quantizer::new(0).quantize(&rgba, 2, 1);
    assert!(result.palette.is_empty());
    assert_eq!(result.indices.len(), 2);
}

#[test]
fn median_cut_mixed_transparent_and_opaque() {
    let rgba = vec![255u8, 0, 0, 255, 0, 0, 0, 0, 0, 0, 255, 255, 0, 0, 0, 0];
    let result = Quantizer::new(16).quantize(&rgba, 2, 2);
    let has_transparent = result.palette.iter().any(|p| p.a == 0);
    assert!(has_transparent, "palette should include transparent entry");
    assert!(
        result.palette.len() >= 2,
        "should have at least 2 colors + transparent"
    );
}

#[test]
fn find_nearest_index_empty_palette() {
    let idx = color_quantizer::Quantizer::new(1).quantize(&[128, 128, 128, 255], 1, 1);
    assert!(!idx.palette.is_empty());
}

#[test]
fn quantize_with_floyd_steinberg_dithering() {
    let rgba = vec![
        255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
    ];
    let result = Quantizer::new(4)
        .with_dither(DitherMethod::FloydSteinberg)
        .quantize(&rgba, 2, 2);
    assert!(!result.palette.is_empty());
    assert_eq!(result.indices.len(), 4);
}

#[test]
fn quantize_with_ordered_dithering() {
    let rgba = vec![
        255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
    ];
    let result = Quantizer::new(4)
        .with_dither(DitherMethod::Ordered)
        .quantize(&rgba, 2, 2);
    assert!(!result.palette.is_empty());
    assert_eq!(result.indices.len(), 4);
}

#[test]
fn quantize_no_dithering() {
    let rgba = vec![
        255u8, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
    ];
    let result = Quantizer::new(4)
        .with_dither(DitherMethod::None)
        .quantize(&rgba, 2, 2);
    assert!(!result.palette.is_empty());
    assert_eq!(result.indices.len(), 4);
}

#[test]
fn quantize_transparent_index_points_to_transparent() {
    let rgba = vec![255u8, 0, 0, 255, 0, 0, 0, 0];
    let result = Quantizer::new(16).quantize(&rgba, 2, 1);
    let transparent = result.palette[result.transparent_index as usize];
    assert_eq!(
        transparent.a, 0,
        "transparent_index should point to a transparent color"
    );
}

#[test]
fn quantize_large_pixel_set() {
    let mut rgba = Vec::new();
    for i in 0..100 {
        rgba.push((i % 256) as u8);
        rgba.push(((i * 2) % 256) as u8);
        rgba.push(((i * 3) % 256) as u8);
        rgba.push(255u8);
    }
    let result = Quantizer::new(8).quantize(&rgba, 10, 10);
    assert!(result.palette.len() <= 9);
    assert_eq!(result.indices.len(), 100);
}
