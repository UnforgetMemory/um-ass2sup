use criterion::{black_box, criterion_group, criterion_main, Criterion};
use ass_parser::{AssFile, Event, EventType, Timestamp};
use ass_parser::karaoke::{KaraokeSegment, KaraokeStyle};
use tiny_skia::Pixmap;
use subtitle_renderer::{
    apply_gaussian_blur, AffineTransform, RenderConfig, Renderer, Shaper,
};

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
        effect: String::new(),
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
    // Create a 320x180 pixmap with some content
    let mut pixmap = Pixmap::new(320, 180).unwrap();
    let data = pixmap.data_mut();
    for y in 0..180u32 {
        for x in 0..320u32 {
            let idx = ((y * 320 + x) * 4) as usize;
            if x >= 80 && x < 240 && y >= 40 && y < 140 {
                data[idx] = 255;
                data[idx + 1] = 255;
                data[idx + 2] = 255;
                data[idx + 3] = 255;
            }
        }
    }

    let mut group = c.benchmark_group("blur_320x180");
    for radius in [1.0, 3.0, 5.0, 10.0] {
        group.bench_function(format!("radius_{:.0}", radius), |b| {
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
            if x >= 80 && x < 240 && y >= 40 && y < 140 {
                data[idx] = 255;
                data[idx + 1] = 255;
                data[idx + 2] = 255;
                data[idx + 3] = 255;
            }
        }
    }
    let src = pixmap.data().to_vec();

    let mut group = c.benchmark_group("transform_320x180");

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

criterion_group!(
    benches,
    bench_render_simple,
    bench_render_complex,
    bench_render_karaoke,
    bench_text_shape,
    bench_blur,
    bench_transform,
);
criterion_main!(benches);
