//! Renderer + compositing benchmarks, rebuilt from the v2.7.0 blueprint
//! (`git show 376593a^:crates/subtitle-renderer/benches/renderer.rs`) and
//! adapted to the current API.
//!
//! API changes since v2.7.0: `Shaper`/`font_manager()` are gone (replaced by
//! `font::SimpleShaper` — see `font_subsystem.rs`), and `Renderer` is driven
//! directly via `render_ass`. The v2.7.0 text_shape/blur/shadow/transform
//! benches were not rebuilt: text shaping moved to `font_subsystem.rs`, and
//! blur/shadow/transform are out of scope for the minimal rebuild.

use std::hint::black_box;

use ass_core::SubtitleDocument;
use criterion::{BatchSize, Criterion, criterion_group, criterion_main};
use subtitle_renderer::effects::composite_over;
use subtitle_renderer::{RenderConfig, Renderer};

const SIMPLE_ASS: &str = "\
[Script Info]
Title: Bench
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World
";

/// End-to-end: render one 1920×1080 frame with a single "Hello World" event.
fn bench_render_simple(c: &mut Criterion) {
    let Ok(doc) = SubtitleDocument::parse(SIMPLE_ASS) else {
        eprintln!("SKIP: simple ASS failed to parse");
        return;
    };
    let renderer = Renderer::new(RenderConfig::default());
    if renderer.load_system_fonts() == 0 {
        eprintln!("SKIP: no system fonts (install fonts-dejavu-core)");
        return;
    }
    c.bench_function("render_simple_1920x1080", |b| {
        b.iter(|| {
            black_box(renderer.render_ass(black_box(&doc), 2000));
        })
    });
}

/// SIMD Porter-Duff `composite_over` on semi-transparent src over opaque dst.
fn bench_composite(c: &mut Criterion) {
    let mut group = c.benchmark_group("composite_over");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(20);

    for (w, h, label) in [(64u32, 32u32, "64x32"), (320u32, 180u32, "320x180")] {
        let mut src = vec![0u8; (w * h * 4) as usize];
        for i in 0..(w * h) as usize {
            let idx = i * 4;
            src[idx] = 255;
            src[idx + 1] = 128;
            src[idx + 2] = 64;
            src[idx + 3] = 180;
        }
        let dst = vec![32u8; (w * h * 4) as usize];

        group.bench_function(label, |b| {
            b.iter_batched(
                || dst.clone(),
                |mut d| composite_over(&mut d, black_box(&src), w, h),
                BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

criterion_group!(benches, bench_render_simple, bench_composite);
criterion_main!(benches);
