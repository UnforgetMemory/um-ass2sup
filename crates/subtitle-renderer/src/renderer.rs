use ass_parser::{AssFile, Event, OverrideTag, Style, Timestamp};
use tiny_skia::Pixmap;

use crate::context::{RenderConfig, RenderContext, RenderedFrame};
use crate::effects;
use crate::font::FontManager;
use crate::karaoke::{KaraokePhase, KaraokeRenderer};
use crate::rasterizer::Rasterizer;
use crate::shaper::Shaper;
use crate::transform::AffineTransform;

pub struct Renderer {
    config: RenderConfig,
    font_manager: FontManager,
}

impl Renderer {
    pub fn new(config: RenderConfig) -> Self {
        let mut fm = FontManager::new();
        fm.load_system_fonts();
        Self {
            config,
            font_manager: fm,
        }
    }

    pub fn font_manager(&self) -> &FontManager {
        &self.font_manager
    }

    pub fn font_manager_mut(&mut self) -> &mut FontManager {
        &mut self.font_manager
    }

    pub fn render_ass(&self, ass: &AssFile, timestamp_ms: u64) -> Option<RenderedFrame> {
        let ts = Timestamp::from_ms(timestamp_ms);
        let mut pixmap = Pixmap::new(self.config.width, self.config.height)?;

        for event in ass.dialogue_events() {
            if !event.is_visible_at(ts) {
                continue;
            }

            let style = ass.find_style(&event.style_name).cloned().unwrap_or_default();
            let event_start = event.start.as_ms();
            let event_end = event.end.as_ms();
            let ctx = self.build_context(event, &style, timestamp_ms, event_start, event_end);

            self.render_event(&mut pixmap, event, &ctx, timestamp_ms, event_start);
        }

        Some(RenderedFrame {
            pts_ms: timestamp_ms,
            duration_ms: 0,
            width: self.config.width,
            height: self.config.height,
            bitmap: pixmap.data().to_vec(),
        })
    }

