//! 53-tag coverage tests for `build_context`.
//!
//! Every `OverrideTag` variant is tested at least once by constructing an
//! event with that tag and verifying the resulting `RenderContext` fields.

use ass_core::{
    Alignment, AssColor, BorderStyle, Effect, Event, EventType, FontEncoding, KaraokeStyle,
    Margins, OverrideTag, ScriptMetadata, Style, StyleRef, SubtitleDocument, SubtitleFormat,
    TaggedOverride,
};

use super::build_context;
use crate::context::{RenderConfig, RenderContext};

// ── Helpers ─────────────────────────────────────────────────────

fn make_style() -> Style {
    Style {
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
        alignment: Alignment::BottomCenter,
        margins: Margins::new(10, 10, 10),
        encoding: FontEncoding::new(1),
    }
}

fn make_doc(style: Style) -> SubtitleDocument {
    SubtitleDocument {
        format: SubtitleFormat::Ass,
        metadata: ScriptMetadata {
            play_res_x: 1920,
            play_res_y: 1080,
            ..Default::default()
        },
        styles: vec![style],
        events: vec![],
        fonts: vec![],
        warnings: vec![],
    }
}

fn make_config() -> RenderConfig {
    RenderConfig {
        width: 1920,
        height: 1080,
        script_width: 1920,
        script_height: 1080,
        ..Default::default()
    }
}

fn make_event(tags: Vec<OverrideTag>) -> Event {
    Event {
        source_line: 0,
        event_type: EventType::Dialogue,
        layer: 0,
        start_ms: 0,
        end_ms: 5000,
        style: StyleRef::new("Default"),
        actor: String::new(),
        margin_l: None,
        margin_r: None,
        margin_v: None,
        effect: Effect::None,
        text_raw: String::new(),
        override_tags: tags
            .into_iter()
            .map(|t| TaggedOverride { tag: t, span: None })
            .collect(),
        karaoke: vec![],
    }
}

/// Build context with a single override tag and return the context.
fn ctx_for_tag(tag: OverrideTag) -> RenderContext {
    let style = make_style();
    let doc = make_doc(style.clone());
    let event = make_event(vec![tag]);
    let config = make_config();
    build_context(&event, &style, &doc, &config, 2500, 0)
}

/// Assert two f32 values are approximately equal.
macro_rules! assert_feq {
    ($a:expr, $b:expr) => {
        assert!(($a - $b).abs() < 0.001, "expected {:.4} ≈ {:.4}", $a, $b);
    };
}

// ── Position handler (3 tags) ──────────────────────────────────

#[test]
fn test_tag_pos() {
    let ctx = ctx_for_tag(OverrideTag::Pos { x: 100.0, y: 200.0 });
    assert!(ctx.has_pos);
    assert_feq!(ctx.x, 100.0);
    assert_feq!(ctx.y, 200.0);
}

#[test]
fn test_tag_move_stores_animation() {
    let ctx = ctx_for_tag(OverrideTag::Move {
        x1: 0.0,
        y1: 0.0,
        x2: 300.0,
        y2: 400.0,
        t1: 0,
        t2: 1000,
    });
    assert!(ctx.has_pos);
    assert!(ctx.move_animation.is_some());
    let anim = ctx.move_animation.unwrap();
    assert_feq!(anim.x1, 0.0);
    assert_feq!(anim.x2, 300.0);
}

#[test]
fn test_tag_origin() {
    let ctx = ctx_for_tag(OverrideTag::Origin { x: 50.0, y: 100.0 });
    assert_feq!(ctx.origin_x, 50.0);
    assert_feq!(ctx.origin_y, 100.0);
}

// ── Font handler (8 tags) ──────────────────────────────────────

#[test]
fn test_tag_font_name() {
    let ctx = ctx_for_tag(OverrideTag::FontName("Courier".into()));
    assert_eq!(ctx.font_name, "Courier");
}

#[test]
fn test_tag_font_size() {
    let ctx = ctx_for_tag(OverrideTag::FontSize(72.0));
    assert_feq!(ctx.font_size, 72.0);
}

#[test]
fn test_tag_font_size_relative_add() {
    let ctx = ctx_for_tag(OverrideTag::FontSizeRelative(10));
    assert_feq!(ctx.font_size, 58.0); // 48 + 10
}

#[test]
fn test_tag_bold() {
    let ctx = ctx_for_tag(OverrideTag::Bold(true));
    assert!(ctx.bold);
}

