use ass_parser::{AssColor, AssFile, Effect, Event, EventType, OverrideTag, Style, Timestamp};
use subtitle_renderer::{
    alignment_to_pos, strip_override_blocks, RenderConfig, RenderContext, Renderer,
};

fn default_ass() -> AssFile {
    AssFile::new()
}

fn default_event() -> Event {
    Event {
        event_type: EventType::Dialogue,
        layer: 0,
        start: Timestamp::from_ms(0),
        end: Timestamp::from_ms(5000),
        style_name: "Default".to_string(),
        name: String::new(),
        margin_l: 0,
        margin_r: 0,
        margin_v: 0,
        effect: Effect::None,
        text: "Hello".to_string(),
        override_tags: Vec::new(),
        karaoke_segments: Vec::new(),
        raw_override_block: String::new(),
    }
}

fn default_renderer() -> Renderer {
    Renderer::new(RenderConfig::default())
}

#[test]
fn test_render_context_default_new_fields() {
    let ctx = RenderContext::default();
    assert_eq!(ctx.origin_x, 0.0);
    assert_eq!(ctx.origin_y, 0.0);
    assert_eq!(ctx.shear_x, 0.0);
    assert_eq!(ctx.shear_y, 0.0);
    assert_eq!(ctx.clip_x1, -1.0);
    assert_eq!(ctx.clip_y1, -1.0);
    assert_eq!(ctx.clip_x2, -1.0);
    assert_eq!(ctx.clip_y2, -1.0);
    assert!(!ctx.clip_enabled);
    assert_eq!(ctx.wrap_style, 0);
    assert!(!ctx.underline);
    assert!(!ctx.strikeout);
}

#[test]
fn test_render_context_default_existing_fields() {
    let ctx = RenderContext::default();
    assert_eq!(ctx.x, 0.0);
    assert_eq!(ctx.y, 0.0);
    assert_eq!(ctx.font_name, "Arial");
    assert_eq!(ctx.font_size, 48.0);
    assert_eq!(ctx.primary_color, [255, 255, 255, 255]);
    assert_eq!(ctx.secondary_color, [0, 0, 255, 255]);
    assert_eq!(ctx.outline_color, [0, 0, 0, 255]);
    assert_eq!(ctx.shadow_color, [0, 0, 0, 128]);
    assert!(!ctx.bold);
    assert!(!ctx.italic);
    assert_eq!(ctx.outline_width, 2.0);
    assert_eq!(ctx.shadow_depth, 2.0);
    assert_eq!(ctx.blur, 0.0);
    assert_eq!(ctx.rotation, 0.0);
    assert_eq!(ctx.scale_x, 100.0);
    assert_eq!(ctx.scale_y, 100.0);
    assert_eq!(ctx.spacing, 0.0);
    assert_eq!(ctx.alignment, 2);
    assert_eq!(ctx.margin_l, 10.0);
    assert_eq!(ctx.margin_r, 10.0);
    assert_eq!(ctx.margin_v, 10.0);
}

#[test]
fn test_alignment_to_pos_all_alignments() {
    assert_eq!(alignment_to_pos(1), (0.0, 1.0));
    assert_eq!(alignment_to_pos(2), (0.5, 1.0));
    assert_eq!(alignment_to_pos(3), (1.0, 1.0));
    assert_eq!(alignment_to_pos(4), (0.0, 0.5));
    assert_eq!(alignment_to_pos(5), (0.5, 0.5));
    assert_eq!(alignment_to_pos(6), (1.0, 0.5));
    assert_eq!(alignment_to_pos(7), (0.0, 0.0));
    assert_eq!(alignment_to_pos(8), (0.5, 0.0));
    assert_eq!(alignment_to_pos(9), (1.0, 0.0));
}

#[test]
fn test_alignment_to_pos_default_fallback() {
    assert_eq!(alignment_to_pos(0), (0.5, 1.0));
    assert_eq!(alignment_to_pos(10), (0.5, 1.0));
    assert_eq!(alignment_to_pos(255), (0.5, 1.0));
}

