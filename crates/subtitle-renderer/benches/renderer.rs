use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ass_parser::{AssFile, Effect, Event, EventType, Timestamp};
use ass_parser::karaoke::{KaraokeSegment, KaraokeStyle};
use tiny_skia::Pixmap;
use subtitle_renderer::{
    apply_gaussian_blur, apply_shadow, composite_over,
    AffineTransform, RenderConfig, Renderer, Shaper,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Minimal ASS file with a single "Hello World" dialogue line.
fn simple_ass() -> AssFile {
    let content = r#"
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
"#;
    AssFile::parse(content).unwrap()
}

/// ASS file with multiple override tags (bold, italic, colors, border, shadow, scale).
fn complex_ass() -> AssFile {
    let content = r#"
[Script Info]
Title: Bench Complex
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\b1\i1\1c&H0000FF&\3c&H00FF00&\bord3\shad2\fscx120\fscy120\frz15}Complex Styled{\b0\i0} Text
Dialogue: 0,0:00:02.00,0:00:06.00,Default,,0,0,0,,{\pos(960,540)\fad(500,500)}Positioned and Faded
Dialogue: 0,0:00:03.00,0:00:07.00,Default,,0,0,0,,{\be5\4c&H000080&}Blur and Shadow{\xshad3\yshad3} Effect
"#;
    AssFile::parse(content).unwrap()
}

/// ASS file with karaoke segments (manually constructed since parser
/// doesn't populate karaoke_segments from override tags).
fn karaoke_ass() -> AssFile {
    let mut ass = simple_ass();
    ass.events.clear();
    ass.events.push(Event {
        event_type: EventType::Dialogue,
        layer: 0,
        start: Timestamp::from_ms(0),
        end: Timestamp::from_ms(10000),
        style_name: "Default".into(),
        name: String::new(),
        margin_l: 0,
        margin_r: 0,
        margin_v: 0,
        effect: Effect::None,
        text: "{\\kf1000}Hel{\\kf500}lo {\\kf800}Wor{\\kf600}ld".into(),
        override_tags: vec![],
        karaoke_segments: vec![
            KaraokeSegment::new(KaraokeStyle::Fill, 1000, "Hel".into(), 0),
            KaraokeSegment::new(KaraokeStyle::Fill, 500, "lo ".into(), 1),
            KaraokeSegment::new(KaraokeStyle::Fill, 800, "Wor".into(), 2),
            KaraokeSegment::new(KaraokeStyle::Fill, 600, "ld".into(), 3),
        ],
        raw_override_block: String::new(),
    });
    ass
}

/// Build an ASS file with `count` simultaneous dialogue events (same timestamp).
fn multi_event_ass(count: usize) -> AssFile {
    let mut content = String::from(
        "[Script Info]\n\
         Title: Event Scaling Bench\n\
         PlayResX: 1920\n\
         PlayResY: 1080\n\
         \n\
         [V4+ Styles]\n\
         Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, \
         OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, \
         ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, \
         Alignment, MarginL, MarginR, MarginV, Encoding\n\
         Style: Default,Arial,40,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,\
         0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1\n\
         \n\
         [Events]\n\
         Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n",
    );
    for i in 0..count {
        let layer = i % 10;
        content.push_str(&format!(
            "Dialogue: {layer},0:00:01.00,0:00:05.00,Default,,0,0,0,,Line {}: \
             The quick brown fox jumps over the lazy dog.\n",
            i + 1,
        ));
    }
    AssFile::parse(&content).expect("multi-event ASS parse")
}

/// Load an ASS fixture from the crate's test fixture directory.
fn load_fixture(name: &str) -> Option<AssFile> {
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures");
    let path = base.join(name);
    AssFile::parse_file(&path).ok()
}

// ---------------------------------------------------------------------------
// End-to-end pipeline benchmarks
// ---------------------------------------------------------------------------

fn bench_render_simple(c: &mut Criterion) {
    let ass = simple_ass();
    let config = RenderConfig::default();
    let renderer = Renderer::new(config);

    c.bench_function("render_simple_1920x1080", |b| {
        b.iter(|| {
            black_box(renderer.render_ass(black_box(&ass), 2000));
        });
    });
}

fn bench_render_complex(c: &mut Criterion) {
    let ass = complex_ass();
    let config = RenderConfig::default();
    let renderer = Renderer::new(config);

    c.bench_function("render_complex_override_tags", |b| {
        b.iter(|| {
            black_box(renderer.render_ass(black_box(&ass), 2000));
        });
    });
}

fn bench_render_karaoke(c: &mut Criterion) {
    let ass = karaoke_ass();
    let config = RenderConfig::default();
    let renderer = Renderer::new(config);

    c.bench_function("render_karaoke_4_syllables", |b| {
        b.iter(|| {
            // Render at 2500ms — second syllable is active
            black_box(renderer.render_ass(black_box(&ass), 2500));
        });
    });
}