    pub fn build_context(
        &self,
        event: &Event,
        style: &Style,
        timestamp_ms: u64,
        event_start_ms: u64,
        event_end_ms: u64,
    ) -> RenderContext {
        let mut ctx = RenderContext {
            font_name: style.font_name.clone(),
            font_size: style.font_size as f32,
            primary_color: style.primary_color.to_rgba(),
            secondary_color: style.secondary_color.to_rgba(),
            outline_color: style.outline_color.to_rgba(),
            shadow_color: style.shadow_color.to_rgba(),
            bold: style.bold,
            italic: style.italic,
            outline_width: style.outline_width as f32,
            shadow_depth: style.shadow_depth as f32,
            alignment: style.alignment,
            margin_l: event.margin_l as f32,
            margin_r: event.margin_r as f32,
            margin_v: event.margin_v as f32,
            ..Default::default()
        };

        let res_x = self.config.script_width as f32;
        let res_y = self.config.script_height as f32;
        let scale_x = self.config.width as f32 / res_x;
        let scale_y = self.config.height as f32 / res_y;
        ctx.margin_l = ctx.margin_l * scale_x;
        ctx.margin_r = ctx.margin_r * scale_x;
        ctx.margin_v = ctx.margin_v * scale_y;
        ctx.font_size = ctx.font_size * self.config.height as f32 / res_y;

        let mut has_pos = false;
        let mut has_move = false;
        let mut move_x2 = 0.0f32;
        let mut move_y2 = 0.0f32;
        let mut move_t1 = 0u64;
        let mut move_t2 = 0u64;
        let mut has_fad = false;
        let mut fad_in = 0u64;
        let mut fad_out = 0u64;
        let mut has_fade_complex = false;
        let mut fade_params = (0u8, 0u8, 0u8, 0u64, 0u64, 0u64, 0u64);

        for tag in &event.override_tags {
            match tag {
                OverrideTag::FontSize(fs) => ctx.font_size = *fs as f32 * scale_y,
                OverrideTag::FontName(name) => ctx.font_name = name.clone(),
                OverrideTag::Bold(b) => ctx.bold = *b,
                OverrideTag::BoldWeight(w) => ctx.bold = *w >= 700,
                OverrideTag::Italic(i) => ctx.italic = *i,
                OverrideTag::Underline(u) => ctx.underline = *u,
                OverrideTag::Strikeout(s) => ctx.strikeout = *s,
                OverrideTag::PrimaryColor(c) => ctx.primary_color = c.to_rgba(),
                OverrideTag::SecondaryColor(c) => ctx.secondary_color = c.to_rgba(),
                OverrideTag::OutlineColor(c) => ctx.outline_color = c.to_rgba(),
                OverrideTag::ShadowColor(c) => ctx.shadow_color = c.to_rgba(),
                OverrideTag::Alpha { value } => {
                    let a = 255 - *value;
                    ctx.primary_color[3] = a;
                    ctx.secondary_color[3] = a;
                    ctx.outline_color[3] = a;
                    ctx.shadow_color[3] = a;
                }
                OverrideTag::PrimaryAlpha { value } => ctx.primary_color[3] = 255 - *value,
                OverrideTag::SecondaryAlpha { value } => ctx.secondary_color[3] = 255 - *value,
                OverrideTag::OutlineAlpha { value } => ctx.outline_color[3] = 255 - *value,
                OverrideTag::ShadowAlpha { value } => ctx.shadow_color[3] = 255 - *value,
                OverrideTag::Border(w) => ctx.outline_width = *w as f32,
                OverrideTag::BorderX(w) => ctx.outline_width = *w as f32,
                OverrideTag::BorderY(w) => ctx.outline_width = *w as f32,
                OverrideTag::Shadow(d) => ctx.shadow_depth = *d as f32,
                OverrideTag::ShadowX(d) => ctx.shadow_depth = *d as f32,
                OverrideTag::ShadowY(d) => ctx.shadow_depth = *d as f32,
                OverrideTag::Blur(r) | OverrideTag::GaussianBlur(r) => ctx.blur = *r as f32,
                OverrideTag::Spacing(s) => ctx.spacing = *s as f32,
                OverrideTag::Scale { x, y } => {
                    ctx.scale_x = *x as f32;
                    ctx.scale_y = *y as f32;
                }
                OverrideTag::Rotation { x, y, z } => {
                    ctx.rotation = *z as f32;
                    ctx.origin_x = *x as f32;
                    ctx.origin_y = *y as f32;
                }
                OverrideTag::Origin { x, y } => {
                    ctx.origin_x = *x as f32 * scale_x;
                    ctx.origin_y = *y as f32 * scale_y;
                }
                OverrideTag::Shear { x, y } => {
                    ctx.shear_x = *x as f32;
                    ctx.shear_y = *y as f32;
                }
                OverrideTag::Alignment(a) => ctx.alignment = *a,
                OverrideTag::AlignmentNumpad(a) => ctx.alignment = *a,
                OverrideTag::WrapStyle(w) => ctx.wrap_style = *w,
                OverrideTag::Pos { x, y } => {
                    ctx.x = *x as f32 * scale_x;
                    ctx.y = *y as f32 * scale_y;
                    has_pos = true;
                }
                OverrideTag::Move { x1, y1, x2, y2, t1, t2 } => {
                    ctx.x = *x1 as f32 * scale_x;
                    ctx.y = *y1 as f32 * scale_y;
                    move_x2 = *x2 as f32 * scale_x;
                    move_y2 = *y2 as f32 * scale_y;
                    move_t1 = *t1 as u64;
                    move_t2 = *t2 as u64;
                    has_move = true;
                    has_pos = true;
                }
                OverrideTag::Fade { duration_in, duration_out } => {
                    fad_in = *duration_in;
                    fad_out = *duration_out;
                    has_fad = true;
                }
                OverrideTag::FadeComplex { alpha_start, alpha_mid, alpha_end, t1, t2, t3, t4 } => {
                    fade_params = (*alpha_start, *alpha_mid, *alpha_end, *t1, *t2, *t3, *t4);
                    has_fade_complex = true;
                }
                OverrideTag::Clip { x1, y1, x2, y2 } => {
                    ctx.clip_x1 = *x1 as f32 * scale_x;
                    ctx.clip_y1 = *y1 as f32 * scale_y;
                    ctx.clip_x2 = *x2 as f32 * scale_x;
                    ctx.clip_y2 = *y2 as f32 * scale_y;
                    ctx.clip_enabled = true;
                    ctx.clip_inverse = false;
                }
                OverrideTag::ClipInverse { x1, y1, x2, y2 } => {
                    ctx.clip_x1 = *x1 as f32 * scale_x;
                    ctx.clip_y1 = *y1 as f32 * scale_y;
                    ctx.clip_x2 = *x2 as f32 * scale_x;
                    ctx.clip_y2 = *y2 as f32 * scale_y;
                    ctx.clip_enabled = true;
                    ctx.clip_inverse = true;
                }
                OverrideTag::Transform { tag: inner_tag, t1, t2, accel } => {
                    apply_transform_tag(
                        &mut ctx, inner_tag,
                        *t1, *t2, *accel,
                        timestamp_ms, event_start_ms, event_end_ms,
                        scale_x, scale_y,
                    );
                }
                OverrideTag::Reset(style_name) => {
                    if style_name.is_empty() {
                        ctx.font_name = style.font_name.clone();
                        ctx.font_size = style.font_size as f32 * scale_y;
                        ctx.bold = style.bold;
                        ctx.italic = style.italic;
                        ctx.primary_color = style.primary_color.to_rgba();
                        ctx.secondary_color = style.secondary_color.to_rgba();
                        ctx.outline_color = style.outline_color.to_rgba();
                        ctx.shadow_color = style.shadow_color.to_rgba();
                        ctx.outline_width = style.outline_width as f32;
                        ctx.shadow_depth = style.shadow_depth as f32;
                        ctx.alignment = style.alignment;
                    }
                }
                _ => {}
            }
        }

        if has_move {
            let elapsed = timestamp_ms.saturating_sub(event_start_ms);
            let (nx, ny) = interpolate_move(
                ctx.x, ctx.y, move_x2, move_y2,
                move_t1, move_t2, elapsed,
            );
            ctx.x = nx;
            ctx.y = ny;
        }

        if has_fad {
            let elapsed = timestamp_ms.saturating_sub(event_start_ms);
            let duration = event_end_ms.saturating_sub(event_start_ms);
            ctx.alpha_multiplier = compute_fad_alpha(elapsed, duration, fad_in, fad_out);
        } else if has_fade_complex {
            let elapsed = timestamp_ms.saturating_sub(event_start_ms);
            let (a1, a2, a3, t1, t2, t3, t4) = fade_params;
            ctx.alpha_multiplier = compute_fade_complex(elapsed, a1, a2, a3, t1, t2, t3, t4);
        }

        if !has_pos {
            let (ax, ay) = alignment_to_pos(ctx.alignment);
            ctx.x = ctx.margin_l + ax * (self.config.width as f32 - ctx.margin_l - ctx.margin_r);
            ctx.y = ctx.margin_v + ay * (self.config.height as f32 - ctx.margin_v * 2.0);
        }

        ctx
    }