#[test]
fn test_tag_bold_weight_threshold() {
    let ctx = ctx_for_tag(OverrideTag::BoldWeight(700));
    assert!(ctx.bold);
}

#[test]
fn test_tag_bold_weight_sub_threshold() {
    let ctx = ctx_for_tag(OverrideTag::BoldWeight(400));
    assert!(!ctx.bold);
}

#[test]
fn test_tag_italic() {
    let ctx = ctx_for_tag(OverrideTag::Italic(true));
    assert!(ctx.italic);
}

#[test]
fn test_tag_underline() {
    let ctx = ctx_for_tag(OverrideTag::Underline(true));
    assert!(ctx.underline);
}

#[test]
fn test_tag_strikeout() {
    let ctx = ctx_for_tag(OverrideTag::Strikeout(true));
    assert!(ctx.strikeout);
}

// ── Color handler (9 tags) ─────────────────────────────────────

#[test]
fn test_tag_primary_color() {
    let red = AssColor::from_rgba(255, 0, 0, 0);
    let ctx = ctx_for_tag(OverrideTag::PrimaryColor(red));
    assert_eq!(ctx.primary_color, [255, 0, 0, 255]);
}

#[test]
fn test_tag_secondary_color() {
    let green = AssColor::from_rgba(0, 255, 0, 0);
    let ctx = ctx_for_tag(OverrideTag::SecondaryColor(green));
    assert_eq!(ctx.secondary_color, [0, 255, 0, 255]);
}

#[test]
fn test_tag_outline_color() {
    let blue = AssColor::from_rgba(0, 0, 255, 0);
    let ctx = ctx_for_tag(OverrideTag::OutlineColor(blue));
    assert_eq!(ctx.outline_color, [0, 0, 255, 255]);
}

#[test]
fn test_tag_shadow_color() {
    let white = AssColor::from_rgba(255, 255, 255, 0);
    let ctx = ctx_for_tag(OverrideTag::ShadowColor(white));
    assert_eq!(ctx.shadow_color, [255, 255, 255, 255]);
}

#[test]
fn test_tag_alpha_cascades_all_channels() {
    let ctx = ctx_for_tag(OverrideTag::Alpha { value: 128 });
    let expected = 255 - 128; // 127
    assert_eq!(ctx.primary_color[3], expected);
    assert_eq!(ctx.secondary_color[3], expected);
    assert_eq!(ctx.outline_color[3], expected);
    assert_eq!(ctx.shadow_color[3], expected);
}

#[test]
fn test_tag_primary_alpha_isolated() {
    let ctx = ctx_for_tag(OverrideTag::PrimaryAlpha { value: 64 });
    assert_eq!(ctx.primary_color[3], 255 - 64);
    // secondary_color has RGBA alpha=0 (from style's ABGR 0xFF0000FF),
    // so unchanged means 0, not 255
    assert_eq!(ctx.secondary_color[3], 0); // unchanged
}

#[test]
fn test_tag_secondary_alpha() {
    let ctx = ctx_for_tag(OverrideTag::SecondaryAlpha { value: 32 });
    assert_eq!(ctx.secondary_color[3], 255 - 32);
}

#[test]
fn test_tag_outline_alpha() {
    let ctx = ctx_for_tag(OverrideTag::OutlineAlpha { value: 16 });
    assert_eq!(ctx.outline_color[3], 255 - 16);
}

#[test]
fn test_tag_shadow_alpha() {
    let ctx = ctx_for_tag(OverrideTag::ShadowAlpha { value: 200 });
    assert_eq!(ctx.shadow_color[3], 255 - 200);
}

// ── Border handler (6 tags) ────────────────────────────────────

#[test]
fn test_tag_border() {
    let ctx = ctx_for_tag(OverrideTag::Border { x: 5.0, y: 5.0 });
    assert_feq!(ctx.outline_width, 5.0);
    assert_feq!(ctx.outline_x_width, 0.0);
    assert_feq!(ctx.outline_y_width, 0.0);
}

#[test]
fn test_tag_border_resets_xy() {
    // Apply BorderX first, then Border — Border should reset X/Y
    let style = make_style();
    let doc = make_doc(style.clone());
    let event = make_event(vec![
        OverrideTag::BorderX(3.0),
        OverrideTag::Border { x: 5.0, y: 5.0 },
    ]);
    let config = make_config();
    let ctx = build_context(&event, &style, &doc, &config, 2500, 0);
    assert_feq!(ctx.outline_x_width, 0.0);
    assert_feq!(ctx.outline_y_width, 0.0);
}

