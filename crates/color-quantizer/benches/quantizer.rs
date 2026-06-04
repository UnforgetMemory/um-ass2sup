use color_quantizer::{DitherMethod, Quantizer};
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn make_test_image(width: u32, height: u32, color_count: usize) -> Vec<u8> {
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            let idx = ((x + y) % color_count as u32) as usize;
            let r = ((idx * 37) % 256) as u8;
            let g = ((idx * 73) % 256) as u8;
            let b = ((idx * 113) % 256) as u8;
            let a = if idx == 0 { 0 } else { 255 };
            rgba.extend_from_slice(&[r, g, b, a]);
        }
    }
    rgba
}

fn bench_quantizer_small(c: &mut Criterion) {
    let rgba = make_test_image(64, 32, 16);
    let quantizer = Quantizer::new(16);
    c.bench_function("quantizer_small_64x32", |b| {
        b.iter(|| {
            let _ = black_box(quantizer.quantize(black_box(&rgba), 64, 32));
        });
    });
}

fn bench_quantizer_medium(c: &mut Criterion) {
    let rgba = make_test_image(320, 180, 64);
    let quantizer = Quantizer::new(128);
    c.bench_function("quantizer_medium_320x180", |b| {
        b.iter(|| {
            let _ = black_box(quantizer.quantize(black_box(&rgba), 320, 180));
        });
    });
}

fn bench_quantizer_large(c: &mut Criterion) {
    let rgba = make_test_image(1920, 1080, 256);
    let quantizer = Quantizer::new(255);
    c.bench_function("quantizer_large_1920x1080", |b| {
        b.iter(|| {
            let _ = black_box(quantizer.quantize(black_box(&rgba), 1920, 1080));
        });
    });
}

fn bench_quantizer_no_dither(c: &mut Criterion) {
    let rgba = make_test_image(320, 180, 64);
    let quantizer = Quantizer::new(128).with_dither(DitherMethod::None);
    c.bench_function("quantizer_no_dither_320x180", |b| {
        b.iter(|| {
            let _ = black_box(quantizer.quantize(black_box(&rgba), 320, 180));
        });
    });
}

fn bench_quantizer_ordered_dither(c: &mut Criterion) {
    let rgba = make_test_image(320, 180, 64);
    let quantizer = Quantizer::new(128).with_dither(DitherMethod::Ordered);
    c.bench_function("quantizer_ordered_dither_320x180", |b| {
        b.iter(|| {
            let _ = black_box(quantizer.quantize(black_box(&rgba), 320, 180));
        });
    });
}

criterion_group!(
    benches,
    bench_quantizer_small,
    bench_quantizer_medium,
    bench_quantizer_large,
    bench_quantizer_no_dither,
    bench_quantizer_ordered_dither,
);
criterion_main!(benches);