    fn render_event(&self, pixmap: &mut Pixmap, event: &Event, ctx: &RenderContext, timestamp_ms: u64, event_start_ms: u64) {
        let plain_text = strip_override_blocks(&event.text);
        if plain_text.is_empty() {
            return;
        }

        let font_id = match self.font_manager.query_with_fallback(&ctx.font_name, ctx.bold, ctx.italic) {
            Some(id) => id,
            None => return,
        };

        if event.has_karaoke() && !event.karaoke_segments.is_empty() {
            self.render_karaoke(pixmap, event, ctx, font_id, timestamp_ms, event_start_ms);
            return;
        }

        let shaper = Shaper::new(&self.font_manager);
        let shaped = match shaper.shape(&plain_text, font_id, ctx.font_size) {
            Ok(s) => s,
            Err(_) => return,
        };

        let w = pixmap.width();
        let h = pixmap.height();
        let mut layer = Pixmap::new(w, h).unwrap();

        let mut x = ctx.x;
        for glyph in &shaped.glyphs {
            Rasterizer::rasterize_glyph(&mut layer, &self.font_manager, font_id, glyph, x, ctx.y, ctx);
            x += glyph.x_advance + ctx.spacing;
        }

        if ctx.blur > 0.0 {
            effects::apply_gaussian_blur(&mut layer, ctx.blur);
        }

        if ctx.shadow_depth > 0.0 {
            let layer_data = layer.data().to_vec();
            let shadow_layer = effects::apply_shadow(
                &layer_data,
                w,
                h,
                ctx.shadow_depth,
                ctx.shadow_depth,
                ctx.blur,
                ctx.shadow_color,
            );
            let mut shadow_pixmap = Pixmap::new(w, h).unwrap();
            shadow_pixmap.data_mut().copy_from_slice(&shadow_layer);
            effects::composite_over(layer.data_mut(), shadow_pixmap.data(), w, h);
        }

        let transform = AffineTransform::rotate_at(ctx.rotation, ctx.origin_x, ctx.origin_y)
            .then(&AffineTransform::scale(ctx.scale_x / 100.0, ctx.scale_y / 100.0))
            .then(&AffineTransform::shear(ctx.shear_x, ctx.shear_y));

        let final_data = if transform.is_identity() {
            layer.data().to_vec()
        } else {
            transform.apply_to_pixmap(layer.data(), w, h, w, h)
        };

        if ctx.clip_enabled {
            apply_clip_mask(&mut final_data.clone(), w, h, ctx);
            effects::composite_over(pixmap.data_mut(), &final_data, w, h);
        } else {
            if ctx.alpha_multiplier < 0.999 {
                let mut alpha_data = final_data;
                apply_alpha_multiplier(&mut alpha_data, ctx.alpha_multiplier);
                effects::composite_over(pixmap.data_mut(), &alpha_data, w, h);
            } else {
                effects::composite_over(pixmap.data_mut(), &final_data, w, h);
            }
        }
    }

