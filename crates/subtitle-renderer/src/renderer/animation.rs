use ass_parser::OverrideTag;
use crate::context::RenderContext;

pub(super) fn interpolate_move(x1: f32, y1: f32, x2: f32, y2: f32, t1: u64, t2: u64, elapsed: u64) -> (f32, f32) {
    if elapsed <= t1 {
        return (x1, y1);
    }
    if elapsed >= t2 {
        return (x2, y2);
    }
    let t = (elapsed - t1) as f32 / (t2 - t1).max(1) as f32;
    (x1 + (x2 - x1) * t, y1 + (y2 - y1) * t)
}

pub(super) fn compute_fad_alpha(elapsed: u64, total_duration: u64, fade_in: u64, fade_out: u64) -> f32 {
    if fade_in > 0 && elapsed < fade_in {
        return elapsed as f32 / fade_in as f32;
    }
    if fade_out > 0 && elapsed > total_duration.saturating_sub(fade_out) {
        let remaining = total_duration.saturating_sub(elapsed);
        return remaining as f32 / fade_out as f32;
    }
    1.0
}

#[allow(clippy::too_many_arguments)]
pub(super) fn compute_fade_complex(
    elapsed: u64,
    alpha_start: u8,
    alpha_mid: u8,
    alpha_end: u8,
    t1: u64, t2: u64, t3: u64, t4: u64,
) -> f32 {
    let (a1, a2, a3) = (
        (255 - alpha_start) as f32 / 255.0,
        (255 - alpha_mid) as f32 / 255.0,
        (255 - alpha_end) as f32 / 255.0,
    );

    if elapsed <= t1 {
        return a1;
    }
    if elapsed <= t2 {
        let t = (elapsed - t1) as f32 / (t2 - t1).max(1) as f32;
        return a1 + (a2 - a1) * t;
    }
    if elapsed <= t3 {
        return a2;
    }
    if elapsed <= t4 {
        let t = (elapsed - t3) as f32 / (t4 - t3).max(1) as f32;
        return a2 + (a3 - a2) * t;
    }
    a3
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_transform_tag(
    ctx: &mut RenderContext,
    inner_tag: &str,
    t1: u64, t2: u64, accel: f64,
    timestamp_ms: u64, event_start_ms: u64, _event_end_ms: u64,
    scale_x: f32, scale_y: f32,
) {
    let anim_start = event_start_ms + t1;
    let anim_end = if t2 > 0 { event_start_ms + t2 } else { u64::MAX };

    if timestamp_ms < anim_start || timestamp_ms > anim_end {
        return;
    }

    let progress = if anim_end == u64::MAX {
        1.0
    } else {
        let t = (timestamp_ms - anim_start) as f32 / (anim_end - anim_start).max(1) as f32;
        t.clamp(0.0, 1.0)
    };

    let p = if accel == 1.0 {
        progress
    } else {
        progress.powf(accel as f32)
    };

    let inner_tags = parse_override_block(inner_tag);
    for inner in &inner_tags {
        match inner {
            OverrideTag::FontSize(fs) => {
                let default_val = ctx.font_size;
                let target = *fs as f32 * scale_y;
                ctx.font_size = default_val + (target - default_val) * p;
            }
            OverrideTag::FontName(name)
                if p >= 0.5 => {
                    ctx.font_name = name.clone();
                }
            OverrideTag::Bold(b)
                if p >= 0.5 => {
                    ctx.bold = *b;
                }
            OverrideTag::Italic(i)
                if p >= 0.5 => {
                    ctx.italic = *i;
                }
            OverrideTag::PrimaryColor(c) => {
                let target = c.to_rgba();
                for (i, target_val) in target.iter().enumerate() {
                    ctx.primary_color[i] = lerp_u8(ctx.primary_color[i], *target_val, p);
                }
            }
            OverrideTag::SecondaryColor(c) => {
                let target = c.to_rgba();
                for (i, target_val) in target.iter().enumerate() {
                    ctx.secondary_color[i] = lerp_u8(ctx.secondary_color[i], *target_val, p);
                }
            }
            OverrideTag::OutlineColor(c) => {
                let target = c.to_rgba();
                for (i, target_val) in target.iter().enumerate() {
                    ctx.outline_color[i] = lerp_u8(ctx.outline_color[i], *target_val, p);
                }
            }
            OverrideTag::ShadowColor(c) => {
                let target = c.to_rgba();
                for (i, target_val) in target.iter().enumerate() {
                    ctx.shadow_color[i] = lerp_u8(ctx.shadow_color[i], *target_val, p);
                }
            }
            OverrideTag::Alpha { value } => {
                let target_a = 255 - *value;
                ctx.primary_color[3] = lerp_u8(ctx.primary_color[3], target_a, p);
                ctx.secondary_color[3] = lerp_u8(ctx.secondary_color[3], target_a, p);
                ctx.outline_color[3] = lerp_u8(ctx.outline_color[3], target_a, p);
                ctx.shadow_color[3] = lerp_u8(ctx.shadow_color[3], target_a, p);
            }
            OverrideTag::PrimaryAlpha { value } => {
                let target_a = 255 - *value;
                ctx.primary_color[3] = lerp_u8(ctx.primary_color[3], target_a, p);
            }
            OverrideTag::OutlineAlpha { value } => {
                let target_a = 255 - *value;
                ctx.outline_color[3] = lerp_u8(ctx.outline_color[3], target_a, p);
            }
            OverrideTag::ShadowAlpha { value } => {
                let target_a = 255 - *value;
                ctx.shadow_color[3] = lerp_u8(ctx.shadow_color[3], target_a, p);
            }
            OverrideTag::SecondaryAlpha { value } => {
                let target_a = 255 - *value;
                ctx.secondary_color[3] = lerp_u8(ctx.secondary_color[3], target_a, p);
            }
            OverrideTag::Border(w) => {
                ctx.outline_width = ctx.outline_width + (*w as f32 - ctx.outline_width) * p;
            }
            OverrideTag::Shadow(d) => {
                ctx.shadow_depth = ctx.shadow_depth + (*d as f32 - ctx.shadow_depth) * p;
            }
            OverrideTag::Blur(r) | OverrideTag::GaussianBlur(r) => {
                ctx.blur = ctx.blur + (*r as f32 - ctx.blur) * p;
            }
            OverrideTag::Spacing(s) => {
                ctx.spacing = ctx.spacing + (*s as f32 - ctx.spacing) * p;
            }
            OverrideTag::Scale { x, y } => {
                let target_x = *x as f32;
                let target_y = *y as f32;
                ctx.scale_x = ctx.scale_x + (target_x - ctx.scale_x) * p;
                ctx.scale_y = ctx.scale_y + (target_y - ctx.scale_y) * p;
            }
            OverrideTag::Rotation { x, y, z } => {
                ctx.rotation = ctx.rotation + (*z as f32 - ctx.rotation) * p;
                ctx.perspective_x = ctx.perspective_x + (*x as f32 - ctx.perspective_x) * p;
                ctx.perspective_y = ctx.perspective_y + (*y as f32 - ctx.perspective_y) * p;
            }
            OverrideTag::Shear { x, y } => {
                ctx.shear_x = ctx.shear_x + (*x as f32 - ctx.shear_x) * p;
                ctx.shear_y = ctx.shear_y + (*y as f32 - ctx.shear_y) * p;
            }
            OverrideTag::BorderX(w) => {
                ctx.outline_x_width = ctx.outline_x_width + (*w as f32 - ctx.outline_x_width) * p;
            }
            OverrideTag::BorderY(w) => {
                ctx.outline_y_width = ctx.outline_y_width + (*w as f32 - ctx.outline_y_width) * p;
            }
            OverrideTag::ShadowX(d) => {
                ctx.shadow_x = ctx.shadow_x + (*d as f32 - ctx.shadow_x) * p;
            }
            OverrideTag::ShadowY(d) => {
                ctx.shadow_y = ctx.shadow_y + (*d as f32 - ctx.shadow_y) * p;
            }
            OverrideTag::Origin { x, y } => {
                ctx.origin_x = ctx.origin_x + (*x as f32 * scale_x - ctx.origin_x) * p;
                ctx.origin_y = ctx.origin_y + (*y as f32 * scale_y - ctx.origin_y) * p;
            }
            OverrideTag::Underline(u)
                if p >= 0.5 => {
                    ctx.underline = *u;
                }
            OverrideTag::Strikeout(s)
                if p >= 0.5 => {
                    ctx.strikeout = *s;
                }
            OverrideTag::BoldWeight(w)
                if p >= 0.5 => {
                    ctx.bold = *w > 0;
                }
            OverrideTag::Pos { x, y } => {
                let target_x = *x as f32 * scale_x;
                let target_y = *y as f32 * scale_y;
                ctx.x = ctx.x + (target_x - ctx.x) * p;
                ctx.y = ctx.y + (target_y - ctx.y) * p;
            }
            _ => {}
        }
    }
}

pub(super) fn parse_override_block(text: &str) -> Vec<OverrideTag> {
    let mut tags = Vec::new();
    let mut current = String::new();
    let mut in_paren = false;

    for ch in text.chars() {
        match ch {
            '\\' if !in_paren => {
                if !current.is_empty() {
                    let tag_str = current.strip_prefix('\\').unwrap_or(&current);
                    if let Some(tag) = ass_parser::parse_override_tag(tag_str) {
                        tags.push(tag);
                    }
                    current.clear();
                }
                current.push(ch);
            }
            '(' => {
                current.push(ch);
                in_paren = true;
            }
            ')' => {
                current.push(ch);
                in_paren = false;
            }
            _ => {
                current.push(ch);
            }
        }
    }

    if !current.is_empty() {
        let tag_str = current.strip_prefix('\\').unwrap_or(&current);
        if let Some(tag) = ass_parser::parse_override_tag(tag_str) {
            tags.push(tag);
        }
    }

    tags
}

pub(super) fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::RenderContext;
    use ass_parser::{AssColor, OverrideTag};

    // ── interpolate_move ───────────────────────────────────────

    #[test]
    fn test_interpolate_move_before_t1() {
        let (x, y) = interpolate_move(0.0, 0.0, 100.0, 100.0, 100, 500, 0);
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn test_interpolate_move_after_t2() {
        let (x, y) = interpolate_move(0.0, 0.0, 100.0, 100.0, 100, 500, 600);
        assert_eq!(x, 100.0);
        assert_eq!(y, 100.0);
    }

    #[test]
    fn test_interpolate_move_at_t1() {
        let (x, y) = interpolate_move(0.0, 0.0, 100.0, 200.0, 100, 500, 100);
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn test_interpolate_move_at_t2() {
        let (x, y) = interpolate_move(0.0, 0.0, 100.0, 200.0, 100, 500, 500);
        assert_eq!(x, 100.0);
        assert_eq!(y, 200.0);
    }

    #[test]
    fn test_interpolate_move_midpoint() {
        let (x, y) = interpolate_move(0.0, 0.0, 100.0, 200.0, 0, 1000, 500);
        assert!((x - 50.0).abs() < 0.01);
        assert!((y - 100.0).abs() < 0.01);
    }

    #[test]
    fn test_interpolate_move_quarter() {
        let (x, y) = interpolate_move(0.0, 0.0, 100.0, 200.0, 0, 1000, 250);
        assert!((x - 25.0).abs() < 0.01);
        assert!((y - 50.0).abs() < 0.01);
    }

    #[test]
    fn test_interpolate_move_same_point() {
        let (x, y) = interpolate_move(50.0, 60.0, 50.0, 60.0, 0, 1000, 500);
        assert!((x - 50.0).abs() < 0.01);
        assert!((y - 60.0).abs() < 0.01);
    }

    // ── compute_fad_alpha ─────────────────────────────────────

    #[test]
    fn test_fad_alpha_no_fade() {
        assert_eq!(compute_fad_alpha(500, 1000, 0, 0), 1.0);
    }

    #[test]
    fn test_fad_alpha_fade_in_start() {
        let a = compute_fad_alpha(0, 1000, 500, 0);
        assert!(a < 0.01); // nearly transparent at start
    }

    #[test]
    fn test_fad_alpha_fade_in_mid() {
        let a = compute_fad_alpha(250, 1000, 500, 0);
        assert!((a - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_fad_alpha_fade_in_complete() {
        let a = compute_fad_alpha(500, 1000, 500, 0);
        assert!((a - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_fad_alpha_fade_out_start() {
        let a = compute_fad_alpha(500, 1000, 0, 500);
        assert!((a - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_fad_alpha_fade_out_mid() {
        let a = compute_fad_alpha(750, 1000, 0, 500);
        assert!((a - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_fad_alpha_fade_out_end() {
        let a = compute_fad_alpha(1000, 1000, 0, 500);
        assert!(a < 0.01);
    }

    #[test]
    fn test_fad_alpha_fade_in_and_out() {
        // fade_in=200, fade_out=300, total=1000
        assert!(compute_fad_alpha(0, 1000, 200, 300) < 0.01);   // start: transparent
        assert!((compute_fad_alpha(200, 1000, 200, 300) - 1.0).abs() < 0.01); // fade-in done
        assert!((compute_fad_alpha(500, 1000, 200, 300) - 1.0).abs() < 0.01); // middle: opaque
        assert!((compute_fad_alpha(850, 1000, 200, 300) - 0.5).abs() < 0.01); // fade-out mid
        assert!(compute_fad_alpha(1000, 1000, 200, 300) < 0.01); // end: transparent
    }

    // ── compute_fade_complex ──────────────────────────────────

    #[test]
    fn test_fade_complex_before_t1() {
        // alpha_start=0 (fully opaque in ASS), alpha_mid=128, alpha_end=255 (fully transparent)
        let a = compute_fade_complex(0, 0, 128, 255, 100, 200, 300, 400);
        assert!((a - 1.0).abs() < 0.02); // 255-0 / 255 = 1.0
    }

    #[test]
    fn test_fade_complex_between_t1_t2() {
        let a = compute_fade_complex(150, 0, 128, 255, 100, 200, 300, 400);
        // At midpoint: lerp from a1=1.0 to a2=(255-128)/255≈0.498
        assert!(a > 0.49 && a < 1.02);
    }

    #[test]
    fn test_fade_complex_between_t2_t3() {
        let a = compute_fade_complex(250, 0, 128, 255, 100, 200, 300, 400);
        // Holds at a2 = (255-128)/255 ≈ 0.498
        assert!((a - 0.498).abs() < 0.02);
    }

    #[test]
    fn test_fade_complex_between_t3_t4() {
        let a = compute_fade_complex(350, 0, 128, 255, 100, 200, 300, 400);
        // Lerping from a2≈0.498 to a3=0.0
        assert!(a > -0.01 && a < 0.51);
    }

    #[test]
    fn test_fade_complex_after_t4() {
        let a = compute_fade_complex(500, 0, 128, 255, 100, 200, 300, 400);
        // a3 = (255-255)/255 = 0.0
        assert!(a.abs() < 0.01);
    }

    #[test]
    fn test_fade_complex_all_opaque() {
        // alpha_start=alpha_mid=alpha_end=0 → all 1.0
        let a = compute_fade_complex(500, 0, 0, 0, 100, 200, 300, 400);
        assert!((a - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_fade_complex_all_transparent() {
        // alpha_start=alpha_mid=alpha_end=255 → all 0.0
        let a = compute_fade_complex(500, 255, 255, 255, 100, 200, 300, 400);
        assert!(a.abs() < 0.01);
    }

    // ── lerp_u8 ───────────────────────────────────────────────

    #[test]
    fn test_lerp_u8_start() {
        assert_eq!(lerp_u8(0, 255, 0.0), 0);
    }

    #[test]
    fn test_lerp_u8_end() {
        assert_eq!(lerp_u8(0, 255, 1.0), 255);
    }

    #[test]
    fn test_lerp_u8_mid() {
        let v = lerp_u8(0, 200, 0.5);
        assert_eq!(v, 100);
    }

    #[test]
    fn test_lerp_u8_same_value() {
        assert_eq!(lerp_u8(100, 100, 0.5), 100);
    }

    #[test]
    fn test_lerp_u8_clamp_high() {
        assert_eq!(lerp_u8(200, 255, 2.0), 255);
    }

    #[test]
    fn test_lerp_u8_clamp_low() {
        assert_eq!(lerp_u8(10, 200, -1.0), 0);
    }

    // ── parse_override_block ──────────────────────────────────

    #[test]
    fn test_parse_override_block_single_tag() {
        let tags = parse_override_block("\\fs20");
        assert_eq!(tags.len(), 1);
        match &tags[0] {
            OverrideTag::FontSize(v) => assert_eq!(*v, 20.0),
            _ => panic!("expected FontSize"),
        }
    }

    #[test]
    fn test_parse_override_block_multiple_tags() {
        let tags = parse_override_block("\\b1\\i1\\fs30");
        assert_eq!(tags.len(), 3);
        assert!(matches!(&tags[0], OverrideTag::Bold(true)));
        assert!(matches!(&tags[1], OverrideTag::Italic(true)));
        assert!(matches!(&tags[2], OverrideTag::FontSize(v) if *v == 30.0));
    }

    #[test]
    fn test_parse_override_block_with_parens() {
        // \\clip(10,20,30,40) — parens should not break parsing
        let tags = parse_override_block("\\bord2\\clip(10,20,30,40)\\shad3");
        assert_eq!(tags.len(), 3);
        assert!(matches!(&tags[0], OverrideTag::Border(v) if *v == 2.0));
        // clip is parsed via the regular ASS parser, not ass_parser::parse_override_tag
        assert!(matches!(&tags[1], OverrideTag::Clip{..}));
        assert!(matches!(&tags[2], OverrideTag::Shadow(v) if *v == 3.0));
    }

    #[test]
    fn test_parse_override_block_empty() {
        let tags = parse_override_block("");
        assert!(tags.is_empty());
    }

    #[test]
    fn test_parse_override_block_karaoke() {
        let tags = parse_override_block("\\k50\\kf100");
        assert_eq!(tags.len(), 2);
        assert!(matches!(&tags[0], OverrideTag::Karaoke { duration: 500, .. }));
        assert!(matches!(&tags[1], OverrideTag::Karaoke { duration: 1000, .. }));
    }

    // ── apply_transform_tag ───────────────────────────────────

    #[test]
    fn test_apply_transform_fontsize() {
        let mut ctx = RenderContext { font_size: 20.0, ..Default::default() };
        apply_transform_tag(&mut ctx, "\\fs40", 0, 1000, 1.0, 500, 0, 1000, 1.0, 1.0);
        assert!(ctx.font_size > 29.0 && ctx.font_size < 31.0);
    }

    #[test]
    fn test_apply_transform_accel() {
        let mut ctx1 = RenderContext::default();
        let mut ctx2 = RenderContext::default();
        // accel=1.0 (linear) vs accel=2.0 (decelerating)
        apply_transform_tag(&mut ctx1, "\\bord10", 0, 1000, 1.0, 500, 0, 1000, 100.0, 100.0);
        apply_transform_tag(&mut ctx2, "\\bord10", 0, 1000, 2.0, 500, 0, 1000, 100.0, 100.0);
        // accel=2.0 at 50% → 0.25 progress, linear at 50% → 0.5
        assert!(ctx2.outline_width < ctx1.outline_width);
    }

    #[test]
    fn test_apply_transform_outside_range() {
        let mut ctx = RenderContext { font_size: 20.0, ..Default::default() };
        apply_transform_tag(&mut ctx, "\\fs40", 500, 1000, 1.0, 200, 0, 1500, 100.0, 100.0);
        // Before t1=500 → no change
        assert_eq!(ctx.font_size, 20.0);
    }

    #[test]
    fn test_apply_transform_color() {
        let mut ctx = RenderContext { primary_color: [255, 0, 0, 255], ..Default::default() }; // red
        let red = AssColor::from_rgb(0, 0, 255); // blue in ASS format (BGR)
        apply_transform_tag(&mut ctx, &format!("\\1c{}", red.to_ass_hex()), 0, 1000, 1.0, 1000, 0, 1000, 100.0, 100.0);
        // At progress=1.0 → fully interpolated to target
        assert_eq!(ctx.primary_color[2], 255); // blue channel should be 255
        assert_eq!(ctx.primary_color[0], 0);   // red channel should be 0
    }

    #[test]
    fn test_apply_transform_scale() {
        let mut ctx = RenderContext { scale_x: 100.0, scale_y: 100.0, ..Default::default() };
        apply_transform_tag(&mut ctx, "\\fscx200", 0, 1000, 1.0, 500, 0, 1000, 100.0, 100.0);
        assert!(ctx.scale_x > 149.0 && ctx.scale_x < 151.0);
        assert_eq!(ctx.scale_y, 100.0); // y unchanged
    }

    // ── apply_transform_tag / Pos ─────────────────────────────

    #[test]
    fn test_apply_transform_pos_interpolation() {
        let mut ctx = RenderContext { x: 0.0, y: 0.0, ..Default::default() };
        // Interpolate from (0,0) to (1920,1080) at 50% progress (t=500, duration=1000)
        apply_transform_tag(&mut ctx, "\\pos(1920,1080)", 0, 1000, 1.0, 500, 0, 1000, 1.0, 1.0);
        assert!((ctx.x - 960.0).abs() < 1.0, "x should be ~960, got {}", ctx.x);
        assert!((ctx.y - 540.0).abs() < 1.0, "y should be ~540, got {}", ctx.y);
    }

    #[test]
    fn test_apply_transform_pos_before_start() {
        let mut ctx = RenderContext { x: 100.0, y: 200.0, ..Default::default() };
        // Before t1=500, no interpolation should happen
        apply_transform_tag(&mut ctx, "\\pos(1920,1080)", 500, 1000, 1.0, 100, 0, 2000, 1.0, 1.0);
        assert_eq!(ctx.x, 100.0);
        assert_eq!(ctx.y, 200.0);
    }

    #[test]
    fn test_apply_transform_pos_after_end() {
        let mut ctx = RenderContext { x: 100.0, y: 200.0, ..Default::default() };
        // At t2=1000 (the end of animation), should be at target
        apply_transform_tag(&mut ctx, "\\pos(1920,1080)", 0, 1000, 1.0, 1000, 0, 1000, 1.0, 1.0);
        assert!((ctx.x - 1920.0).abs() < 1.0);
        assert!((ctx.y - 1080.0).abs() < 1.0);
    }
}