#[test]
fn test_strip_override_blocks_no_tags() {
    assert_eq!(strip_override_blocks("Hello World"), "Hello World");
}

#[test]
fn test_strip_override_blocks_single_block() {
    assert_eq!(strip_override_blocks("{\\b1}Bold"), "Bold");
}

#[test]
fn test_strip_override_blocks_multiple_blocks() {
    assert_eq!(
        strip_override_blocks("{\\b1}Hello{\\i1} World"),
        "Hello World"
    );
}

#[test]
fn test_strip_override_blocks_nested_braces() {
    assert_eq!(strip_override_blocks("{{\\b1}}Text"), "Text");
}

#[test]
fn test_strip_override_blocks_empty_string() {
    assert_eq!(strip_override_blocks(""), "");
}

#[test]
fn test_strip_override_blocks_only_tags() {
    assert_eq!(strip_override_blocks("{\\b1}{\\i1}"), "");
}

#[test]
fn test_strip_override_blocks_text_with_newlines() {
    assert_eq!(
        strip_override_blocks("{\\b1}Line1\\NLine2"),
        "Line1\\NLine2"
    );
}

#[test]
fn test_build_context_pos_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::Pos { x: 100.0, y: 200.0 }];
    let style = Style::default();
    // \pos sets fixed position — no time interpolation
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.x, 100.0);
    assert_eq!(ctx.y, 200.0);
    // Same position at different timestamps
    let ctx2 = renderer.build_context(&event, &style, &default_ass(), 500, 0, 5000);
    assert_eq!(ctx2.x, 100.0);
    assert_eq!(ctx2.y, 200.0);
}

#[test]
fn test_build_context_move_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::Move {
        x1: 50.0,
        y1: 60.0,
        x2: 300.0,
        y2: 400.0,
        t1: 0,
        t2: 1000,
    }];
    let style = Style::default();
    // At t=2500, move t2=1000 => animation complete => end position
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.x, 300.0);
    assert_eq!(ctx.y, 400.0);
    // At t=500 (between t1=0 and t2=1000) => interpolated position
    let ctx_mid = renderer.build_context(&event, &style, &default_ass(), 500, 0, 5000);
    assert!(ctx_mid.x > 50.0 && ctx_mid.x < 300.0);
    assert!(ctx_mid.y > 60.0 && ctx_mid.y < 400.0);
}

#[test]
fn test_build_context_clip_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::Clip {
        x1: 10.0,
        y1: 20.0,
        x2: 100.0,
        y2: 200.0,
    }];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert!(ctx.clip_enabled);
    assert_eq!(ctx.clip_x1, 10.0);
    assert_eq!(ctx.clip_y1, 20.0);
    assert_eq!(ctx.clip_x2, 100.0);
    assert_eq!(ctx.clip_y2, 200.0);
}

#[test]
fn test_build_context_clip_inverse_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::ClipInverse {
        x1: 5.0,
        y1: 15.0,
        x2: 95.0,
        y2: 195.0,
    }];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert!(ctx.clip_enabled);
    assert_eq!(ctx.clip_x1, 5.0);
    assert_eq!(ctx.clip_y1, 15.0);
    assert_eq!(ctx.clip_x2, 95.0);
    assert_eq!(ctx.clip_y2, 195.0);
}

#[test]
fn test_build_context_scale_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::Scale { x: 150.0, y: 80.0 }];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.scale_x, 150.0);
    assert_eq!(ctx.scale_y, 80.0);
}

#[test]
fn test_build_context_rotation_z_only() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::Rotation {
        x: 0.0,
        y: 0.0,
        z: 45.0,
    }];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.rotation, 45.0);
    assert_eq!(ctx.origin_x, 0.0);
    assert_eq!(ctx.origin_y, 0.0);
}