    fn render_karaoke(
        &self,
        pixmap: &mut Pixmap,
        event: &Event,
        ctx: &RenderContext,
        font_id: fontdb::ID,
        timestamp_ms: u64,
        event_start_ms: u64,
    ) {
        let w = pixmap.width();
        let h = pixmap.height();

        let syllables = KaraokeRenderer::compute_syllable_states(
            &event.karaoke_segments,
            event_start_ms,
            timestamp_ms,
        );

        let shaper = Shaper::new(&self.font_manager);

        let mut bg_layer = Pixmap::new(w, h).unwrap();
        let mut fg_layer = Pixmap::new(w, h).unwrap();

        for syllable in &syllables {
            if syllable.text.is_empty() {
                continue;
            }

            let is_done = matches!(syllable.phase, KaraokePhase::Done);
            let is_active = matches!(syllable.phase, KaraokePhase::Active { .. });

            let mut sy_ctx = ctx.clone();
            if is_done || is_active {
                sy_ctx.primary_color = ctx.primary_color;
            } else {
                sy_ctx.primary_color = ctx.secondary_color;
            }

            if let Ok(shaped) = shaper.shape(&syllable.text, font_id, ctx.font_size) {
                let mut sx = ctx.x;
                for glyph in &shaped.glyphs {
                    if is_active {
                        Rasterizer::rasterize_glyph(&mut fg_layer, &self.font_manager, font_id, glyph, sx, ctx.y, &sy_ctx);
                    } else {
                        Rasterizer::rasterize_glyph(&mut bg_layer, &self.font_manager, font_id, glyph, sx, ctx.y, &sy_ctx);
                    }
                    sx += glyph.x_advance + ctx.spacing;
                }
            }
        }

        if ctx.blur > 0.0 {
            effects::apply_gaussian_blur(&mut bg_layer, ctx.blur);
            effects::apply_gaussian_blur(&mut fg_layer, ctx.blur);
        }

        effects::composite_over(pixmap.data_mut(), bg_layer.data(), w, h);
        effects::composite_over(pixmap.data_mut(), fg_layer.data(), w, h);
    }
}

fn interpolate_move(x1: f32, y1: f32, x2: f32, y2: f32, t1: u64, t2: u64, elapsed: u64) -> (f32, f32) {
    if elapsed <= t1 {
        return (x1, y1);
    }
    if elapsed >= t2 {
        return (x2, y2);
    }
    let t = (elapsed - t1) as f32 / (t2 - t1).max(1) as f32;
    (x1 + (x2 - x1) * t, y1 + (y2 - y1) * t)
}

fn compute_fad_alpha(elapsed: u64, total_duration: u64, fade_in: u64, fade_out: u64) -> f32 {
    if fade_in > 0 && elapsed < fade_in {
        return elapsed as f32 / fade_in as f32;
    }
    if fade_out > 0 && elapsed > total_duration.saturating_sub(fade_out) {
        let remaining = total_duration.saturating_sub(elapsed);
        return remaining as f32 / fade_out as f32;
    }
    1.0
}

