//! Parse a minimal ASS subtitle file and print its events.
//!
//! Run with: `cargo run -p ass-parser --example parse_ass`

use ass_parser::AssFile;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ass = r#"[Script Info]
Title: Example
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,48,&H00FFFFFF,&H000000FF,&H00000000,&H00000000,0,0,0,0,100,100,0,0,1,2,1,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Hello, world!
Dialogue: 0,0:00:04.00,0:00:06.00,Default,,0,0,0,,Second line
"#;

    let script = AssFile::parse(ass)?;

    println!("Title: {:?}", script.script_info.title);
    println!("Styles: {}", script.styles.len());
    println!("Events: {}", script.events.len());

    for event in &script.events {
        let start = event.start.as_ms();
        let end = event.end.as_ms();
        println!(
            "  [{start:>5}ms - {end:>5}ms] {:?}: {}",
            event.event_type, event.text
        );
    }

    Ok(())
}