#[test]
fn test_build_context_rotation_with_origin() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![
        OverrideTag::Rotation {
            x: 0.0,
            y: 0.0,
            z: 90.0,
        },
        OverrideTag::Origin { x: 100.0, y: 200.0 },
    ];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.rotation, 90.0);
    assert_eq!(
        ctx.origin_x, 100.0,
        "Origin x scaled by width/script_width (1920/1920=1.0)"
    );
    assert_eq!(
        ctx.origin_y, 200.0,
        "Origin y scaled by height/script_height (1080/1080=1.0)"
    );
}

#[test]
fn test_build_context_blur_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::Blur(5.0)];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.blur, 5.0);
}

#[test]
fn test_build_context_gaussian_blur_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::GaussianBlur(3.0)];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.blur, 3.0);
}

#[test]
fn test_build_context_shadow_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::Shadow(4.0)];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.shadow_depth, 4.0);
}

#[test]
fn test_build_context_shadow_xy_tags() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::ShadowX(3.0), OverrideTag::ShadowY(7.0)];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.shadow_x, 3.0, "ShadowX sets shadow_x");
    assert_eq!(ctx.shadow_y, 7.0, "ShadowY sets shadow_y");
    assert_eq!(ctx.shadow_depth, 2.0, "ShadowDepth stays at style default");
}

#[test]
fn test_build_context_border_xy_tags() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::BorderX(1.5), OverrideTag::BorderY(4.0)];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.outline_x_width, 1.5, "BorderX sets outline_x_width");
    assert_eq!(ctx.outline_y_width, 4.0, "BorderY sets outline_y_width");
    assert_eq!(
        ctx.outline_width, 2.0,
        "outline_width stays at style default"
    );
}

#[test]
fn test_build_context_alpha_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::Alpha { value: 128 }];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    let expected = 255 - 128;
    assert_eq!(ctx.primary_color[3], expected);
    assert_eq!(ctx.secondary_color[3], expected);
    assert_eq!(ctx.outline_color[3], expected);
    assert_eq!(ctx.shadow_color[3], expected);
}

#[test]
fn test_build_context_primary_alpha_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::PrimaryAlpha { value: 100 }];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.primary_color[3], 255 - 100);
    assert_eq!(ctx.secondary_color[3], 255, "other colors unaffected");
}

#[test]
fn test_build_context_wrap_style_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::WrapStyle(2)];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.wrap_style, 2);
}

#[test]
fn test_build_context_underline_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::Underline(true)];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert!(ctx.underline);
}

#[test]
fn test_build_context_strikeout_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::Strikeout(true)];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert!(ctx.strikeout);
}

#[test]
fn test_build_context_font_size_scaling() {
    let config = RenderConfig {
        width: 960,
        height: 540,
        script_width: 1920,
        script_height: 1080,
        ..Default::default()
    };
    let renderer = Renderer::new(config);
    let event = default_event();
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    let expected = style.font_size as f32 * 540.0 / 1080.0;
    assert!((ctx.font_size - expected).abs() < 0.01);
}

#[test]
fn test_build_context_margin_scaling() {
    let config = RenderConfig {
        width: 960,
        height: 540,
        script_width: 1920,
        script_height: 1080,
        ..Default::default()
    };
    let renderer = Renderer::new(config);
    let mut event = default_event();
    event.margin_l = 20;
    event.margin_r = 40;
    event.margin_v = 30;
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert!((ctx.margin_l - 10.0).abs() < 0.01);
    assert!((ctx.margin_r - 20.0).abs() < 0.01);
    assert!((ctx.margin_v - 15.0).abs() < 0.01);
}

#[test]
fn test_build_context_alignment_sets_position() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::AlignmentNumpad(7)];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.alignment, 7);
    assert!(
        (ctx.x - ctx.margin_l).abs() < 0.01,
        "alignment 7 should be left-aligned"
    );
    // alignment 7 is top-aligned: y = margin_v + font_size (baseline shift
    // to keep upward-extending glyphs within frame)
    assert!(
        (ctx.y - (ctx.margin_v + ctx.font_size)).abs() < 0.01,
        "alignment 7 should be top-aligned with font_size baseline shift"
    );
}

