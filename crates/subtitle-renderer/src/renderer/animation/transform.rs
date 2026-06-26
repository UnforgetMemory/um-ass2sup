use crate::context::RenderContext;
use ass_core::OverrideTag;

/// Apply a `\t(tag, t1, t2, accel)` transform animation to a RenderContext.
#[allow(clippy::too_many_arguments)]
pub fn apply_transform_tag(
    ctx: &mut RenderContext,
    tag: &str,
    t1: u64,
    t2: u64,
    accel: f64,
    timestamp_ms: u64,
    event_start_ms: u64,
    event_end_ms: u64,
    scale_x: f32,
    scale_y: f32,
) {
    let event_duration = event_end_ms.saturating_sub(event_start_ms);
    let absolute_t1 = event_start_ms + t1.min(event_duration);
    let absolute_t2 = event_start_ms + t2.min(event_duration);
    let elapsed = timestamp_ms;
    let anim_duration = absolute_t2.saturating_sub(absolute_t1);

    if anim_duration == 0 {
        // Apply start state directly
        apply_tag_state(ctx, tag, false, scale_x, scale_y);
        return;
    }

    let raw_progress = if elapsed <= absolute_t1 {
        0.0
    } else if elapsed >= absolute_t2 {
        1.0
    } else {
        (elapsed - absolute_t1) as f64 / anim_duration as f64
    };

    // Apply acceleration
    let progress = if accel == 1.0 {
        raw_progress as f32
    } else if accel > 0.0 {
        let a = accel;
        // Accelerating: starts slow, ends fast
        (raw_progress * (1.0 - a) + raw_progress.powf(1.0 + a * 2.0) * a) as f32
    } else {
        let a = -accel;
        // Decelerating: starts fast, ends slow
        (raw_progress * (1.0 - a) + raw_progress.powf(1.0 / (1.0 + a * 2.0)) * a) as f32
    };

    // Parse the inner tags and interpolate
    let inner_tags = parse_override_block(tag);
    let start_ctx = {
        let mut c = ctx.clone();
        apply_tag_state(&mut c, tag, false, scale_x, scale_y);
        c
    };
    let end_ctx = {
        let mut c = ctx.clone();
        apply_tag_state(&mut c, tag, true, scale_x, scale_y);
        c
    };

    // Interpolate numeric fields
    ctx.font_size = start_ctx.font_size + (end_ctx.font_size - start_ctx.font_size) * progress;
    ctx.rotation = start_ctx.rotation + (end_ctx.rotation - start_ctx.rotation) * progress;
    ctx.scale_x = start_ctx.scale_x + (end_ctx.scale_x - start_ctx.scale_x) * progress;
    ctx.scale_y = start_ctx.scale_y + (end_ctx.scale_y - start_ctx.scale_y) * progress;
    ctx.spacing = start_ctx.spacing + (end_ctx.spacing - start_ctx.spacing) * progress;
    ctx.outline_width =
        start_ctx.outline_width + (end_ctx.outline_width - start_ctx.outline_width) * progress;
    ctx.shadow_depth =
        start_ctx.shadow_depth + (end_ctx.shadow_depth - start_ctx.shadow_depth) * progress;
    ctx.blur = start_ctx.blur + (end_ctx.blur - start_ctx.blur) * progress;
    ctx.shear_x = start_ctx.shear_x + (end_ctx.shear_x - start_ctx.shear_x) * progress;
    ctx.shear_y = start_ctx.shear_y + (end_ctx.shear_y - start_ctx.shear_y) * progress;
    ctx.perspective_x =
        start_ctx.perspective_x + (end_ctx.perspective_x - start_ctx.perspective_x) * progress;
    ctx.perspective_y =
        start_ctx.perspective_y + (end_ctx.perspective_y - start_ctx.perspective_y) * progress;

    // Interpolate colors if both start and end have them specified
    let color_tags: Vec<&OverrideTag> = inner_tags
        .iter()
        .filter(|t| {
            matches!(
                t,
                OverrideTag::PrimaryColor(_)
                    | OverrideTag::SecondaryColor(_)
                    | OverrideTag::OutlineColor(_)
                    | OverrideTag::ShadowColor(_)
            )
        })
        .collect();
    if !color_tags.is_empty() {
        for tag in &color_tags {
            match tag {
                OverrideTag::PrimaryColor(_c) => {
                    let sc = start_ctx.primary_color;
                    let ec = end_ctx.primary_color;
                    ctx.primary_color = lerp_color(sc, ec, progress);
                }
                OverrideTag::SecondaryColor(_c) => {
                    let sc = start_ctx.secondary_color;
                    let ec = end_ctx.secondary_color;
                    ctx.secondary_color = lerp_color(sc, ec, progress);
                }
                OverrideTag::OutlineColor(_c) => {
                    let sc = start_ctx.outline_color;
                    let ec = end_ctx.outline_color;
                    ctx.outline_color = lerp_color(sc, ec, progress);
                }
                OverrideTag::ShadowColor(_c) => {
                    let sc = start_ctx.shadow_color;
                    let ec = end_ctx.shadow_color;
                    ctx.shadow_color = lerp_color(sc, ec, progress);
                }
                _ => {}
            }
        }
    }
}