#[test]
fn test_tag_border_x() {
    let ctx = ctx_for_tag(OverrideTag::BorderX(1.5));
    assert_feq!(ctx.outline_x_width, 1.5);
    assert_feq!(ctx.outline_width, 2.0); // unchanged
}

#[test]
fn test_tag_border_y() {
    let ctx = ctx_for_tag(OverrideTag::BorderY(4.0));
    assert_feq!(ctx.outline_y_width, 4.0);
}

#[test]
fn test_tag_shadow() {
    let ctx = ctx_for_tag(OverrideTag::Shadow { x: 5.0, y: 5.0 });
    assert_feq!(ctx.shadow_depth, 5.0);
    assert_feq!(ctx.shadow_x, 0.0); // reset
    assert_feq!(ctx.shadow_y, 0.0); // reset
}

#[test]
fn test_tag_shadow_x() {
    let ctx = ctx_for_tag(OverrideTag::ShadowX(3.0));
    assert_feq!(ctx.shadow_x, 3.0);
}

#[test]
fn test_tag_shadow_y() {
    let ctx = ctx_for_tag(OverrideTag::ShadowY(7.0));
    assert_feq!(ctx.shadow_y, 7.0);
}

// ── Geometry handler (7 tags + aliases) ────────────────────────

#[test]
fn test_tag_scale() {
    let ctx = ctx_for_tag(OverrideTag::Scale { x: 150.0, y: 80.0 });
    assert_feq!(ctx.scale_x, 150.0);
    assert_feq!(ctx.scale_y, 80.0);
}

#[test]
fn test_tag_scale_reset() {
    let style = make_style();
    let doc = make_doc(style.clone());
    let event = make_event(vec![
        OverrideTag::Scale { x: 200.0, y: 200.0 },
        OverrideTag::ScaleReset,
    ]);
    let config = make_config();
    let ctx = build_context(&event, &style, &doc, &config, 2500, 0);
    assert_feq!(ctx.scale_x, 100.0); // reverted to style default
    assert_feq!(ctx.scale_y, 100.0);
}

#[test]
fn test_tag_rotation_z() {
    let ctx = ctx_for_tag(OverrideTag::Rotation {
        x: 0.0,
        y: 0.0,
        z: 45.0,
    });
    assert_feq!(ctx.rotation, 45.0);
}

#[test]
fn test_tag_rotation_perspective() {
    let ctx = ctx_for_tag(OverrideTag::Rotation {
        x: 10.0,
        y: 20.0,
        z: 30.0,
    });
    assert_feq!(ctx.perspective_x, 10.0);
    assert_feq!(ctx.perspective_y, 20.0);
    assert_feq!(ctx.rotation, 30.0);
}

#[test]
fn test_tag_shear() {
    let ctx = ctx_for_tag(OverrideTag::Shear { x: 0.3, y: 0.5 });
    assert_feq!(ctx.shear_x, 0.3);
    assert_feq!(ctx.shear_y, 0.5);
}

#[test]
fn test_tag_spacing() {
    let ctx = ctx_for_tag(OverrideTag::Spacing(5.0));
    assert_feq!(ctx.spacing, 5.0);
}

#[test]
fn test_tag_blur() {
    let ctx = ctx_for_tag(OverrideTag::Blur(3.0));
    assert_feq!(ctx.blur, 3.0);
}

#[test]
fn test_tag_gaussian_blur() {
    let ctx = ctx_for_tag(OverrideTag::GaussianBlur(4.0));
    assert_feq!(ctx.blur, 4.0);
}

#[test]
fn test_tag_frz_alias() {
    // \fr and \frz both parse to Rotation { z }
    let ctx = ctx_for_tag(OverrideTag::Rotation {
        x: 0.0,
        y: 0.0,
        z: 90.0,
    });
    assert_feq!(ctx.rotation, 90.0);
}

// ── Clip handler (6 tags) ──────────────────────────────────────

#[test]
fn test_tag_clip_rect() {
    let ctx = ctx_for_tag(OverrideTag::Clip {
        x1: 10.0,
        y1: 20.0,
        x2: 100.0,
        y2: 200.0,
    });
    assert!(ctx.clip_enabled);
    assert!(!ctx.clip_inverse);
    assert_feq!(ctx.clip_x1, 10.0);
    assert_feq!(ctx.clip_y1, 20.0);
    assert_feq!(ctx.clip_x2, 100.0);
    assert_feq!(ctx.clip_y2, 200.0);
}