fn bench_end_to_end_fixture(c: &mut Criterion) {
    let ass = match load_fixture("writing_mode.ass") {
        Some(a) => a,
        None => {
            eprintln!("SKIP: writing_mode.ass fixture not found");
            return;
        }
    };

    let config = RenderConfig {
        width: 1920,
        height: 1080,
        script_width: 1920,
        script_height: 1080,
        ..RenderConfig::default()
    };
    let renderer = Renderer::new(config);

    // Ten timestamps covering events at 1 s, 5 s, and 9 s (with gaps for misses).
    let timestamps: [u64; 10] = [1000, 1500, 3000, 4500, 5000, 5500, 7000, 9000, 9500, 11000];

    let mut group = c.benchmark_group("end_to_end");
    group.measurement_time(std::time::Duration::from_secs(10));
    group.sample_size(20);

    group.bench_function("render_10_frames_writing_mode", |b| {
        b.iter(|| {
            for &ts in &timestamps {
                black_box(renderer.render_ass(black_box(&ass), black_box(ts)));
            }
        });
    });

    group.finish();
}

// ---------------------------------------------------------------------------
// Per-component benchmarks
// ---------------------------------------------------------------------------

fn bench_text_shape(c: &mut Criterion) {
    let config = RenderConfig::default();
    let renderer = Renderer::new(config);
    let shaper = Shaper::new(renderer.font_manager());

    // Find a usable font
    let font_id = renderer
        .font_manager()
        .query_with_fallback("Arial", false, false)
        .expect("no font available");

    // 100 words of text
    let text = "The quick brown fox jumps over the lazy dog. \
        Pack my box with five dozen liquor jugs. \
        How vexingly quick daft zebras jump. \
        The five boxing wizards jump quickly. \
        Sphinx of black quartz judge my vow. \
        Two driven jocks help fax my big quiz. \
        The jay pig fox zebra my woes quack. \
        Five quacking zephyrs jolt my wax bed. \
        The quick brown fox jumps over the lazy dog.";

    c.bench_function("text_shape_100_words", |b| {
        b.iter(|| {
            let _ = black_box(shaper.shape(black_box(text), font_id, 48.0));
        });
    });
}

