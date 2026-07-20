use color_quantizer::{QuantizedFrame, Rgba};
use criterion::{criterion_group, criterion_main, Criterion};
use pgs_encoder::domain::rle::rle_encode;
use std::hint::black_box;

fn make_test_frame(width: u32, height: u32, color_count: usize) -> QuantizedFrame {
    let mut palette = Vec::new();
    for i in 0..color_count.min(255) {
        palette.push(Rgba::new(
            ((i * 37) % 256) as u8,
            ((i * 73) % 256) as u8,
            ((i * 113) % 256) as u8,
            255,
        ));
    }
    palette.push(Rgba::new(0, 0, 0, 0));

    let mut indices = Vec::with_capacity((width * height) as usize);
    for y in 0..height {
        for x in 0..width {
            if x == 0 && y == 0 {
                indices.push((palette.len() - 1) as u8);
            } else {
                indices.push(((x + y) % color_count as u32) as u8);
            }
        }
    }

    QuantizedFrame {
        width,
        height,
        transparent_index: (palette.len() - 1) as u8,
        palette,
        indices,
        x: 0,
        y: 0,
        color_space: Default::default(),
        pts_ms: 0,
        duration_ms: 0,
    }
}

fn bench_rle_small(c: &mut Criterion) {
    let frame = make_test_frame(64, 32, 16);
    c.bench_function("rle_small_64x32", |b| {
        b.iter(|| {
            let _ = black_box(rle_encode(
                black_box(&frame.indices),
                64,
                32,
                frame.transparent_index,
            ));
        });
    });
}

fn bench_rle_medium(c: &mut Criterion) {
    let frame = make_test_frame(320, 180, 64);
    c.bench_function("rle_medium_320x180", |b| {
        b.iter(|| {
            let _ = black_box(rle_encode(
                black_box(&frame.indices),
                320,
                180,
                frame.transparent_index,
            ));
        });
    });
}

fn bench_rle_large(c: &mut Criterion) {
    let frame = make_test_frame(1920, 1080, 256);
    c.bench_function("rle_large_1920x1080", |b| {
        b.iter(|| {
            let _ = black_box(rle_encode(
                black_box(&frame.indices),
                1920,
                1080,
                frame.transparent_index,
            ));
        });
    });
}

fn bench_pgs_encode_small(c: &mut Criterion) {
    let frame = make_test_frame(64, 32, 16);
    c.bench_function("pgs_encode_small_64x32", |b| {
        b.iter(|| {
            let mut enc = pgs_encoder::PgsEncoder::new(1920, 1080, 24.0);
            let _ = black_box(enc.encode_frame(black_box(&frame), 1000, 2000));
        });
    });
}

fn bench_pgs_encode_medium(c: &mut Criterion) {
    let frame = make_test_frame(320, 180, 64);
    c.bench_function("pgs_encode_medium_320x180", |b| {
        b.iter(|| {
            let mut enc = pgs_encoder::PgsEncoder::new(1920, 1080, 24.0);
            let _ = black_box(enc.encode_frame(black_box(&frame), 1000, 2000));
        });
    });
}

fn bench_pgs_encode_ntsc(c: &mut Criterion) {
    let frame = make_test_frame(320, 180, 64);
    c.bench_function("pgs_encode_ntsc_320x180", |b| {
        b.iter(|| {
            let mut enc = pgs_encoder::PgsEncoder::new(1920, 1080, 23.976);
            let _ = black_box(enc.encode_frame(black_box(&frame), 1000, 2000));
        });
    });
}

criterion_group!(
    benches,
    bench_rle_small,
    bench_rle_medium,
    bench_rle_large,
    bench_pgs_encode_small,
    bench_pgs_encode_medium,
    bench_pgs_encode_ntsc,
);
criterion_main!(benches);
