use ass_parser::{AssFile, Event, OverrideTag, Style, Timestamp};
use tiny_skia::Pixmap;

use crate::context::{RenderConfig, RenderContext, RenderedFrame};
use crate::effects;
use crate::font::FontManager;
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
            let ctx = self.build_context(event, &style);

            self.render_event(&mut pixmap, event, &ctx);
        }

        Some(RenderedFrame {
            pts_ms: timestamp_ms,
            duration_ms: 0,
            width: self.config.width,
            height: self.config.height,
            bitmap: pixmap.data().to_vec(),
        })
    }

    pub fn build_context(&self, event: &Event, style: &Style) -> RenderContext {
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
        ctx.margin_l = ctx.margin_l * self.config.width as f32 / res_x;
        ctx.margin_r = ctx.margin_r * self.config.width as f32 / res_x;
        ctx.margin_v = ctx.margin_v * self.config.height as f32 / res_y;
        ctx.font_size = ctx.font_size * self.config.height as f32 / res_y;

        for tag in &event.override_tags {
            match tag {
                OverrideTag::FontSize(fs) => ctx.font_size = *fs as f32,
                OverrideTag::FontName(name) => ctx.font_name = name.clone(),
                OverrideTag::Bold(b) => ctx.bold = *b,
                OverrideTag::BoldWeight(w) => ctx.bold = *w >= 700,
                OverrideTag::Italic(i) => ctx.italic = *i,
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
                OverrideTag::Shadow(d) => ctx.shadow_depth = *d as f32,
                OverrideTag::Blur(r) | OverrideTag::GaussianBlur(r) => ctx.blur = *r as f32,
                OverrideTag::Spacing(s) => ctx.spacing = *s as f32,
                OverrideTag::Scale { x, y } => {
                    ctx.scale_x = *x as f32;
                    ctx.scale_y = *y as f32;
                }
                OverrideTag::Rotation { x, y, z } => {
                    ctx.rotation = *z as f32;
                    if *x != 0.0 { ctx.origin_x = *x as f32; }
                    if *y != 0.0 { ctx.origin_y = *y as f32; }
                }
                OverrideTag::Alignment(a) => ctx.alignment = *a,
                OverrideTag::AlignmentNumpad(a) => ctx.alignment = *a,
                OverrideTag::Pos { x, y } => {
                    ctx.x = *x as f32 * self.config.width as f32 / self.config.script_width as f32;
                    ctx.y = *y as f32 * self.config.height as f32 / self.config.script_height as f32;
                }
                OverrideTag::Move { x1, y1, .. } => {
                    // TODO: time interpolation — for now, use start position statically
                    ctx.x = *x1 as f32 * self.config.width as f32 / self.config.script_width as f32;
                    ctx.y = *y1 as f32 * self.config.height as f32 / self.config.script_height as f32;
                }
                OverrideTag::Clip { x1, y1, x2, y2 } => {
                    ctx.clip_x1 = *x1 as f32;
                    ctx.clip_y1 = *y1 as f32;
                    ctx.clip_x2 = *x2 as f32;
                    ctx.clip_y2 = *y2 as f32;
                    ctx.clip_enabled = true;
                }
                OverrideTag::ClipInverse { x1, y1, x2, y2 } => {
                    // Same as clip for now (visual clipping)
                    ctx.clip_x1 = *x1 as f32;
                    ctx.clip_y1 = *y1 as f32;
                    ctx.clip_x2 = *x2 as f32;
                    ctx.clip_y2 = *y2 as f32;
                    ctx.clip_enabled = true;
                }
                OverrideTag::BorderX(w) => ctx.outline_width = *w as f32,
                OverrideTag::BorderY(w) => ctx.outline_width = *w as f32,
                OverrideTag::ShadowX(d) => ctx.shadow_depth = *d as f32,
                OverrideTag::ShadowY(d) => ctx.shadow_depth = *d as f32,
                OverrideTag::WrapStyle(w) => ctx.wrap_style = *w,
                OverrideTag::Underline(u) => ctx.underline = *u,
                OverrideTag::Strikeout(s) => ctx.strikeout = *s,
                // TODO: add when Origin variant exists in ass-parser
                // TODO: add when Shear variant exists in ass-parser
                _ => {}
            }
        }

        if !event.override_tags.iter().any(|t| matches!(t, OverrideTag::Pos { .. } | OverrideTag::Move { .. })) {
            let (ax, ay) = alignment_to_pos(ctx.alignment);
            ctx.x = ctx.margin_l + ax * (self.config.width as f32 - ctx.margin_l - ctx.margin_r);
            ctx.y = ctx.margin_v + ay * (self.config.height as f32 - ctx.margin_v * 2.0);
        }

        ctx
    }

    fn render_event(&self, pixmap: &mut Pixmap, event: &Event, ctx: &RenderContext) {
        let plain_text = strip_override_blocks(&event.text);
        if plain_text.is_empty() {
            return;
        }

        let font_id = match self.font_manager.query(&ctx.font_name, ctx.bold, ctx.italic) {
            Some(id) => id,
            None => return,
        };

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

        effects::composite_over(pixmap.data_mut(), &final_data, w, h);

        if ctx.clip_enabled {
            let x1 = ctx.clip_x1.max(0.0) as u32;
            let y1 = ctx.clip_y1.max(0.0) as u32;
            let x2 = ctx.clip_x2.max(0.0).min(w as f32) as u32;
            let y2 = ctx.clip_y2.max(0.0).min(h as f32) as u32;
            let data = pixmap.data_mut();
            for py in 0..h {
                for px in 0..w {
                    if px < x1 || px >= x2 || py < y1 || py >= y2 {
                        let idx = ((py * w + px) * 4) as usize;
                        data[idx] = 0;
                        data[idx + 1] = 0;
                        data[idx + 2] = 0;
                        data[idx + 3] = 0;
                    }
                }
            }
        }
    }
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