fn bench_blur(c: &mut Criterion) {
    // Create a 320×180 pixmap with some content
    let mut pixmap = Pixmap::new(320, 180).unwrap();
    let data = pixmap.data_mut();
    for y in 0..180u32 {
        for x in 0..320u32 {
            let idx = ((y * 320 + x) * 4) as usize;
            if (80..240).contains(&x) && (40..140).contains(&y) {
                data[idx] = 255;
                data[idx + 1] = 255;
                data[idx + 2] = 255;
                data[idx + 3] = 255;
            }
        }
    }

    let mut group = c.benchmark_group("apply_gaussian_blur");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(20);

    for radius in [1.0f32, 5.0, 10.0] {
        group.bench_function(format!("radius_{}", radius as u32), |b| {
            b.iter_batched(
                || pixmap.clone(),
                |mut pm| {
                    apply_gaussian_blur(&mut pm, black_box(radius));
                    black_box(pm.data()[0]);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

fn bench_transform(c: &mut Criterion) {
    let mut pixmap = Pixmap::new(320, 180).unwrap();
    let data = pixmap.data_mut();
    for y in 0..180u32 {
        for x in 0..320u32 {
            let idx = ((y * 320 + x) * 4) as usize;
            if (80..240).contains(&x) && (40..140).contains(&y) {
                data[idx] = 255;
                data[idx + 1] = 255;
                data[idx + 2] = 255;
                data[idx + 3] = 255;
            }
        }
    }
    let src = pixmap.data().to_vec();

    let mut group = c.benchmark_group("apply_to_pixmap");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(20);

    group.bench_function("translate_10_10", |b| {
        let t = AffineTransform::translate(10.0, 10.0);
        b.iter(|| {
            black_box(t.apply_to_pixmap(black_box(&src), 320, 180, 320, 180));
        });
    });

    group.bench_function("rotate_45deg", |b| {
        let t = AffineTransform::rotate_at(45.0, 160.0, 90.0);
        b.iter(|| {
            black_box(t.apply_to_pixmap(black_box(&src), 320, 180, 320, 180));
        });
    });

    group.bench_function("scale_150pct", |b| {
        let t = AffineTransform::scale(1.5, 1.5);
        b.iter(|| {
            black_box(t.apply_to_pixmap(black_box(&src), 320, 180, 320, 180));
        });
    });

    group.bench_function("rotate_scale_shear", |b| {
        let t = AffineTransform::rotate_at(30.0, 160.0, 90.0)
            .then(&AffineTransform::scale(1.2, 1.2))
            .then(&AffineTransform::shear(0.1, 0.0));
        b.iter(|| {
            black_box(t.apply_to_pixmap(black_box(&src), 320, 180, 320, 180));
        });
    });

    group.finish();
}

fn bench_shadow(c: &mut Criterion) {
    let w = 320u32;
    let h = 180u32;
    let mut src = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            let idx = ((y * w + x) * 4) as usize;
            if (80..240).contains(&x) && (40..140).contains(&y) {
                src[idx] = 255;
                src[idx + 1] = 255;
                src[idx + 2] = 255;
                src[idx + 3] = 255;
            }
        }
    }

    let mut group = c.benchmark_group("apply_shadow");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(20);

    for (ox, oy) in [(-5.0f32, -5.0), (2.0, 2.0), (8.0, 8.0)] {
        group.bench_function(format!("offset_{}_{}", ox as i32, oy as i32), |b| {
            b.iter(|| {
                black_box(apply_shadow(
                    black_box(&src),
                    w,
                    h,
                    black_box(ox),
                    black_box(oy),
                    black_box(0.0),
                    black_box([0, 0, 0, 128]),
                ));
            });
        });
    }

    group.bench_function("blurred_shadow_radius_5", |b| {
        b.iter(|| {
            black_box(apply_shadow(
                black_box(&src),
                w,
                h,
                black_box(2.0),
                black_box(2.0),
                black_box(5.0),
                black_box([0, 0, 0, 128]),
            ));
        });
    });

    group.finish();
}

fn bench_composite(c: &mut Criterion) {
    let mut group = c.benchmark_group("composite_over");
    group.measurement_time(std::time::Duration::from_secs(5));
    group.sample_size(20);

    for (w, h, label) in [
        (64u32, 32u32, "64x32"),
        (320u32, 180u32, "320x180"),
        (640u32, 360u32, "640x360"),
    ] {
        // Semi-transparent source
        let mut src = vec![0u8; (w * h * 4) as usize];
        for i in 0..(w * h) as usize {
            let idx = i * 4;
            src[idx] = 255;
            src[idx + 1] = 128;
            src[idx + 2] = 64;
            src[idx + 3] = 180;
        }
        // Opaque dark destination
        let dst = vec![32u8; (w * h * 4) as usize];

        group.bench_function(label, |b| {
            b.iter_batched(
                || dst.clone(),
                |mut d| composite_over(&mut d, black_box(&src), w, h),
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Sub-region vs full-frame benchmark
// ---------------------------------------------------------------------------

fn bench_subregion_vs_fullframe(c: &mut Criterion) {
    // Sub-region eligible: no rotation, no clip, no shear.
    let sub_text = "Hello World This Text Uses Sub Region Optimization";

    // Full-frame forced: a small rotation disables the sub-region path.
    let full_text = "{\\frz2}Hello World This Text Uses Full Frame Rendering";

    let ass_content = format!(
        "\
[Script Info]
Title: SubRegion Bench
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, \
OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, \
ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, \
Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,\
0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{sub}
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{full}
",
        sub = sub_text,
        full = full_text,
    );
    let ass = AssFile::parse(&ass_content).expect("sub-region ASS parse");

    let config = RenderConfig::default();
    let renderer = Renderer::new(config);

    let mut group = c.benchmark_group("subregion_vs_fullframe");
    group.sample_size(20);

    group.bench_function("sub_region", |b| {
        b.iter(|| {
            black_box(renderer.render_ass(black_box(&ass), 2000));
        });
    });
    group.finish();

    // Build a separate ASS with only the full-frame event for a fair isolated comparison.
    let full_content = format!(
        "\
[Script Info]
Title: FullFrame Bench
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, \
OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, \
ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, \
Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,\
0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{full}
",
        full = full_text,
    );
    let full_ass = AssFile::parse(&full_content).expect("full-frame ASS parse");

    let mut group = c.benchmark_group("subregion_vs_fullframe");
    group.sample_size(20);

    group.bench_function("full_frame", |b| {
        b.iter(|| {
            black_box(renderer.render_ass(black_box(&full_ass), 2000));
        });
    });
    group.finish();
}

// ---------------------------------------------------------------------------
// Event scaling benchmark
// ---------------------------------------------------------------------------

fn bench_event_scaling(c: &mut Criterion) {
    let config = RenderConfig::default();
    let renderer = Renderer::new(config);

    let mut group = c.benchmark_group("event_scaling");
    group.sample_size(20);

    for count in [1usize, 5, 20] {
        let ass = multi_event_ass(count);
        group.bench_function(format!("{}_simultaneous_events", count), |b| {
            b.iter(|| {
                black_box(renderer.render_ass(black_box(&ass), 3000));
            });
        });
    }
    group.finish();
}

// ---------------------------------------------------------------------------
// Criterion plumbing
// ---------------------------------------------------------------------------

criterion_group!(
    benches,
    // End-to-end
    bench_render_simple,
    bench_render_complex,
    bench_render_karaoke,
    bench_end_to_end_fixture,
    // Per-component
    bench_text_shape,
    bench_blur,
    bench_transform,
    bench_shadow,
    bench_composite,
    // Comparison
    bench_subregion_vs_fullframe,
    bench_event_scaling,
);
criterion_main!(benches);