/// Apply the start or end state of a transform tag.
fn apply_tag_state(ctx: &mut RenderContext, tag: &str, is_end: bool, scale_x: f32, scale_y: f32) {
    use ass_core::OverrideTag::*;
    let inner_tags = parse_override_block(tag);
    for inner in &inner_tags {
        match inner {
            FontSize(fs) => ctx.font_size = *fs as f32 * scale_y,
            FontName(n) => ctx.font_name = n.clone(),
            Bold(b) => ctx.bold = *b,
            Italic(i) => ctx.italic = *i,
            PrimaryColor(c) => ctx.primary_color = c.to_rgba(),
            SecondaryColor(c) => ctx.secondary_color = c.to_rgba(),
            OutlineColor(c) => ctx.outline_color = c.to_rgba(),
            ShadowColor(c) => ctx.shadow_color = c.to_rgba(),
            Alpha { value } => {
                let a = 255 - *value;
                ctx.primary_color[3] = a;
                ctx.secondary_color[3] = a;
                ctx.outline_color[3] = a;
                ctx.shadow_color[3] = a;
            }
            Border { x: w, .. } => ctx.outline_width = *w as f32,
            Shadow { x: d, .. } => ctx.shadow_depth = *d as f32,
            Blur(r) | GaussianBlur(r) => ctx.blur = *r as f32,
            Spacing(s) => ctx.spacing = *s as f32,
            Scale { x, y } => {
                ctx.scale_x = *x as f32;
                ctx.scale_y = *y as f32;
            }
            ScaleReset => {
                ctx.scale_x = 100.0;
                ctx.scale_y = 100.0;
            }
            Rotation { x, y, z } => {
                ctx.rotation = *z as f32;
                if is_end {
                    ctx.perspective_x = *x as f32;
                    ctx.perspective_y = *y as f32;
                }
            }
            Shear { x, y } => {
                ctx.shear_x = *x as f32;
                ctx.shear_y = *y as f32;
            }
            Clip { x1, y1, x2, y2 } => {
                ctx.clip_x1 = *x1 as f32 * scale_x;
                ctx.clip_y1 = *y1 as f32 * scale_y;
                ctx.clip_x2 = *x2 as f32 * scale_x;
                ctx.clip_y2 = *y2 as f32 * scale_y;
                ctx.clip_enabled = true;
            }
            Pos { x, y } => {
                ctx.x = *x as f32 * scale_x;
                ctx.y = *y as f32 * scale_y;
            }
            Charset(c) => ctx.font_charset = *c,
            _ => {}
        }
    }
}

/// Parse an override block string into individual override tags.
/// Handles nested parentheses (e.g., `\t(\pos(960,540),0,3000,1)`).
pub fn parse_override_block(text: &str) -> Vec<OverrideTag> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut paren_depth: usize = 0;
    for ch in text.chars() {
        match ch {
            '\\' if paren_depth == 0 => {
                if !current.is_empty() {
                    parts.push(current);
                }
                current = String::from("\\");
            }
            '(' => {
                paren_depth += 1;
                current.push(ch);
            }
            ')' => {
                paren_depth = paren_depth.saturating_sub(1);
                current.push(ch);
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        parts.push(current);
    }
    // Parse each part
    let mut tags = Vec::new();
    for part in parts {
        if let Some(tag) = ass_core::override_tag::parse_one_tag(&part) {
            tags.push(tag);
        }
    }
    tags
}

/// Linearly interpolate two RGBA color arrays.
fn lerp_color(a: [u8; 4], b: [u8; 4], t: f32) -> [u8; 4] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
        (a[3] as f32 + (b[3] as f32 - a[3] as f32) * t) as u8,
    ]
}
