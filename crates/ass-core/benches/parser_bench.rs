//! Benchmarks for ass-core parser performance.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_parse_simple(c: &mut Criterion) {
    let content = "\
[Script Info]
Title: Test
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,Hello World
";

    c.bench_function("parse_simple", |b| {
        b.iter(|| {
            let _ = black_box(ass_core::SubtitleDocument::parse(black_box(content)));
        })
    });
}

fn bench_parse_karaoke(c: &mut Criterion) {
    let content = "\
[Script Info]
Title: Karaoke Test
PlayResX: 1920
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,{\\k50}Hel{\\k100}lo {\\kf75}World
Dialogue: 0,0:00:06.00,0:00:10.00,Default,,0,0,0,,{\\b1\\i1}Bold Italic {\\b0\\i0}Normal
Dialogue: 0,0:00:11.00,0:00:15.00,Default,,0,0,0,,{\\clip(1,m 0 0 l 100 0)}Vector
";

    c.bench_function("parse_karaoke", |b| {
        b.iter(|| {
            let _ = black_box(ass_core::SubtitleDocument::parse(black_box(content)));
        })
    });
}

fn bench_parse_recovery(c: &mut Criterion) {
    let content = "\
[Script Info]
Title: Recovery Test
PlayResX: invalid
PlayResY: 1080

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,2,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:05.00,Default,,0,0,0,,First
Dialogue: 0,bad-time,0:00:06.00,Default,,0,0,0,,Bad time
Dialogue: 0,0:00:07.00,0:00:10.00,Default,,0,0,0,,Second
";

    c.bench_function("parse_recovery", |b| {
        b.iter(|| {
            let _ = black_box(ass_core::SubtitleDocument::parse_with_recovery(black_box(
                content,
            )));
        })
    });
}

criterion_group!(
    benches,
    bench_parse_simple,
    bench_parse_karaoke,
    bench_parse_recovery
);
criterion_main!(benches);