fn compute_fade_complex(
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

fn apply_transform_tag(
    ctx: &mut RenderContext,
    inner_tag: &str,
    t1: u64, t2: u64, accel: f64,
    timestamp_ms: u64, event_start_ms: u64, _event_end_ms: u64,
    _scale_x: f32, scale_y: f32,
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
            OverrideTag::FontName(name) => {
                if p >= 0.5 {
                    ctx.font_name = name.clone();
                }
            }
            OverrideTag::Bold(b) => {
                if p >= 0.5 {
                    ctx.bold = *b;
                }
            }
            OverrideTag::Italic(i) => {
                if p >= 0.5 {
                    ctx.italic = *i;
                }
            }
            OverrideTag::PrimaryColor(c) => {
                let target = c.to_rgba();
                for i in 0..4 {
                    ctx.primary_color[i] = lerp_u8(ctx.primary_color[i], target[i], p);
                }
            }
            OverrideTag::SecondaryColor(c) => {
                let target = c.to_rgba();
                for i in 0..4 {
                    ctx.secondary_color[i] = lerp_u8(ctx.secondary_color[i], target[i], p);
                }
            }
            OverrideTag::OutlineColor(c) => {
                let target = c.to_rgba();
                for i in 0..4 {
                    ctx.outline_color[i] = lerp_u8(ctx.outline_color[i], target[i], p);
                }
            }
            OverrideTag::ShadowColor(c) => {
                let target = c.to_rgba();
                for i in 0..4 {
                    ctx.shadow_color[i] = lerp_u8(ctx.shadow_color[i], target[i], p);
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
                ctx.origin_x = ctx.origin_x + (*x as f32 - ctx.origin_x) * p;
                ctx.origin_y = ctx.origin_y + (*y as f32 - ctx.origin_y) * p;
            }
            _ => {}
        }
    }
}

