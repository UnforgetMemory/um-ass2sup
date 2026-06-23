//! Common test helpers for subtitle-renderer tests using ass_core types.
//!
//! Provides builders for [`SubtitleDocument`], [`Event`], and a shorthand
//! render function so individual tests don't repeat boilerplate.
#![allow(dead_code)]

use ass_core::{
    AssColor, BorderStyle, Effect, Event, EventType, Fps, Margins, ScriptMetadata, Style, StyleRef,
    SubtitleDocument, SubtitleFormat,
};
use subtitle_renderer::{RenderConfig, RenderedFrame, Renderer};

/// Default FPS for tests (23.976 NTSC).
pub fn default_fps() -> Fps {
    Fps::NTSC_24
}

/// Create a document with one "Default" style and an empty event list.
pub fn make_test_doc() -> SubtitleDocument {
    SubtitleDocument {
        format: SubtitleFormat::Ass,
        metadata: ScriptMetadata {
            play_res_x: 1920,
            play_res_y: 1080,
            ..Default::default()
        },
        styles: vec![Style {
            name: StyleRef::new("Default"),
            font_name: "Arial".into(),
            font_size: 48.0,
            primary_color: AssColor::WHITE,
            secondary_color: AssColor::from_raw_abgr(0xFF0000FF),
            outline_color: AssColor::BLACK,
            shadow_color: AssColor::from_raw_abgr(0x80000000),
            bold: false,
            italic: false,
            underline: false,
            strikeout: false,
            scale_x: 100.0,
            scale_y: 100.0,
            spacing: 0.0,
            angle: 0.0,
            border_style: BorderStyle::OutlineAndShadow,
            outline: 2.0,
            shadow: 2.0,
            alignment: ass_core::Alignment::BottomCenter,
            margins: Margins::new(10, 10, 10),
            encoding: ass_core::FontEncoding::new(1),
        }],
        events: Vec::new(),
        fonts: Vec::new(),
        warnings: Vec::new(),
    }
}

/// Create a default renderer (1920×1080).
pub fn default_renderer() -> Renderer {
    Renderer::new(RenderConfig::default())
}

/// Render a subtitle document at the given timestamp.
///
/// Shorthand for `renderer.render_ass(&af, ts)`.
/// NOTE: This function requires conversion from SubtitleDocument to AssFile.
/// For now, callers should use render_ass directly with an AssFile.
/*
pub fn render_doc(renderer: &Renderer, doc: &SubtitleDocument, ts: u64) -> Option<RenderedFrame> {
    renderer.render_ass(&af, ts)
}
*/

/// Build a `SubtitleDocument` with a single dialogue event.
pub fn make_simple_doc(text: &str, start_ms: u64, end_ms: u64) -> SubtitleDocument {
    let mut doc = make_test_doc();
    doc.events.push(make_event(text, start_ms, end_ms));
    doc
}

/// Build a single dialogue event with default field values.
pub fn make_event(text: &str, start_ms: u64, end_ms: u64) -> Event {
    Event {
        source_line: 0,
        event_type: EventType::Dialogue,
        layer: 0,
        start_ms,
        end_ms,
        style: StyleRef::new("Default"),
        actor: String::new(),
        margin_l: None,
        margin_r: None,
        margin_v: None,
        effect: Effect::None,
        text_raw: text.to_string(),
        override_tags: Vec::new(),
        karaoke: Vec::new(),
    }
}

/// Parse an ASS string into a subtitle document.
pub fn parse_doc(content: &str) -> SubtitleDocument {
    SubtitleDocument::parse(content).expect("parse ASS content")
}
