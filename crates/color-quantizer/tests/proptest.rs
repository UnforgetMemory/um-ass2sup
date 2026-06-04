use color_quantizer::{DitherMethod, Quantizer};
use proptest::prelude::*;

// ============================================================
// Property: Palette size never exceeds max_colors + 1
// ============================================================
proptest! {
    #![proptest_config(ProptestConfig::with_cases(500))]

    #[test]
    fn palette_size_within_limit(
        (width, height) in (1u32..=64, 1u32..=64),
    ) {
        let total = (width * height * 4) as usize;
        let mut rgba = Vec::with_capacity(total);
        for y in 0..height {
            for x in 0..width {
                rgba.push((x * 7) as u8);
                rgba.push((y * 13) as u8);
                rgba.push(((x + y) * 5) as u8);
                rgba.push(255u8);
            }
        }

        for &max_colors in &[16usize, 32, 64, 128, 255] {
            let q = Quantizer::new(max_colors).with_dither(DitherMethod::None);
            let frame = q.quantize(&rgba, width, height);
            assert!(
                frame.palette_size() <= max_colors + 1,
                "palette_size {} > max_colors {} + 1",
                frame.palette_size(),
                max_colors,
            );
        }
    }
}

// ============================================================
// Property: Output frame dimensions match input
// ============================================================
proptest! {
    #[test]
    fn output_dimensions_match(
        (width, height) in (1u32..=64, 1u32..=64),
    ) {
        let total = (width * height * 4) as usize;
        let rgba = vec![0u8; total];

        let q = Quantizer::default();
        let frame = q.quantize(&rgba, width, height);
        assert_eq!(frame.width, width);
        assert_eq!(frame.height, height);
    }
}

// ============================================================
// Property: Index buffer has correct length
// ============================================================
proptest! {
    #[test]
    fn indices_have_correct_length(
        (width, height) in (1u32..=32, 1u32..=32),
    ) {
        let total = (width * height * 4) as usize;
        let rgba = vec![255u8; total];

        let q = Quantizer::new(128);
        let frame = q.quantize(&rgba, width, height);
        assert_eq!(frame.indices.len(), (width * height) as usize);
    }
}

// ============================================================
// Property: Transparent pixels map to a single transparent index
// ============================================================
proptest! {
    #[test]
    fn transparent_pixels_have_single_index(
        (width, height) in (1u32..=16, 1u32..=16),
    ) {
        let total = (width * height * 4) as usize;
        let rgba = vec![0u8; total];

        let q = Quantizer::new(64).with_dither(DitherMethod::None);
        let frame = q.quantize(&rgba, width, height);

        for &idx in &frame.indices {
            assert_eq!(idx, frame.transparent_index);
        }
    }
}

// ============================================================
// Non-proptest deterministic property tests
// ============================================================

#[test]
fn quantize_empty_input() {
    let q = Quantizer::new(255);
    let frame = q.quantize(&[], 0, 0);
    assert!(frame.palette.is_empty());
    assert_eq!(frame.indices.len(), 0);
}

#[test]
fn quantize_zero_max_colors() {
    let rgba = vec![255u8; 16];
    let q = Quantizer::new(0);
    let frame = q.quantize(&rgba, 1, 4);
    assert!(frame.palette.is_empty());
    assert_eq!(frame.indices.len(), 4);
}

#[test]
fn all_dither_methods_produce_valid_output() {
    let width = 16;
    let height = 16;
    let total = (width * height * 4) as usize;
    let mut rgba = Vec::with_capacity(total);
    for y in 0..height {
        for x in 0..width {
            rgba.push((x * 16) as u8);
            rgba.push((y * 16) as u8);
            rgba.push(((x + y) * 8) as u8);
            rgba.push(255u8);
        }
    }

    for dither in &[
        DitherMethod::None,
        DitherMethod::FloydSteinberg,
        DitherMethod::Ordered,
    ] {
        let q = Quantizer::new(128).with_dither(*dither);
        let frame = q.quantize(&rgba, width, height);
        assert!(frame.palette_size() <= 129);
        assert_eq!(frame.indices.len(), (width * height) as usize);
    }
}