fn parse_override_block(text: &str) -> Vec<OverrideTag> {
    let mut tags = Vec::new();
    let mut current = String::new();
    let mut in_paren = false;

    for ch in text.chars() {
        match ch {
            '\\' if !in_paren => {
                if !current.is_empty() {
                    if let Some(tag) = parse_single_tag_from_str(&current) {
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
        if let Some(tag) = parse_single_tag_from_str(&current) {
            tags.push(tag);
        }
    }

    tags
}

fn parse_single_tag_from_str(raw: &str) -> Option<OverrideTag> {
    let s = raw.strip_prefix('\\').unwrap_or(raw);
    if s.is_empty() {
        return None;
    }

    if let Some(rest) = s.strip_prefix("fs") {
        if let Ok(v) = rest.parse::<f64>() {
            return Some(OverrideTag::FontSize(v));
        }
    }
    if let Some(rest) = s.strip_prefix("fn") {
        return Some(OverrideTag::FontName(rest.to_string()));
    }
    if s == "b1" { return Some(OverrideTag::Bold(true)); }
    if s == "b0" { return Some(OverrideTag::Bold(false)); }
    if s == "i1" { return Some(OverrideTag::Italic(true)); }
    if s == "i0" { return Some(OverrideTag::Italic(false)); }
    if let Some(rest) = s.strip_prefix("bord") {
        if let Ok(v) = rest.parse::<f64>() {
            return Some(OverrideTag::Border(v));
        }
    }
    if let Some(rest) = s.strip_prefix("shad") {
        if let Ok(v) = rest.parse::<f64>() {
            return Some(OverrideTag::Shadow(v));
        }
    }
    if let Some(rest) = s.strip_prefix("fscx") {
        if let Ok(v) = rest.parse::<f64>() {
            return Some(OverrideTag::Scale { x: v, y: 100.0 });
        }
    }
    if let Some(rest) = s.strip_prefix("fscy") {
        if let Ok(v) = rest.parse::<f64>() {
            return Some(OverrideTag::Scale { x: 100.0, y: v });
        }
    }
    if let Some(rest) = s.strip_prefix("frz") {
        if let Ok(v) = rest.parse::<f64>() {
            return Some(OverrideTag::Rotation { x: 0.0, y: 0.0, z: v });
        }
    }
    if let Some(rest) = s.strip_prefix("k") {
        if let Ok(v) = rest.parse::<u64>() {
            return Some(OverrideTag::Karaoke {
                style: ass_parser::KaraokeStyle::Instant,
                duration: v * 10,
            });
        }
    }
    if let Some(rest) = s.strip_prefix("kf") {
        if let Ok(v) = rest.parse::<u64>() {
            return Some(OverrideTag::Karaoke {
                style: ass_parser::KaraokeStyle::Fill,
                duration: v * 10,
            });
        }
    }
    if let Some(rest) = s.strip_prefix("an") {
        if let Ok(v) = rest.parse::<u8>() {
            return Some(OverrideTag::AlignmentNumpad(v));
        }
    }
    if let Some(rest) = s.strip_prefix("1c&H") {
        let hex = rest.strip_suffix('&').unwrap_or(rest);
        if let Ok(c) = parse_ass_color(hex) {
            return Some(OverrideTag::PrimaryColor(c));
        }
    }
    if let Some(rest) = s.strip_prefix("2c&H") {
        let hex = rest.strip_suffix('&').unwrap_or(rest);
        if let Ok(c) = parse_ass_color(hex) {
            return Some(OverrideTag::SecondaryColor(c));
        }
    }
    if let Some(rest) = s.strip_prefix("3c&H") {
        let hex = rest.strip_suffix('&').unwrap_or(rest);
        if let Ok(c) = parse_ass_color(hex) {
            return Some(OverrideTag::OutlineColor(c));
        }
    }
    if let Some(rest) = s.strip_prefix("4c&H") {
        let hex = rest.strip_suffix('&').unwrap_or(rest);
        if let Ok(c) = parse_ass_color(hex) {
            return Some(OverrideTag::ShadowColor(c));
        }
    }
    if let Some(rest) = s.strip_prefix("clip(").and_then(|r| r.strip_suffix(')')) {
        let nums: Vec<f64> = rest.split(',')
            .filter_map(|n| n.trim().parse().ok())
            .collect();
        if nums.len() == 4 {
            return Some(OverrideTag::Clip { x1: nums[0], y1: nums[1], x2: nums[2], y2: nums[3] });
        }
    }
    if let Some(rest) = s.strip_prefix("iclip(").and_then(|r| r.strip_suffix(')')) {
        let nums: Vec<f64> = rest.split(',')
            .filter_map(|n| n.trim().parse().ok())
            .collect();
        if nums.len() == 4 {
            return Some(OverrideTag::ClipInverse { x1: nums[0], y1: nums[1], x2: nums[2], y2: nums[3] });
        }
    }

    None
}

fn parse_ass_color(hex: &str) -> Result<ass_parser::AssColor, ()> {
    let clean = hex.trim_start_matches("0x").trim_start_matches("0X");
    match clean.len() {
        6 => {
            let b = u8::from_str_radix(&clean[0..2], 16).map_err(|_| ())?;
            let g = u8::from_str_radix(&clean[2..4], 16).map_err(|_| ())?;
            let r = u8::from_str_radix(&clean[4..6], 16).map_err(|_| ())?;
            Ok(ass_parser::AssColor { alpha: 0, blue: b, green: g, red: r })
        }
        8 => {
            let a = u8::from_str_radix(&clean[0..2], 16).map_err(|_| ())?;
            let b = u8::from_str_radix(&clean[2..4], 16).map_err(|_| ())?;
            let g = u8::from_str_radix(&clean[4..6], 16).map_err(|_| ())?;
            let r = u8::from_str_radix(&clean[6..8], 16).map_err(|_| ())?;
            Ok(ass_parser::AssColor { alpha: a, blue: b, green: g, red: r })
        }
        _ => Err(()),
    }
}

fn apply_alpha_multiplier(data: &mut [u8], alpha: f32) {
    let factor = alpha.clamp(0.0, 1.0);
    for i in (3..data.len()).step_by(4) {
        data[i] = (data[i] as f32 * factor) as u8;
    }
}

fn apply_clip_mask(data: &mut [u8], w: u32, h: u32, ctx: &RenderContext) {
    let x1 = ctx.clip_x1.max(0.0) as u32;
    let y1 = ctx.clip_y1.max(0.0) as u32;
    let x2 = ctx.clip_x2.max(0.0).min(w as f32) as u32;
    let y2 = ctx.clip_y2.max(0.0).min(h as f32) as u32;

    for py in 0..h {
        for px in 0..w {
            let inside = px >= x1 && px < x2 && py >= y1 && py < y2;
            let clear = if ctx.clip_inverse { inside } else { !inside };
            if clear {
                let idx = ((py * w + px) * 4) as usize;
                data[idx] = 0;
                data[idx + 1] = 0;
                data[idx + 2] = 0;
                data[idx + 3] = 0;
            }
        }
    }
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).clamp(0.0, 255.0) as u8
}

pub fn alignment_to_pos(alignment: u8) -> (f32, f32) {
    match alignment {
        1 => (0.0, 1.0),
        2 => (0.5, 1.0),
        3 => (1.0, 1.0),
        4 => (0.0, 0.5),
        5 => (0.5, 0.5),
        6 => (1.0, 0.5),
        7 => (0.0, 0.0),
        8 => (0.5, 0.0),
        9 => (1.0, 0.0),
        _ => (0.5, 1.0),
    }
}

pub fn strip_override_blocks(text: &str) -> String {
    let mut result = String::new();
    let mut depth = 0;
    for ch in text.chars() {
        match ch {
            '{' => depth += 1,
            '}' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ if depth == 0 => result.push(ch),
            _ => {}
        }
    }
    result
}

trait EventExt {
    fn is_visible_at(&self, ts: Timestamp) -> bool;
}

impl EventExt for Event {
    fn is_visible_at(&self, ts: Timestamp) -> bool {
        ts.as_ms() >= self.start.as_ms() && ts.as_ms() < self.end.as_ms()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ass_parser::AssColor;

    // ── interpolate_move ──────────────────────────────────────

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

    // ── apply_alpha_multiplier ────────────────────────────────

    #[test]
    fn test_alpha_multiplier_full() {
        let mut data = vec![255, 255, 255, 200, 128, 128, 128, 100];
        apply_alpha_multiplier(&mut data, 1.0);
        assert_eq!(data[3], 200); // unchanged
        assert_eq!(data[7], 100); // unchanged
    }

    #[test]
    fn test_alpha_multiplier_half() {
        let mut data = vec![255, 255, 255, 200, 128, 128, 128, 100];
        apply_alpha_multiplier(&mut data, 0.5);
        assert_eq!(data[3], 100); // 200 * 0.5
        assert_eq!(data[7], 50);  // 100 * 0.5
    }

    #[test]
    fn test_alpha_multiplier_zero() {
        let mut data = vec![255, 255, 255, 200];
        apply_alpha_multiplier(&mut data, 0.0);
        assert_eq!(data[3], 0);
    }

    #[test]
    fn test_alpha_multiplier_only_alpha() {
        let mut data = vec![100, 150, 200, 160];
        apply_alpha_multiplier(&mut data, 0.5);
        assert_eq!(data[0], 100); // R unchanged
        assert_eq!(data[1], 150); // G unchanged
        assert_eq!(data[2], 200); // B unchanged
        assert_eq!(data[3], 80);  // A halved
    }

    // ── apply_clip_mask ───────────────────────────────────────

    #[test]
    fn test_clip_mask_normal_inside_preserved() {
        let mut data = vec![0u8; 4 * 4 * 4]; // 4x4 RGBA image
        // Fill all pixels with white
        for i in 0..16 {
            data[i * 4] = 255;
            data[i * 4 + 1] = 255;
            data[i * 4 + 2] = 255;
            data[i * 4 + 3] = 255;
        }
        let mut ctx = RenderContext::default();
        ctx.clip_enabled = true;
        ctx.clip_x1 = 1.0;
        ctx.clip_y1 = 1.0;
        ctx.clip_x2 = 3.0;
        ctx.clip_y2 = 3.0;
        ctx.clip_inverse = false;
        apply_clip_mask(&mut data, 4, 4, &ctx);
        // Inside clip: pixel (1,1) should be preserved
        let inside_idx = ((1 * 4 + 1) * 4) as usize;
        assert_eq!(data[inside_idx + 3], 255);
        // Outside clip: pixel (0,0) should be cleared
        assert_eq!(data[3], 0);
    }

    #[test]
    fn test_clip_mask_inverse_inside_cleared() {
        let mut data = vec![0u8; 4 * 4 * 4]; // 4x4 image
        for i in 0..16 {
            data[i * 4] = 255;
            data[i * 4 + 1] = 255;
            data[i * 4 + 2] = 255;
            data[i * 4 + 3] = 255;
        }
        let mut ctx = RenderContext::default();
        ctx.clip_enabled = true;
        ctx.clip_x1 = 1.0;
        ctx.clip_y1 = 1.0;
        ctx.clip_x2 = 3.0;
        ctx.clip_y2 = 3.0;
        ctx.clip_inverse = true;
        apply_clip_mask(&mut data, 4, 4, &ctx);
        // Inside clip: pixel (1,1) should be CLEARED
        let inside_idx = ((1 * 4 + 1) * 4) as usize;
        assert_eq!(data[inside_idx + 3], 0);
        // Outside clip: pixel (0,0) should be PRESERVED
        assert_eq!(data[3], 255);
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
        // clip is parsed via the regular ASS parser, not parse_single_tag_from_str
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

    // ── parse_single_tag_from_str ─────────────────────────────

    #[test]
    fn test_parse_single_tag_fs() {
        assert!(matches!(parse_single_tag_from_str("fs48"), Some(OverrideTag::FontSize(v)) if v == 48.0));
    }

    #[test]
    fn test_parse_single_tag_fn() {
        assert!(matches!(parse_single_tag_from_str("fnArial"), Some(OverrideTag::FontName(n)) if n == "Arial"));
    }

    #[test]
    fn test_parse_single_tag_bold() {
        assert!(matches!(parse_single_tag_from_str("b1"), Some(OverrideTag::Bold(true))));
        assert!(matches!(parse_single_tag_from_str("b0"), Some(OverrideTag::Bold(false))));
    }

    #[test]
    fn test_parse_single_tag_italic() {
        assert!(matches!(parse_single_tag_from_str("i1"), Some(OverrideTag::Italic(true))));
        assert!(matches!(parse_single_tag_from_str("i0"), Some(OverrideTag::Italic(false))));
    }

    #[test]
    fn test_parse_single_tag_bord() {
        assert!(matches!(parse_single_tag_from_str("bord3"), Some(OverrideTag::Border(v)) if v == 3.0));
    }

    #[test]
    fn test_parse_single_tag_shad() {
        assert!(matches!(parse_single_tag_from_str("shad5"), Some(OverrideTag::Shadow(v)) if v == 5.0));
    }

    #[test]
    fn test_parse_single_tag_fscx() {
        assert!(matches!(parse_single_tag_from_str("fscx150"), Some(OverrideTag::Scale { x, y: 100.0 }) if x == 150.0));
    }

    #[test]
    fn test_parse_single_tag_fscy() {
        assert!(matches!(parse_single_tag_from_str("fscy80"), Some(OverrideTag::Scale { x: 100.0, y }) if y == 80.0));
    }

    #[test]
    fn test_parse_single_tag_frz() {
        assert!(matches!(parse_single_tag_from_str("frz45"), Some(OverrideTag::Rotation { x: 0.0, y: 0.0, z }) if z == 45.0));
    }

    #[test]
    fn test_parse_single_tag_an() {
        assert!(matches!(parse_single_tag_from_str("an8"), Some(OverrideTag::AlignmentNumpad(v)) if v == 8));
    }

    #[test]
    fn test_parse_single_tag_unknown() {
        assert!(parse_single_tag_from_str("zzz").is_none());
    }

    #[test]
    fn test_parse_single_tag_empty() {
        assert!(parse_single_tag_from_str("").is_none());
    }

    // ── apply_transform_tag integration ───────────────────────

    #[test]
    fn test_apply_transform_fontsize() {
        let mut ctx = RenderContext::default();
        ctx.font_size = 20.0;
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
        let mut ctx = RenderContext::default();
        ctx.font_size = 20.0;
        apply_transform_tag(&mut ctx, "\\fs40", 500, 1000, 1.0, 200, 0, 1500, 100.0, 100.0);
        // Before t1=500 → no change
        assert_eq!(ctx.font_size, 20.0);
    }

    #[test]
    fn test_apply_transform_color() {
        let mut ctx = RenderContext::default();
        ctx.primary_color = [255, 0, 0, 255]; // red
        let red = AssColor::from_rgb(0, 0, 255); // blue in ASS format (BGR)
        apply_transform_tag(&mut ctx, &format!("\\1c{}", red.to_ass_hex()), 0, 1000, 1.0, 1000, 0, 1000, 100.0, 100.0);
        // At progress=1.0 → fully interpolated to target
        assert_eq!(ctx.primary_color[2], 255); // blue channel should be 255
        assert_eq!(ctx.primary_color[0], 0);   // red channel should be 0
    }

    #[test]
    fn test_apply_transform_scale() {
        let mut ctx = RenderContext::default();
        ctx.scale_x = 100.0;
        ctx.scale_y = 100.0;
        apply_transform_tag(&mut ctx, "\\fscx200", 0, 1000, 1.0, 500, 0, 1000, 100.0, 100.0);
        assert!(ctx.scale_x > 149.0 && ctx.scale_x < 151.0);
        assert_eq!(ctx.scale_y, 100.0); // y unchanged
    }
}