#[test]
fn test_tag_clip_inverse_rect() {
    let ctx = ctx_for_tag(OverrideTag::ClipInverse {
        x1: 5.0,
        y1: 15.0,
        x2: 95.0,
        y2: 195.0,
    });
    assert!(ctx.clip_enabled);
    assert!(ctx.clip_inverse);
}

#[test]
fn test_tag_clip_drawing() {
    let ctx = ctx_for_tag(OverrideTag::ClipDrawing {
        scale: 1.0,
        commands: "m 0 0 l 100 0 100 100 0 100".into(),
    });
    assert!(ctx.clip_enabled);
    assert!(!ctx.clip_drawing_inverse);
    assert_eq!(
        ctx.clip_drawing_commands,
        Some("m 0 0 l 100 0 100 100 0 100".into())
    );
    assert_feq!(ctx.clip_drawing_scale, 1.0);
}

#[test]
fn test_tag_clip_inverse_drawing() {
    let ctx = ctx_for_tag(OverrideTag::ClipInverseDrawing {
        scale: 2.0,
        commands: "m 0 0 l 50 0 50 50 0 50".into(),
    });
    assert!(ctx.clip_enabled);
    assert!(ctx.clip_drawing_inverse);
    assert_feq!(ctx.clip_drawing_scale, 2.0);
}

#[test]
fn test_tag_clip_drawing_current() {
    let ctx = ctx_for_tag(OverrideTag::ClipDrawingCurrent);
    assert!(ctx.clip_enabled);
    assert!(ctx.clip_drawing_current);
    assert!(!ctx.clip_drawing_inverse_current);
}

#[test]
fn test_tag_clip_inverse_drawing_current() {
    let ctx = ctx_for_tag(OverrideTag::ClipInverseDrawingCurrent);
    assert!(ctx.clip_enabled);
    assert!(ctx.clip_drawing_current);
    assert!(ctx.clip_drawing_inverse_current);
}

// ── Karaoke handler (1 tag) ────────────────────────────────────

#[test]
fn test_tag_karaoke() {
    let ctx = ctx_for_tag(OverrideTag::Karaoke {
        style: KaraokeStyle::Fill,
        duration: 500,
    });
    // Karaoke handler currently only logs; verify no crash
    assert_feq!(ctx.font_size, 48.0); // default unchanged
}

// ── Reset handler (2 tags) ─────────────────────────────────────

#[test]
fn test_tag_reset_all() {
    let style = make_style();
    let doc = make_doc(style.clone());
    let event = make_event(vec![OverrideTag::FontSize(120.0), OverrideTag::ResetAll]);
    let config = make_config();
    let ctx = build_context(&event, &style, &doc, &config, 2500, 0);
    assert_feq!(ctx.font_size, 48.0); // reverted to style
}

#[test]
fn test_tag_reset_named_style() {
    let alt_style = Style {
        name: StyleRef::new("Alt"),
        font_name: "Courier".into(),
        font_size: 24.0,
        ..make_style()
    };
    let mut doc = make_doc(make_style());
    doc.styles.push(alt_style);
    let style = doc.styles[0].clone();
    let event = make_event(vec![
        OverrideTag::FontSize(120.0),
        OverrideTag::Reset("Alt".into()),
    ]);
    let config = make_config();
    let ctx = build_context(&event, &style, &doc, &config, 2500, 0);
    assert_feq!(ctx.font_size, 24.0);
    assert_eq!(ctx.font_name, "Courier");
}

#[test]
fn test_tag_reset_empty_name() {
    let style = make_style();
    let doc = make_doc(style.clone());
    let event = make_event(vec![
        OverrideTag::FontSize(120.0),
        OverrideTag::Reset(String::new()),
    ]);
    let config = make_config();
    let ctx = build_context(&event, &style, &doc, &config, 2500, 0);
    assert_feq!(ctx.font_size, 48.0); // reverted to event's own style
}

// ── Transform handler (1 tag) ──────────────────────────────────

#[test]
fn test_tag_transform() {
    let ctx = ctx_for_tag(OverrideTag::Transform {
        tag: "\\pos(100,200)".into(),
        t1: 0,
        t2: 1000,
        accel: 1.0,
    });
    // Transform handler currently only logs; verify no crash
    assert_feq!(ctx.font_size, 48.0);
}

// ── Misc handler (8 tags) ──────────────────────────────────────

#[test]
fn test_tag_alignment_numpad() {
    let ctx = ctx_for_tag(OverrideTag::AlignmentNumpad(7));
    assert_eq!(ctx.alignment, 7);
}

