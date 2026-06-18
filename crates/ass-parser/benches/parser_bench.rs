//! Criterion benchmark scaffold for the v2.0 Sub-9 testing sprint.
//!
//! Run with:
//!   cargo bench -p ass-parser
//!
//! Benchmarks:
//!   - `parse_simple_ass`     : cold parse of a small ASS file
//!   - `parse_karaoke_ass`    : cold parse of a karaoke-tagged ASS
//!   - `parse_with_recovery` : parse_with_recovery on a malformed ASS

use ass_parser::AssFile;
use criterion::{criterion_group, criterion_main, Criterion};

fn parse_simple_ass(c: &mut Criterion) {
    let input = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,Hello world\n";
    c.bench_function("parse_simple_ass", |b| {
        b.iter(|| AssFile::parse(input).expect("parse"));
    });
}

fn parse_karaoke_ass(c: &mut Criterion) {
    let input = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:00.00,0:00:05.00,Default,,0,0,0,,{\\k50}Hello {\\k50}world\n";
    c.bench_function("parse_karaoke_ass", |b| {
        b.iter(|| AssFile::parse(input).expect("parse"));
    });
}

fn parse_with_recovery(c: &mut Criterion) {
    let input = "[Script Info]\nScriptType: v4.00+\n\n[V4+ Styles]\nFormat: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding\nStyle: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,BAD,0:00:05.00,Default,,0,0,0,,oops\nDialogue: 0,0:00:05.00,0:00:10.00,Default,,0,0,0,,valid\n";
    c.bench_function("parse_with_recovery", |b| {
        b.iter(|| {
            let (_file, _warnings) = AssFile::parse_with_recovery(input);
        });
    });
}

criterion_group!(
    benches,
    parse_simple_ass,
    parse_karaoke_ass,
    parse_with_recovery,
);
criterion_main!(benches);