#[test]
fn test_build_context_no_pos_alignment_computed() {
    let renderer = default_renderer();
    let event = default_event();
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.alignment, 2);
    let expected_x = ctx.margin_l;
    let expected_y = ctx.margin_v + 1.0 * (1080.0 - ctx.margin_v * 2.0);
    assert!((ctx.x - expected_x).abs() < 0.01);
    assert!((ctx.y - expected_y).abs() < 0.01);
}

#[test]
fn test_build_context_font_name_and_size_override() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![
        OverrideTag::FontName("Courier".to_string()),
        OverrideTag::FontSize(72.0),
    ];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.font_name, "Courier");
    assert_eq!(ctx.font_size, 72.0);
}

#[test]
fn test_build_context_bold_and_italic_override() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::Bold(true), OverrideTag::Italic(true)];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert!(ctx.bold);
    assert!(ctx.italic);
}

#[test]
fn test_build_context_bold_weight_threshold() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::BoldWeight(700)];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert!(ctx.bold, "weight 700 should be bold");

    let mut event2 = default_event();
    event2.override_tags = vec![OverrideTag::BoldWeight(400)];
    let ctx2 = renderer.build_context(&event2, &style, &default_ass(), 1000, 0, 5000);
    assert!(!ctx2.bold, "weight 400 should not be bold");
}

#[test]
fn test_build_context_color_overrides() {
    let renderer = default_renderer();
    let mut event = default_event();
    let red = AssColor::from_rgba(255, 0, 0, 0);
    let green = AssColor::from_rgba(0, 255, 0, 0);
    event.override_tags = vec![
        OverrideTag::PrimaryColor(red),
        OverrideTag::SecondaryColor(green),
    ];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.primary_color, [255, 0, 0, 255]);
    assert_eq!(ctx.secondary_color, [0, 255, 0, 255]);
}

#[test]
fn test_build_context_spacing_tag() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![OverrideTag::Spacing(5.0)];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.spacing, 5.0);
}

#[test]
fn test_build_context_multiple_tags() {
    let renderer = default_renderer();
    let mut event = default_event();
    event.override_tags = vec![
        OverrideTag::FontSize(36.0),
        OverrideTag::Bold(true),
        OverrideTag::Border(3.0),
        OverrideTag::Shadow(5.0),
        OverrideTag::Blur(2.0),
        OverrideTag::WrapStyle(1),
        OverrideTag::Underline(true),
        OverrideTag::Strikeout(true),
        OverrideTag::Clip {
            x1: 10.0,
            y1: 20.0,
            x2: 500.0,
            y2: 400.0,
        },
    ];
    let style = Style::default();
    let ctx = renderer.build_context(&event, &style, &default_ass(), 2500, 0, 5000);
    assert_eq!(ctx.font_size, 36.0);
    assert!(ctx.bold);
    assert_eq!(ctx.outline_width, 3.0);
    assert_eq!(ctx.shadow_depth, 5.0);
    assert_eq!(ctx.blur, 2.0);
    assert_eq!(ctx.wrap_style, 1);
    assert!(ctx.underline);
    assert!(ctx.strikeout);
    assert!(ctx.clip_enabled);
    assert_eq!(ctx.clip_x1, 10.0);
    assert_eq!(ctx.clip_x2, 500.0);
}

#[test]
fn test_render_context_clone() {
    let ctx = RenderContext::default();
    let cloned = ctx.clone();
    assert_eq!(cloned.origin_x, 0.0);
    assert!(!cloned.clip_enabled);
    assert_eq!(cloned.wrap_style, 0);
    assert!(!cloned.underline);
    assert!(!cloned.strikeout);
}

#[test]
fn test_render_context_debug() {
    let ctx = RenderContext::default();
    let debug_str = format!("{:?}", ctx);
    assert!(debug_str.contains("origin_x"));
    assert!(debug_str.contains("clip_enabled"));
    assert!(debug_str.contains("wrap_style"));
    assert!(debug_str.contains("underline"));
    assert!(debug_str.contains("strikeout"));
}