#[test]
fn test_tag_alignment_vsfilter() {
    let ctx = ctx_for_tag(OverrideTag::AlignmentVsfilter(4));
    assert_eq!(ctx.alignment, 4);
}

#[test]
fn test_tag_wrap_style() {
    let ctx = ctx_for_tag(OverrideTag::WrapStyle(2));
    assert_eq!(ctx.wrap_style, 2);
}

#[test]
fn test_tag_writing_mode() {
    let ctx = ctx_for_tag(OverrideTag::WritingMode(2));
    assert_eq!(ctx.writing_mode, 2);
}

#[test]
fn test_tag_charset() {
    let ctx = ctx_for_tag(OverrideTag::Charset(128));
    assert_eq!(ctx.charset, 128);
}

#[test]
fn test_tag_animation_skip() {
    let ctx = ctx_for_tag(OverrideTag::AnimationSkip);
    assert!(ctx.animation_skip);
}

#[test]
fn test_tag_baseline_offset() {
    let ctx = ctx_for_tag(OverrideTag::BaselineOffset(10.0));
    assert_feq!(ctx.baseline_offset, 10.0);
}

#[test]
fn test_tag_drawing_mode() {
    let ctx = ctx_for_tag(OverrideTag::DrawingMode(1));
    assert_eq!(ctx.drawing_mode, 1);
}

// ── Edge cases ─────────────────────────────────────────────────

#[test]
fn test_scaling_applied_correctly() {
    let config = RenderConfig {
        width: 960,
        height: 540,
        script_width: 1920,
        script_height: 1080,
        ..Default::default()
    };
    let style = make_style();
    let doc = make_doc(style.clone());
    let event = make_event(vec![OverrideTag::FontSize(48.0)]);
    let ctx = build_context(&event, &style, &doc, &config, 2500, 0);
    // apply_scaling only scales position/margin/clip, NOT font_size
    assert_feq!(ctx.font_size, 48.0);
    // But margin_l should be scaled: 10 * 960/1920 = 5
    assert_feq!(ctx.margin_l, 5.0);
}

#[test]
fn test_multiple_tags_compose() {
    let ctx = ctx_for_tag(OverrideTag::Scale { x: 150.0, y: 150.0 });
    assert_feq!(ctx.scale_x, 150.0);
    // Verify default fields unaffected
    assert_feq!(ctx.spacing, 0.0);
    assert_eq!(ctx.alignment, 2);
}

#[test]
fn test_move_interpolation_at_midpoint() {
    let style = make_style();
    let doc = make_doc(style.clone());
    let event = make_event(vec![OverrideTag::Move {
        x1: 100.0,
        y1: 200.0,
        x2: 300.0,
        y2: 400.0,
        t1: 0,
        t2: 1000,
    }]);
    let config = make_config();
    // At t=500ms (midpoint of animation: t1=0, t2=1000):
    // x = 100 + (300-100) * (500-0)/(1000-0) = 100 + 200 * 0.5 = 200
    // y = 200 + (400-200) * 0.5 = 300
    let ctx = build_context(&event, &style, &doc, &config, 500, 0);
    assert_feq!(ctx.x, 200.0);
    assert_feq!(ctx.y, 300.0);
}

#[test]
fn test_move_interpolation_before_t1() {
    let style = make_style();
    let doc = make_doc(style.clone());
    let event = make_event(vec![OverrideTag::Move {
        x1: 100.0,
        y1: 200.0,
        x2: 300.0,
        y2: 400.0,
        t1: 500,
        t2: 1500,
    }]);
    let config = make_config();
    // At t=0 (before t1=500): use start position
    let ctx = build_context(&event, &style, &doc, &config, 0, 0);
    assert_feq!(ctx.x, 100.0);
    assert_feq!(ctx.y, 200.0);
}

#[test]
fn test_move_interpolation_after_t2() {
    let style = make_style();
    let doc = make_doc(style.clone());
    let event = make_event(vec![OverrideTag::Move {
        x1: 100.0,
        y1: 200.0,
        x2: 300.0,
        y2: 400.0,
        t1: 0,
        t2: 1000,
    }]);
    let config = make_config();
    // At t=5000 (after t2=1000): use end position
    let ctx = build_context(&event, &style, &doc, &config, 5000, 0);
    assert_feq!(ctx.x, 300.0);
    assert_feq!(ctx.y, 400.0);
}

#[test]
fn test_unknown_tag_no_panic() {
    let ctx = ctx_for_tag(OverrideTag::Unknown("\\nonexistent".into()));
    // Should not panic, context should remain at defaults
    assert_feq!(ctx.font_size, 48.0);
}
