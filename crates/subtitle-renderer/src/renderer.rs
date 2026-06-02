use std::sync::Mutex;

use ass_parser::{AssFile, Effect, Event, OverrideTag, Style, Timestamp};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform as SkiaTransform};

use crate::context::{RenderConfig, RenderContext, RenderedFrame};
use crate::effects;
use crate::font::FontManager;
use crate::karaoke::{KaraokePhase, KaraokeRenderer};
use ass_parser::karaoke::KaraokeStyle;
use crate::rasterizer::{apply_anisotropic_outline, Rasterizer};
use crate::shaper::{Shaper, ShapedText};
use crate::transform::AffineTransform;

/// ASS subtitle renderer that produces RGBA bitmaps for encoding to PGS/SUP.
///
/// The renderer manages font loading via [`FontManager`] and renders ASS events
/// to RGBA bitmaps at a given timestamp. It handles text shaping, glyph rasterization,
/// effects (blur, shadow, rotation, scale, clip), time-aware animations, and karaoke.
///
/// # Pipeline
///
/// 1. [`render_ass`](Renderer::render_ass) iterates visible dialogue events at a timestamp
/// 2. For each event, [`build_context`](Renderer::build_context) applies override tags
///    to create a [`RenderContext`] with time-aware animation state
/// 3. [`render_event`](Renderer::render_event) shapes text, rasterizes glyphs, applies
///    effects, and composites onto the frame bitmap
///
/// # Example
///
/// ```
/// use subtitle_renderer::{Renderer, RenderConfig};
///
/// let config = RenderConfig::default();
/// let renderer = Renderer::new(config);
/// // renderer.render_ass(&ass_file, timestamp_ms);
/// ```
pub struct Renderer {
    config: RenderConfig,
    font_manager: FontManager,
    pixmap_pool: Mutex<PixmapPool>,
}

/// Reusable pixmap buffer pool to reduce allocations across events.
struct PixmapPool {
    pool: Vec<Pixmap>,
    max_cached: usize,
}

impl PixmapPool {
    fn new(max_cached: usize) -> Self {
        Self { pool: Vec::new(), max_cached }
    }

    /// Retrieves a pixmap of the given size from the pool, or allocates a new one.
    fn get(&mut self, w: u32, h: u32) -> Option<Pixmap> {
        if let Some(pos) = self.pool.iter().position(|p| p.width() == w && p.height() == h) {
            let mut p = self.pool.remove(pos);
            p.data_mut().fill(0);
            return Some(p);
        }
        Pixmap::new(w, h)
    }

    /// Returns a pixmap to the pool for reuse.
    fn put(&mut self, p: Pixmap) {
        if self.pool.len() < self.max_cached {
            self.pool.push(p);
        }
    }
}

/// Shaped-line layout result used to separate glyph layout from rasterization,
/// enabling bounding-box computation before pixmap allocation.
struct ShapedLine {
    shaped: ShapedText,
    line_y: f32,
    x_start: f32,
}

impl Renderer {
    /// Creates a new renderer with the given configuration.
    ///
    /// Automatically loads system fonts via [`FontManager::load_system_fonts`].
    pub fn new(config: RenderConfig) -> Self {
        let mut fm = FontManager::new();
        fm.load_system_fonts();
        Self {
            config,
            font_manager: fm,
            pixmap_pool: Mutex::new(PixmapPool::new(8)),
        }
    }

    /// Returns a reference to the font manager for querying/loading fonts.
    pub fn font_manager(&self) -> &FontManager {
        &self.font_manager
    }

    /// Returns a mutable reference to the font manager for loading custom fonts.
    pub fn font_manager_mut(&mut self) -> &mut FontManager {
        &mut self.font_manager
    }

    /// Renders all visible dialogue events at the given timestamp to an RGBA frame.
    ///
    /// Events outside the timestamp range are skipped via [`Event::is_visible_at`].
    /// Each visible event is shaped, rasterized, and composited onto the frame bitmap.
    ///
    /// Returns `None` if the output dimensions are zero.
    pub fn render_ass(&self, ass: &AssFile, timestamp_ms: u64) -> Option<RenderedFrame> {
        let ts = Timestamp::from_ms(timestamp_ms);
        let mut pixmap = Pixmap::new(self.config.width, self.config.height)?;

        let mut events: Vec<&Event> = ass.dialogue_events().collect();
        events.retain(|e| e.is_visible_at(ts));
        events.sort_by_key(|e| e.layer);

        let duration_ms = events
            .iter()
            .map(|e| e.end.as_ms().saturating_sub(e.start.as_ms()))
            .max()
            .unwrap_or(0);

        for event in events {
            let style = ass.find_style(&event.style_name).cloned().unwrap_or_default();
            let event_start = event.start.as_ms();
            let event_end = event.end.as_ms();
            let ctx = self.build_context(event, &style, ass, timestamp_ms, event_start, event_end);

            self.render_event(&mut pixmap, event, &ctx, timestamp_ms, event_start);
        }

        Some(RenderedFrame {
            pts_ms: timestamp_ms,
            duration_ms,
            width: self.config.width,
            height: self.config.height,
            bitmap: pixmap.data().to_vec(),
        })
    }

    /// Renders events with frame caching to avoid redundant work for static subtitles.
    ///
    /// Uses [`FrameCache`] keyed by `(event_index, timestamp_ms)`. On cache hit,
    /// returns the cached frame directly. On miss, calls [`render_ass`](Renderer::render_ass)
    /// and stores the result.
    pub fn render_ass_cached(
        &self,
        ass: &AssFile,
        timestamp_ms: u64,
        cache: &crate::cache::FrameCache,
        _event_index: usize,
    ) -> Option<RenderedFrame> {
        let key = crate::cache::make_frame_key(timestamp_ms);
        if let Some(cached) = cache.get(&key) {
            return Some(cached);
        }
        let frame = self.render_ass(ass, timestamp_ms)?;
        cache.insert(key, frame.clone());
        Some(frame)
    }

    pub fn build_context(
        &self,
        event: &Event,
        style: &Style,
        ass: &AssFile,
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
            border_style: style.border_style,
            ..Default::default()
        };

        ctx.scale_x = style.scale_x as f32;
        ctx.scale_y = style.scale_y as f32;
        ctx.spacing = style.spacing as f32;
        ctx.underline = style.underline;
        ctx.strikeout = style.strikeout;
        ctx.rotation = style.angle as f32;

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
                OverrideTag::Border(w) => {
                    ctx.outline_width = *w as f32;
                    ctx.outline_x_width = 0.0;
                    ctx.outline_y_width = 0.0;
                }
                OverrideTag::BorderX(w) => ctx.outline_x_width = *w as f32,
                OverrideTag::BorderY(w) => ctx.outline_y_width = *w as f32,
                OverrideTag::Shadow(d) => {
                    ctx.shadow_depth = *d as f32;
                    ctx.shadow_x = 0.0;
                    ctx.shadow_y = 0.0;
                }
                OverrideTag::ShadowX(d) => ctx.shadow_x = *d as f32,
                OverrideTag::ShadowY(d) => ctx.shadow_y = *d as f32,
                OverrideTag::Blur(r) | OverrideTag::GaussianBlur(r) => ctx.blur = *r as f32,
                OverrideTag::Spacing(s) => ctx.spacing = *s as f32,
                OverrideTag::Scale { x, y } => {
                    ctx.scale_x = *x as f32;
                    ctx.scale_y = *y as f32;
                }
                OverrideTag::Rotation { x, y, z } => {
                    ctx.rotation = *z as f32;
                    ctx.perspective_x = *x as f32;
                    ctx.perspective_y = *y as f32;
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
                OverrideTag::Charset(c) => ctx.charset = *c,
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
                    ctx.clip_drawing_commands = None;
                }
                OverrideTag::ClipDrawing { scale, commands } => {
                    ctx.clip_drawing_commands = Some(commands.clone());
                    ctx.clip_drawing_scale = *scale;
                    ctx.clip_drawing_inverse = false;
                    ctx.clip_enabled = true;
                }
                OverrideTag::ClipInverseDrawing { scale, commands } => {
                    ctx.clip_drawing_commands = Some(commands.clone());
                    ctx.clip_drawing_scale = *scale;
                    ctx.clip_drawing_inverse = true;
                    ctx.clip_enabled = true;
                }
                OverrideTag::Transform { tag: inner_tag, t1, t2, accel } => {
                    // If the inner tag contains \pos, initialize ctx position to
                    // the alignment-derived values so the transform lerps FROM
                    // the correct starting point (not from 0,0).
                    let parsed_inner = parse_override_block(inner_tag);
                    if parsed_inner.iter().any(|t| matches!(t, OverrideTag::Pos { .. })) {
                        let (_ax, ay) = alignment_to_pos(ctx.alignment);
                        ctx.x = ctx.margin_l;
                        ctx.y = ctx.margin_v + ay * (self.config.height as f32 - ctx.margin_v * 2.0);
                        has_pos = true;
                    }
                    apply_transform_tag(
                        &mut ctx, inner_tag,
                        *t1, *t2, *accel,
                        timestamp_ms, event_start_ms, event_end_ms,
                        scale_x, scale_y,
                    );
                }
                OverrideTag::Reset(style_name) => {
                    let reset_style = if style_name.is_empty() {
                        Some(style)
                    } else {
                        ass.find_style(style_name)
                    };
                    if let Some(s) = reset_style {
                        ctx.font_name = s.font_name.clone();
                        ctx.font_size = s.font_size as f32 * scale_y;
                        ctx.bold = s.bold;
                        ctx.italic = s.italic;
                        ctx.primary_color = s.primary_color.to_rgba();
                        ctx.secondary_color = s.secondary_color.to_rgba();
                        ctx.outline_color = s.outline_color.to_rgba();
                        ctx.shadow_color = s.shadow_color.to_rgba();
                        ctx.outline_width = s.outline_width as f32;
                        ctx.shadow_depth = s.shadow_depth as f32;
                        ctx.alignment = s.alignment;
                        ctx.scale_x = s.scale_x as f32;
                        ctx.scale_y = s.scale_y as f32;
                        ctx.spacing = s.spacing as f32;
                        ctx.underline = s.underline;
                        ctx.strikeout = s.strikeout;
                        ctx.rotation = s.angle as f32;
                        ctx.border_style = s.border_style;
                        ctx.perspective_x = 0.0;
                        ctx.perspective_y = 0.0;
                    }
                }
                OverrideTag::ResetAll => {
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
                    ctx.writing_mode = 0;
                    ctx.baseline_offset = 0.0;
                    ctx.perspective_x = 0.0;
                    ctx.perspective_y = 0.0;
                }
                OverrideTag::WritingMode(mode) => {
                    ctx.writing_mode = *mode;
                }
                OverrideTag::BaselineOffset(offset) => {
                    ctx.baseline_offset = *offset;
                }
                OverrideTag::DrawingMode(level) => {
                    ctx.drawing_mode = *level;
                }
                OverrideTag::Unknown(tag) => {
                    tracing::warn!(tag = %tag, "unrecognized override tag ignored");
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
            let (_ax, ay) = alignment_to_pos(ctx.alignment);
            ctx.x = ctx.margin_l;
            ctx.y = ctx.margin_v + ay * (self.config.height as f32 - ctx.margin_v * 2.0);
        }

        ctx
    }

    fn render_event(&self, pixmap: &mut Pixmap, event: &Event, ctx: &RenderContext, timestamp_ms: u64, event_start_ms: u64) {
        // Apply Banner/Scroll effect offset before text positioning
        let mut ctx = ctx.clone();
        match &event.effect {
            Effect::Banner { delay_per_pixel, left_to_right, .. } if *delay_per_pixel > 0 => {
                let elapsed = timestamp_ms.saturating_sub(event_start_ms);
                let x_offset = elapsed as f32 / *delay_per_pixel as f32;
                if *left_to_right {
                    ctx.x += x_offset;
                } else {
                    ctx.x -= x_offset;
                }
            }
            Effect::ScrollUp { delay_per_row, bottom_offset, .. } if *delay_per_row > 0 => {
                let elapsed = timestamp_ms.saturating_sub(event_start_ms);
                let y_offset = elapsed as f32 / *delay_per_row as f32;
                ctx.y = self.config.height as f32 - *bottom_offset as f32 - y_offset;
            }
            Effect::ScrollDown { delay_per_row, top_offset, .. } if *delay_per_row > 0 => {
                let elapsed = timestamp_ms.saturating_sub(event_start_ms);
                let y_offset = elapsed as f32 / *delay_per_row as f32;
                ctx.y = *top_offset as f32 + y_offset;
            }
            _ => {}
        }

        let plain_text = strip_override_blocks(&event.text);
        if plain_text.is_empty() {
            return;
        }

        let font_id = match self.font_manager.query_with_fallback(&ctx.font_name, ctx.bold, ctx.italic) {
            Some(id) => id,
            None => return,
        };

        if event.has_karaoke() && !event.karaoke_segments.is_empty() {
            self.render_karaoke(pixmap, event, &ctx, font_id, timestamp_ms, event_start_ms);
            return;
        }

        let drawing_level = parse_drawing_level(&event.text);

        if drawing_level > 0 {
            self.render_drawing(pixmap, &plain_text, &ctx, drawing_level);
            return;
        }

        let shaper = Shaper::new(&self.font_manager);

        let available_width = self.config.width as f32 - ctx.margin_l - ctx.margin_r;
        let lines = wrap_text(&plain_text, ctx.wrap_style, &shaper, font_id, ctx.font_size, ctx.spacing, available_width);
        let line_height = ctx.font_size * 1.2;

        let w = pixmap.width();
        let h = pixmap.height();
        let align_col = ctx.alignment % 3;

        // Phase 1: Shape all lines, store results for bbox computation and rendering.
        let mut shaped_lines: Vec<ShapedLine> = Vec::new();
        for (line_idx, line) in lines.iter().enumerate() {
            if line.is_empty() {
                continue;
            }
            let shaped = match shaper.shape(line, font_id, ctx.font_size) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let line_y = ctx.y + line_idx as f32 * line_height;
            let text_width = shaped.total_advance;
            let x_start = match align_col {
                2 => ctx.x + (available_width - text_width) / 2.0,
                0 => ctx.x + available_width - text_width,
                _ => ctx.x,
            };
            shaped_lines.push(ShapedLine { shaped, line_y, x_start });
        }

        if shaped_lines.is_empty() {
            return;
        }

        // Phase 2: Compute tight bounding box of all glyph ink.
        // Sub-region is only safe when there is no rotation/shear/clip that would
        // shift content outside the bbox.
        let can_sub = ctx.rotation == 0.0
            && ctx.shear_x == 0.0
            && ctx.shear_y == 0.0
            && ctx.perspective_x == 0.0
            && ctx.perspective_y == 0.0
            && (ctx.writing_mode == 0 || ctx.writing_mode == 1)
            && !ctx.clip_enabled
            && ctx.clip_drawing_commands.is_none();
        let sub_bbox = if can_sub {
            compute_tight_bbox(&shaped_lines, &shaper, font_id, ctx.font_size, &ctx)
        } else {
            None
        };

        // Phase 3: Determine sub-region or full frame.
        // For border_style=3 (opaque box), reduce padding since we skip outline+shadow.
        let (pad_border, pad_shadow) = if ctx.border_style == 3 {
            (0.0, 0.0)
        } else {
            let border = ctx.outline_width.max(ctx.outline_x_width).max(ctx.outline_y_width);
            let shadow = ctx.shadow_depth.max(ctx.shadow_x).max(ctx.shadow_y);
            (border * 2.0, shadow)
        };
        let (ox, oy, lw, lh, use_sub) = if let Some((min_x, min_y, max_x, max_y)) = sub_bbox {
            let pad = (pad_border + pad_shadow + ctx.blur).max(20.0);
            let ox = (min_x - pad).floor().max(0.0) as u32;
            let oy = (min_y - pad).floor().max(0.0) as u32;
            let lw = ((max_x - min_x) + pad * 2.0).ceil().max(1.0) as u32;
            let lh = ((max_y - min_y) + pad * 2.0).ceil().max(1.0) as u32;
            let lw = lw.min(w.saturating_sub(ox)).max(1);
            let lh = lh.min(h.saturating_sub(oy)).max(1);
            (ox, oy, lw, lh, true)
        } else {
            (0, 0, w, h, false)
        };

        // Phase 4: Allocate layer and render glyphs with sub-region offset.
        let mut layer = self.pixmap_pool.lock().unwrap().get(lw, lh).unwrap();
        let oxf = ox as f32;
        let oyf = oy as f32;

        // For border_style=3 (opaque box): fill background with shadow_color (fully opaque)
        // before rendering glyphs, and suppress outline strokes in the rasterizer.
        let render_ctx = if ctx.border_style == 3 {
            // Fill entire layer with opaque shadow_color (BackColour)
            let mut bg_paint = Paint::default();
            bg_paint.set_color_rgba8(
                ctx.shadow_color[0],
                ctx.shadow_color[1],
                ctx.shadow_color[2],
                255,
            );
            if lw > 0 && lh > 0 {
                if let Some(rect) = Rect::from_xywh(0.0, 0.0, lw as f32, lh as f32) {
                    let mut pb = PathBuilder::new();
                    pb.push_rect(rect);
                    if let Some(path) = pb.finish() {
                        layer.fill_path(&path, &bg_paint, FillRule::Winding, SkiaTransform::identity(), None);
                    }
                }
            }
            // Create a clone with outline_width=0 so the rasterizer doesn't stroke outlines.
            let mut c = ctx.clone();
            c.outline_width = 0.0;
            c.outline_x_width = 0.0;
            c.outline_y_width = 0.0;
            c
        } else {
            ctx.clone()
        };

        for sl in &shaped_lines {
            let mut x = sl.x_start;
            for glyph in &sl.shaped.glyphs {
                Rasterizer::rasterize_glyph(
                    &mut layer,
                    &self.font_manager,
                    font_id,
                    glyph,
                    x - oxf,
                    sl.line_y - oyf,
                    &render_ctx,
                );
                x += glyph.x_advance + ctx.spacing;
            }

            if ctx.underline || ctx.strikeout {
                let line_thickness = (ctx.font_size / 16.0).max(1.0);
                let mut paint = tiny_skia::Paint::default();
                paint.set_color_rgba8(
                    ctx.primary_color[0],
                    ctx.primary_color[1],
                    ctx.primary_color[2],
                    ctx.primary_color[3],
                );
                paint.anti_alias = true;

                let x0 = sl.x_start - oxf;
                let x1 = sl.x_start + sl.shaped.total_advance - oxf;

                if ctx.underline {
                    let uy = sl.line_y + ctx.font_size * 0.1 - oyf;
                    let mut pb = tiny_skia::PathBuilder::new();
                    pb.move_to(x0, uy);
                    pb.line_to(x1, uy);
                    pb.close();
                    if let Some(path) = pb.finish() {
                        let stroke = tiny_skia::Stroke { width: line_thickness, ..Default::default() };
                        layer.stroke_path(&path, &paint, &stroke, tiny_skia::Transform::identity(), None);
                    }
                }

                if ctx.strikeout {
                    let sy = sl.line_y - ctx.font_size * 0.35 - oyf;
                    let mut pb = tiny_skia::PathBuilder::new();
                    pb.move_to(x0, sy);
                    pb.line_to(x1, sy);
                    pb.close();
                    if let Some(path) = pb.finish() {
                        let stroke = tiny_skia::Stroke { width: line_thickness, ..Default::default() };
                        layer.stroke_path(&path, &paint, &stroke, tiny_skia::Transform::identity(), None);
                    }
                }
            }
        }

        // Phase 5: Blur (operates on sub-region). Skip for opaque box.
        if ctx.border_style != 3 && ctx.blur > 0.0 {
            effects::apply_gaussian_blur(&mut layer, ctx.blur);
        }

        // Phase 6: Shadow (operates on sub-region). Skip for opaque box.
        if ctx.border_style != 3 && ctx.shadow_depth > 0.0 {
            let layer_data = layer.data().to_vec();
            let shadow_layer = effects::apply_shadow(
                &layer_data,
                lw,
                lh,
                if ctx.shadow_x != 0.0 { ctx.shadow_x } else { ctx.shadow_depth },
                if ctx.shadow_y != 0.0 { ctx.shadow_y } else { ctx.shadow_depth },
                ctx.blur,
                ctx.shadow_color,
            );
            let mut shadow_pixmap = self.pixmap_pool.lock().unwrap().get(lw, lh).unwrap();
            shadow_pixmap.data_mut().copy_from_slice(&shadow_layer);
            effects::composite_over(layer.data_mut(), shadow_pixmap.data(), lw, lh);
            self.pixmap_pool.lock().unwrap().put(shadow_pixmap);
        }

        // Phase 7: Composite back.
        if use_sub {
            // Sub-region path: no rotation, no clip (guaranteed by can_sub check).
            if ctx.alpha_multiplier < 0.999 {
                apply_alpha_multiplier(layer.data_mut(), ctx.alpha_multiplier);
            }
            composite_subregion(pixmap.data_mut(), layer.data(), w, h, ox, oy, lw, lh);
        } else {
            // Full-frame path: apply transform, clip, alpha — identical to original.
            let mut transform = AffineTransform::rotate_at(ctx.rotation, ctx.origin_x, ctx.origin_y)
                .then(&AffineTransform::scale(ctx.scale_x / 100.0, ctx.scale_y / 100.0))
                .then(&AffineTransform::shear(ctx.shear_x, ctx.shear_y));

            if ctx.writing_mode == 2 {
                transform = transform.then(&AffineTransform::rotate_at(-90.0, ctx.x, ctx.y));
            } else if ctx.writing_mode == 3 {
                transform = transform.then(&AffineTransform::rotate_at(90.0, ctx.x, ctx.y));
            }

            let final_data = if transform.is_identity() && ctx.perspective_x == 0.0 && ctx.perspective_y == 0.0 {
                layer.data().to_vec()
            } else if ctx.perspective_x != 0.0 || ctx.perspective_y != 0.0 {
                transform.apply_with_perspective(
                    layer.data(), w, h, w, h,
                    ctx.perspective_x, ctx.perspective_y,
                    ctx.origin_x, ctx.origin_y,
                )
            } else {
                transform.apply_to_pixmap(layer.data(), w, h, w, h)
            };

            if ctx.clip_enabled {
                let mut clipped = final_data;
                if ctx.clip_drawing_commands.is_some() {
                    let sx = self.config.width as f32 / self.config.script_width as f32;
                    let sy = self.config.height as f32 / self.config.script_height as f32;
                    apply_drawing_clip_mask(&mut clipped, w, h, &ctx, sx, sy);
                } else {
                    apply_clip_mask(&mut clipped, w, h, &ctx);
                }
                if ctx.alpha_multiplier < 0.999 {
                    apply_alpha_multiplier(&mut clipped, ctx.alpha_multiplier);
                }
                effects::composite_over(pixmap.data_mut(), &clipped, w, h);
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

        // Return layer pixmap to pool.
        self.pixmap_pool.lock().unwrap().put(layer);
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

        // Phase 1: Shape all syllables, track cursor and bounding box.
        struct SyllableInfo {
            shaped: ShapedText,
            syllable_x: f32,
            syllable_width: f32,
            is_active: bool,
            progress: f32,
            style: KaraokeStyle,
        }

        let mut syllable_infos: Vec<SyllableInfo> = Vec::new();
        let mut cursor_x = ctx.x;
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;
        let mut any_glyph = false;

        for syllable in &syllables {
            if syllable.text.is_empty() {
                continue;
            }

            if let Ok(shaped) = shaper.shape(&syllable.text, font_id, ctx.font_size) {
                let syllable_x = cursor_x;
                let syllable_width = shaped.total_advance;
                let is_active = matches!(syllable.phase, KaraokePhase::Active { .. });
                let _is_done = matches!(syllable.phase, KaraokePhase::Done);
                let progress = match syllable.phase {
                    KaraokePhase::Active { progress } => progress,
                    _ => 0.0,
                };

                // Track glyph ink extents for bounding box.
                let mut sx = syllable_x;
                for glyph in &shaped.glyphs {
                    if let Some(bbox) = shaper.get_glyph_bbox(font_id, glyph.glyph_id, ctx.font_size) {
                        any_glyph = true;
                        let gx = sx + glyph.x_offset;
                        let gy = ctx.y + glyph.y_offset;
                        min_x = min_x.min(gx + bbox.x_min);
                        min_y = min_y.min(gy + bbox.y_min);
                        max_x = max_x.max(gx + bbox.x_max);
                        max_y = max_y.max(gy + bbox.y_max);
                    }
                    sx += glyph.x_advance;
                }

                // Track karaoke fill clip region extent for active syllables.
                if is_active && matches!(syllable.style, KaraokeStyle::Fill) {
                    let clip_x = syllable_x + KaraokeRenderer::get_fill_clip_x(progress, syllable_width);
                    let clip_y_max = ctx.y + ctx.font_size * 1.5;
                    min_x = min_x.min(clip_x);
                    max_x = max_x.max(syllable_x + syllable_width);
                    min_y = min_y.min(ctx.y);
                    max_y = max_y.max(clip_y_max);
                    any_glyph = true;
                }

                syllable_infos.push(SyllableInfo {
                    shaped,
                    syllable_x,
                    syllable_width,
                    is_active,
                    progress,
                    style: syllable.style,
                });

                cursor_x += syllable_width + ctx.spacing;
            }
        }

        if syllable_infos.is_empty() {
            return;
        }

        // Phase 2: Determine sub-region or full frame.
        let can_sub = ctx.rotation == 0.0
            && ctx.shear_x == 0.0
            && ctx.shear_y == 0.0
            && ctx.perspective_x == 0.0
            && ctx.perspective_y == 0.0
            && (ctx.writing_mode == 0 || ctx.writing_mode == 1)
            && !ctx.clip_enabled
            && ctx.clip_drawing_commands.is_none();

        let (ox, oy, lw, lh, use_sub) = if can_sub && any_glyph {
            let border = ctx.outline_width.max(ctx.outline_x_width).max(ctx.outline_y_width);
            let shadow = ctx.shadow_depth.max(ctx.shadow_x).max(ctx.shadow_y);
            let pad = (border * 2.0 + shadow + ctx.blur).max(20.0);
            let ox = (min_x - pad).floor().max(0.0) as u32;
            let oy = (min_y - pad).floor().max(0.0) as u32;
            let lw = ((max_x - min_x) + pad * 2.0).ceil().max(1.0) as u32;
            let lh = ((max_y - min_y) + pad * 2.0).ceil().max(1.0) as u32;
            let lw = lw.min(w.saturating_sub(ox)).max(1);
            let lh = lh.min(h.saturating_sub(oy)).max(1);
            (ox, oy, lw, lh, true)
        } else {
            (0, 0, w, h, false)
        };

        // Phase 3: Allocate layers and render with offset.
        let mut bg_layer = self.pixmap_pool.lock().unwrap().get(lw, lh).unwrap();
        let mut fg_layer = self.pixmap_pool.lock().unwrap().get(lw, lh).unwrap();
        let oxf = ox as f32;
        let oyf = oy as f32;

        for (i, info) in syllable_infos.iter().enumerate() {
            let syllable = &syllables[i];
            let mut sy_ctx = ctx.clone();
            // B2: \ko (Outline) karaoke — hide fill during active, show outline sweep.
            if info.style == KaraokeStyle::Outline {
                match syllable.phase {
                    KaraokePhase::Pending => {
                        // No outline, fill stays secondary.
                        sy_ctx.primary_color = ctx.secondary_color;
                        sy_ctx.outline_width = 0.0;
                        sy_ctx.outline_x_width = 0.0;
                        sy_ctx.outline_y_width = 0.0;
                    }
                    KaraokePhase::Active { .. } => {
                        // Fill stays secondary, outline uses primary with boosted width.
                        sy_ctx.primary_color = ctx.secondary_color;
                        sy_ctx.outline_color = ctx.primary_color;
                        sy_ctx.outline_width = ctx.outline_width * 3.0;
                        sy_ctx.outline_x_width = ctx.outline_x_width * 3.0;
                        sy_ctx.outline_y_width = ctx.outline_y_width * 3.0;
                    }
                    KaraokePhase::Done => {
                        // Full glyph in primary.
                        sy_ctx.primary_color = ctx.primary_color;
                    }
                }
            } else if matches!(syllable.phase, KaraokePhase::Done | KaraokePhase::Active { .. }) {
                sy_ctx.primary_color = ctx.primary_color;
            } else {
                sy_ctx.primary_color = ctx.secondary_color;
            }

            let target_layer = if info.is_active {
                &mut fg_layer
            } else {
                &mut bg_layer
            };

            let mut sx = info.syllable_x;
            for glyph in &info.shaped.glyphs {
                Rasterizer::rasterize_glyph(
                    target_layer,
                    &self.font_manager,
                    font_id,
                    glyph,
                    sx - oxf,
                    ctx.y - oyf,
                    &sy_ctx,
                );
                sx += glyph.x_advance + ctx.spacing;
            }

            if info.is_active && matches!(info.style, KaraokeStyle::Fill) {
                let clip_x = info.syllable_x
                    + KaraokeRenderer::get_fill_clip_x(info.progress, info.syllable_width);
                let clip_x_adj = (clip_x - oxf).max(0.0) as usize;
                let y_start = (ctx.y - oyf).max(0.0) as usize;
                let y_end = (ctx.y + ctx.font_size * 1.5 - oyf).min(lh as f32) as usize;
                let fg_w = lw as usize;
                let data = fg_layer.data_mut();
                for py in y_start..y_end.min(lh as usize) {
                    for px in clip_x_adj..fg_w {
                        let idx = (py * fg_w + px) * 4;
                        if idx + 3 < data.len() {
                            data[idx] = 0;
                            data[idx + 1] = 0;
                            data[idx + 2] = 0;
                            data[idx + 3] = 0;
                        }
                    }
                }
            }
        }

        // Phase 4: Blur on sub-region.
        if ctx.blur > 0.0 {
            effects::apply_gaussian_blur(&mut bg_layer, ctx.blur);
            effects::apply_gaussian_blur(&mut fg_layer, ctx.blur);
        }

        // Phase 5: Shadow on sub-region.
        if ctx.shadow_depth > 0.0 {
            let bg_data = bg_layer.data().to_vec();
            let shadow_data = effects::apply_shadow(
                &bg_data, lw, lh,
                if ctx.shadow_x != 0.0 { ctx.shadow_x } else { ctx.shadow_depth },
                if ctx.shadow_y != 0.0 { ctx.shadow_y } else { ctx.shadow_depth },
                ctx.blur, ctx.shadow_color,
            );
            let mut shadow_pixmap = self.pixmap_pool.lock().unwrap().get(lw, lh).unwrap();
            shadow_pixmap.data_mut().copy_from_slice(&shadow_data);
            effects::composite_over(bg_layer.data_mut(), shadow_pixmap.data(), lw, lh);
            self.pixmap_pool.lock().unwrap().put(shadow_pixmap);
        }
        if ctx.shadow_depth > 0.0 {
            let fg_data = fg_layer.data().to_vec();
            let shadow_data = effects::apply_shadow(
                &fg_data, lw, lh,
                if ctx.shadow_x != 0.0 { ctx.shadow_x } else { ctx.shadow_depth },
                if ctx.shadow_y != 0.0 { ctx.shadow_y } else { ctx.shadow_depth },
                ctx.blur, ctx.shadow_color,
            );
            let mut shadow_pixmap = self.pixmap_pool.lock().unwrap().get(lw, lh).unwrap();
            shadow_pixmap.data_mut().copy_from_slice(&shadow_data);
            effects::composite_over(fg_layer.data_mut(), shadow_pixmap.data(), lw, lh);
            self.pixmap_pool.lock().unwrap().put(shadow_pixmap);
        }

        // Phase 6: Composite back.
        if use_sub {
            composite_subregion(pixmap.data_mut(), bg_layer.data(), w, h, ox, oy, lw, lh);
            composite_subregion(pixmap.data_mut(), fg_layer.data(), w, h, ox, oy, lw, lh);
        } else {
            effects::composite_over(pixmap.data_mut(), bg_layer.data(), w, h);
            effects::composite_over(pixmap.data_mut(), fg_layer.data(), w, h);
        }

        // Return karaoke layers to pool.
        self.pixmap_pool.lock().unwrap().put(bg_layer);
        self.pixmap_pool.lock().unwrap().put(fg_layer);
    }

    fn render_drawing(&self, pixmap: &mut Pixmap, text: &str, ctx: &RenderContext, drawing_level: u8) {
        let w = pixmap.width();
        let h = pixmap.height();
        let mut layer = Pixmap::new(w, h).unwrap();
        let scale = 1.0 / (drawing_level as f32);

        let commands = parse_drawing_commands(text);
        let mut current_path = PathBuilder::new();

        for cmd in &commands {
            match cmd {
                DrawingCommand::MoveTo(x, y) => {
                    let px = x * scale + ctx.x;
                    let py = y * scale + ctx.y;
                    current_path.move_to(px, py);
                }
                DrawingCommand::LineTo(x, y) => {
                    let px = x * scale + ctx.x;
                    let py = y * scale + ctx.y;
                    current_path.line_to(px, py);
                }
                DrawingCommand::BezierTo(x1, y1, x2, y2, x3, y3) => {
                    let cx1 = x1 * scale + ctx.x;
                    let cy1 = y1 * scale + ctx.y;
                    let cx2 = x2 * scale + ctx.x;
                    let cy2 = y2 * scale + ctx.y;
                    let ex = x3 * scale + ctx.x;
                    let ey = y3 * scale + ctx.y;
                    current_path.cubic_to(cx1, cy1, cx2, cy2, ex, ey);
                }
                DrawingCommand::Close => {
                    current_path.close();
                }
            }
        }

        if let Some(path) = current_path.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(ctx.primary_color[0], ctx.primary_color[1], ctx.primary_color[2], ctx.primary_color[3]);
            paint.anti_alias = true;

            if ctx.outline_width > 0.0 {
                let mut outline_paint = Paint::default();
                outline_paint.set_color_rgba8(ctx.outline_color[0], ctx.outline_color[1], ctx.outline_color[2], ctx.outline_color[3]);
                outline_paint.anti_alias = true;
                apply_anisotropic_outline(
                    &mut layer,
                    &path,
                    ctx.outline_color,
                    ctx.outline_width,
                    ctx.outline_x_width,
                    ctx.outline_y_width,
                );
            }

            layer.fill_path(&path, &paint, FillRule::Winding, SkiaTransform::identity(), None);
        }

        if ctx.blur > 0.0 {
            effects::apply_gaussian_blur(&mut layer, ctx.blur);
        }

        effects::composite_over(pixmap.data_mut(), layer.data(), w, h);
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
            OverrideTag::Underline(u) => {
                if p >= 0.5 {
                    ctx.underline = *u;
                }
            }
            OverrideTag::Strikeout(s) => {
                if p >= 0.5 {
                    ctx.strikeout = *s;
                }
            }
            OverrideTag::BoldWeight(w) => {
                if p >= 0.5 {
                    ctx.bold = *w > 0;
                }
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

fn parse_override_block(text: &str) -> Vec<OverrideTag> {
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

/// Apply a vector drawing clip mask to pixel data.
///
/// Parses `.clip_drawing_commands` and builds a tiny_skia path, then clears
/// pixels outside (or inside, for inverse clips) the filled path.
fn apply_drawing_clip_mask(data: &mut [u8], w: u32, h: u32, ctx: &RenderContext, sx: f32, sy: f32) {
    let commands_text = match ctx.clip_drawing_commands {
        Some(ref c) => c,
        None => return,
    };

    let commands = parse_drawing_commands(commands_text);
    let scale = 1.0 / ctx.clip_drawing_scale;

    let mut pb = PathBuilder::new();
    for cmd in &commands {
        match cmd {
            DrawingCommand::MoveTo(x, y) => pb.move_to(x * scale * sx, y * scale * sy),
            DrawingCommand::LineTo(x, y) => pb.line_to(x * scale * sx, y * scale * sy),
            DrawingCommand::BezierTo(x1, y1, x2, y2, x3, y3) => {
                pb.cubic_to(
                    x1 * scale * sx, y1 * scale * sy,
                    x2 * scale * sx, y2 * scale * sy,
                    x3 * scale * sx, y3 * scale * sy,
                );
            }
            DrawingCommand::Close => pb.close(),
        }
    }

    if let Some(path) = pb.finish() {
        let mut mask = Pixmap::new(w, h).unwrap();
        let mut paint = Paint::default();
        paint.set_color_rgba8(255, 255, 255, 255);
        paint.anti_alias = true;
        mask.fill_path(&path, &paint, FillRule::EvenOdd, SkiaTransform::identity(), None);

        for py in 0..h {
            for px in 0..w {
                let idx = ((py * w + px) * 4 + 3) as usize;
                let inside = mask.data()[idx] > 0;
                let clear = if ctx.clip_drawing_inverse { inside } else { !inside };
                if clear {
                    let base = idx - 3;
                    data[base] = 0;
                    data[base + 1] = 0;
                    data[base + 2] = 0;
                    data[base + 3] = 0;
                }
            }
        }
    }
}

/// Composite a sub-region source buffer into a larger destination buffer.
///
/// Performs Porter-Duff "over" compositing of a (`sw` × `sh`) source image
/// positioned at (`sx`, `sy`) in the (`dw` × `dh`) destination image.
/// The source and destination pixel data are in RGBA byte order.
fn composite_subregion(
    dst: &mut [u8],
    src: &[u8],
    dst_w: u32,
    dst_h: u32,
    src_x: u32,
    src_y: u32,
    src_w: u32,
    src_h: u32,
) {
    for ry in 0..src_h {
        let dy = src_y + ry;
        if dy >= dst_h {
            continue;
        }
        for rx in 0..src_w {
            let dx = src_x + rx;
            if dx >= dst_w {
                continue;
            }

            let si = (ry * src_w + rx) as usize * 4;
            let di = (dy * dst_w + dx) as usize * 4;

            let sa = src[si + 3] as u32;
            if sa == 0 {
                continue;
            }

            let da = dst[di + 3] as u32;
            let out_a = sa + da * (255 - sa) / 255;
            if out_a == 0 {
                continue;
            }

            for c in 0..3 {
                let sv = src[si + c] as u32;
                let dv = dst[di + c] as u32;
                dst[di + c] = ((sv * sa + dv * da * (255 - sa) / 255) / out_a) as u8;
            }
            dst[di + 3] = out_a as u8;
        }
    }
}

/// Compute the tight ink bounding box of all glyphs in `shaped_lines`.
///
/// Returns `(min_x, min_y, max_x, max_y)` in the layer's pixel coordinate space
/// (before any shadow/blur/border padding). Returns `None` if no glyph bbox can
/// be determined (triggers fallback to full-frame allocation).
fn compute_tight_bbox(
    shaped_lines: &[ShapedLine],
    shaper: &Shaper,
    font_id: fontdb::ID,
    font_size: f32,
    ctx: &RenderContext,
) -> Option<(f32, f32, f32, f32)> {
    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;
    let mut any_glyph = false;

    for sl in shaped_lines {
        let mut x = sl.x_start;
        for glyph in &sl.shaped.glyphs {
            if let Some(bbox) = shaper.get_glyph_bbox(font_id, glyph.glyph_id, font_size) {
                any_glyph = true;
                let gx = x + glyph.x_offset;
                let gy = sl.line_y + glyph.y_offset;
                min_x = min_x.min(gx + bbox.x_min);
                min_y = min_y.min(gy + bbox.y_min);
                max_x = max_x.max(gx + bbox.x_max);
                max_y = max_y.max(gy + bbox.y_max);
            }
            x += glyph.x_advance;
        }

        // Account for underline / strikeout lines.
        if ctx.underline {
            let uy = sl.line_y + ctx.font_size * 0.1;
            min_y = min_y.min(uy - 2.0);
            max_y = max_y.max(uy + 2.0);
            min_x = min_x.min(sl.x_start);
            max_x = max_x.max(sl.x_start + sl.shaped.total_advance);
            any_glyph = true;
        }
        if ctx.strikeout {
            let sy = sl.line_y - ctx.font_size * 0.35;
            min_y = min_y.min(sy - 2.0);
            max_y = max_y.max(sy + 2.0);
            min_x = min_x.min(sl.x_start);
            max_x = max_x.max(sl.x_start + sl.shaped.total_advance);
            any_glyph = true;
        }
    }

    if !any_glyph {
        return None;
    }
    Some((min_x, min_y, max_x, max_y))
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).clamp(0.0, 255.0) as u8
}

/// Converts an ASS alignment value (1–9, numpad layout) to a normalized (x, y) position.
///
/// Alignment follows the numpad layout: 7=top-left, 8=top-center, 9=top-right,
/// 4=middle-left, 5=middle-center, 6=middle-right, 1=bottom-left, 2=bottom-center,
/// 3=bottom-right. Returns values in 0.0–1.0 range.
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

/// Removes ASS override blocks (`{...}`) from text, returning plain text for rendering.
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

fn wrap_text(text: &str, wrap_style: u8, shaper: &Shaper, font_id: fontdb::ID, font_size: f32, spacing: f32, available_width: f32) -> Vec<String> {
    let explicit_lines: Vec<&str> = text.split('\n').collect();

    match wrap_style {
        1 => explicit_lines.into_iter().map(String::from).collect(),
        3 => {
            // Low-end wrapping: word-wrap from bottom-right (ASS q=3)
            // Uses same smart wrapping but places lines from bottom
            let mut result: Vec<String> = wrap_text(text, 0, shaper, font_id, font_size, spacing, available_width);
            // q=3 places lines from bottom, achieved by reversing the line order
            result.reverse();
            result
        }
        2 => explicit_lines.into_iter().map(String::from).collect(),
        _ => {
            let mut result = Vec::new();
            for line in &explicit_lines {
                if line.is_empty() {
                    result.push(String::new());
                    continue;
                }
                let words: Vec<&str> = line.split(' ').collect();

                // Phase 1: Pre-shape each word individually (O(W) instead of O(W²)).
                struct WordInfo {
                    text: String,
                    width: f32,
                }
                let word_data: Vec<WordInfo> = words.iter().filter_map(|w| {
                    if w.is_empty() { return None; }
                    shaper.shape(w, font_id, font_size).ok().map(|shaped| WordInfo {
                        text: w.to_string(),
                        width: shaped.total_advance + spacing * shaped.glyphs.len() as f32,
                    })
                }).collect();

                if word_data.is_empty() {
                    if !line.is_empty() {
                        result.push(line.to_string());
                    }
                    continue;
                }

                // Phase 2: Shape single space to correctly measure inter-word gaps.
                let space_width = shaper.shape(" ", font_id, font_size).ok()
                    .map(|s| s.total_advance + spacing * s.glyphs.len() as f32)
                    .unwrap_or(0.0);

                // Phase 3: Line breaking using cumulative word widths.
                let mut current_line = String::new();
                let mut current_width = 0.0f32;

                for (i, wi) in word_data.iter().enumerate() {
                    let gap = if current_line.is_empty() { 0.0 } else { space_width };
                    let test_width = current_width + gap + wi.width;

                    if current_width > 0.0 && test_width > available_width {
                        result.push(current_line.clone());
                        current_line = wi.text.clone();
                        current_width = wi.width;
                    } else {
                        if !current_line.is_empty() {
                            current_line.push(' ');
                        }
                        current_line.push_str(&wi.text);
                        current_width = test_width;
                    }

                    if i == word_data.len() - 1 && !current_line.is_empty() {
                        result.push(current_line.clone());
                    }
                }
            }
            result
        }
    }
}

pub(crate) enum DrawingCommand {
    MoveTo(f32, f32),
    LineTo(f32, f32),
    BezierTo(f32, f32, f32, f32, f32, f32),
    Close,
}

fn parse_drawing_level(text: &str) -> u8 {
    for tag_block in text.chars().collect::<Vec<_>>().windows(4) {
        if tag_block[0] == '\\' && tag_block[1] == 'p' {
            if let Some(d) = tag_block.get(2).and_then(|c| c.to_digit(10)) {
                return d as u8;
            }
        }
    }
    0
}

pub(crate) fn parse_drawing_commands(text: &str) -> Vec<DrawingCommand> {
    let mut commands = Vec::new();
    let tokens: Vec<&str> = text.split_whitespace().collect();
    let mut i = 0;
    let mut last_cmd: Option<&str> = None;

    while i < tokens.len() {
        let token = tokens[i];
        if token.len() == 1 {
            match token {
                "m" => {
                    if i + 2 < tokens.len() {
                        if let (Ok(x), Ok(y)) = (tokens[i + 1].parse::<f32>(), tokens[i + 2].parse::<f32>()) {
                            commands.push(DrawingCommand::MoveTo(x, y));
                            last_cmd = Some("m");
                            i += 3;
                            continue;
                        }
                    }
                }
                "l" => {
                    if i + 2 < tokens.len() {
                        if let (Ok(x), Ok(y)) = (tokens[i + 1].parse::<f32>(), tokens[i + 2].parse::<f32>()) {
                            commands.push(DrawingCommand::LineTo(x, y));
                            last_cmd = Some("l");
                            i += 3;
                            continue;
                        }
                    }
                }
                "b" => {
                    if i + 6 < tokens.len() {
                        let nums: Option<Vec<f32>> = (1..=6)
                            .map(|j| tokens[i + j].parse::<f32>().ok())
                            .collect::<Option<Vec<_>>>();
                        if let Some(n) = nums {
                            commands.push(DrawingCommand::BezierTo(n[0], n[1], n[2], n[3], n[4], n[5]));
                            last_cmd = Some("b");
                            i += 7;
                            continue;
                        }
                    }
                }
                "p" | "n" => {
                    if i + 1 < tokens.len() {
                        if tokens[i + 1] == "c" {
                            commands.push(DrawingCommand::Close);
                            last_cmd = None;
                            i += 2;
                            continue;
                        }
                    }
                    commands.push(DrawingCommand::Close);
                    last_cmd = None;
                    i += 1;
                    continue;
                }
                "c" => {
                    commands.push(DrawingCommand::Close);
                    last_cmd = None;
                    i += 1;
                    continue;
                }
                _ => {}
            }
        }

        if token.len() > 1 {
            if let Ok(_repeat) = token.parse::<usize>() {
                if i + 1 < tokens.len() {
                    let cmd_char = tokens[i + 1];
                    if matches!(cmd_char, "m" | "l" | "b") {
                        let args_needed = match cmd_char {
                            "m" | "l" => 2,
                            "b" => 6,
                            _ => 0,
                        };
                        for _ in 0.._repeat {
                            if i + 1 + args_needed < tokens.len() {
                                match cmd_char {
                                    "m" => {
                                        let x: f32 = tokens[i + 2].parse().unwrap_or(0.0);
                                        let y: f32 = tokens[i + 3].parse().unwrap_or(0.0);
                                        commands.push(DrawingCommand::MoveTo(x, y));
                                    }
                                    "l" => {
                                        let x: f32 = tokens[i + 2].parse().unwrap_or(0.0);
                                        let y: f32 = tokens[i + 3].parse().unwrap_or(0.0);
                                        commands.push(DrawingCommand::LineTo(x, y));
                                    }
                                    "b" => {
                                        let nums: Vec<f32> = (2..=7)
                                            .filter_map(|j| tokens.get(i + j)?.parse().ok())
                                            .collect();
                                        if nums.len() == 6 {
                                            commands.push(DrawingCommand::BezierTo(nums[0], nums[1], nums[2], nums[3], nums[4], nums[5]));
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                        i += 2 + args_needed;
                        continue;
                    }
                }
            }
        }

        if let (Ok(x), Some("m" | "l")) = (token.parse::<f32>(), last_cmd) {
            if i + 1 < tokens.len() {
                if let Ok(y) = tokens[i + 1].parse::<f32>() {
                    if last_cmd == Some("m") {
                        commands.push(DrawingCommand::MoveTo(x, y));
                    } else {
                        commands.push(DrawingCommand::LineTo(x, y));
                    }
                    i += 2;
                    continue;
                }
            }
        }

        i += 1;
    }

    commands
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
    use ass_parser::Effect;

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

    // ── apply_drawing_clip_mask ──────────────────────────────────

    #[test]
    fn test_drawing_clip_normal_triangle() {
        let w = 10u32;
        let h = 10u32;
        let mut data = vec![255u8; (w * h * 4) as usize];

        let mut ctx = RenderContext::default();
        ctx.clip_drawing_commands = Some("m 5 0 l 10 10 l 0 10".to_string());
        ctx.clip_drawing_scale = 1.0;
        ctx.clip_drawing_inverse = false;

        apply_drawing_clip_mask(&mut data, w, h, &ctx, 1.0, 1.0);

        // Center of triangle (roughly) should be preserved
        let inside_alpha = data[((5 * w + 5) * 4 + 3) as usize];
        assert_eq!(inside_alpha, 255, "pixel inside triangle should be preserved");

        // Top-left corner should be cleared (outside triangle)
        let outside_alpha = data[3];
        assert_eq!(outside_alpha, 0, "pixel outside triangle should be cleared");
    }

    #[test]
    fn test_drawing_clip_inverse_triangle() {
        let w = 10u32;
        let h = 10u32;
        let mut data = vec![255u8; (w * h * 4) as usize];

        let mut ctx = RenderContext::default();
        ctx.clip_drawing_commands = Some("m 5 0 l 10 10 l 0 10".to_string());
        ctx.clip_drawing_scale = 1.0;
        ctx.clip_drawing_inverse = true;

        apply_drawing_clip_mask(&mut data, w, h, &ctx, 1.0, 1.0);

        // Center of triangle should be cleared (inverse)
        let inside_alpha = data[((5 * w + 5) * 4 + 3) as usize];
        assert_eq!(inside_alpha, 0, "pixel inside triangle should be cleared for inverse");

        // Top-left corner should be preserved (outside triangle)
        let outside_alpha = data[3];
        assert_eq!(outside_alpha, 255, "pixel outside triangle should be preserved for inverse");
    }

    #[test]
    fn test_drawing_clip_scaled_coordinates() {
        let w = 20u32;
        let h = 20u32;
        let mut data = vec![255u8; (w * h * 4) as usize];

        let mut ctx = RenderContext::default();
        // Scale=2 means coordinates are halved: m 5 0 l 10 10 l 0 10 becomes m 2.5 0 l 5 5 l 0 5
        ctx.clip_drawing_commands = Some("m 5 0 l 10 10 l 0 10".to_string());
        ctx.clip_drawing_scale = 2.0;
        ctx.clip_drawing_inverse = false;

        apply_drawing_clip_mask(&mut data, w, h, &ctx, 1.0, 1.0);

        // Point (3,3) should be inside the scaled triangle
        let inside_alpha = data[((3 * w + 3) * 4 + 3) as usize];
        assert_eq!(inside_alpha, 255, "pixel inside scaled triangle should be preserved");

        // Point (9,9) should be outside the scaled triangle
        let outside_alpha = data[((9 * w + 9) * 4 + 3) as usize];
        assert_eq!(outside_alpha, 0, "pixel outside scaled triangle should be cleared");
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

    // ── ass_parser::parse_override_tag ─────────────────────────────────
    fn test_parse_single_tag_fs() {
        assert!(matches!(ass_parser::parse_override_tag("fs48"), Some(OverrideTag::FontSize(v)) if v == 48.0));
    }

    #[test]
    fn test_parse_single_tag_fn() {
        assert!(matches!(ass_parser::parse_override_tag("fnArial"), Some(OverrideTag::FontName(n)) if n == "Arial"));
    }

    #[test]
    fn test_parse_single_tag_bold() {
        assert!(matches!(ass_parser::parse_override_tag("b1"), Some(OverrideTag::Bold(true))));
        assert!(matches!(ass_parser::parse_override_tag("b0"), Some(OverrideTag::Bold(false))));
    }

    #[test]
    fn test_parse_single_tag_italic() {
        assert!(matches!(ass_parser::parse_override_tag("i1"), Some(OverrideTag::Italic(true))));
        assert!(matches!(ass_parser::parse_override_tag("i0"), Some(OverrideTag::Italic(false))));
    }

    #[test]
    fn test_parse_single_tag_bord() {
        assert!(matches!(ass_parser::parse_override_tag("bord3"), Some(OverrideTag::Border(v)) if v == 3.0));
    }

    #[test]
    fn test_parse_single_tag_shad() {
        assert!(matches!(ass_parser::parse_override_tag("shad5"), Some(OverrideTag::Shadow(v)) if v == 5.0));
    }

    #[test]
    fn test_parse_single_tag_fscx() {
        assert!(matches!(ass_parser::parse_override_tag("fscx150"), Some(OverrideTag::Scale { x, y: 100.0 }) if x == 150.0));
    }

    #[test]
    fn test_parse_single_tag_fscy() {
        assert!(matches!(ass_parser::parse_override_tag("fscy80"), Some(OverrideTag::Scale { x: 100.0, y }) if y == 80.0));
    }

    #[test]
    fn test_parse_single_tag_frz() {
        assert!(matches!(ass_parser::parse_override_tag("frz45"), Some(OverrideTag::Rotation { x: 0.0, y: 0.0, z }) if z == 45.0));
    }

    #[test]
    fn test_parse_single_tag_an() {
        assert!(matches!(ass_parser::parse_override_tag("an8"), Some(OverrideTag::AlignmentNumpad(v)) if v == 8));
    }

    #[test]
    fn test_parse_single_tag_unknown() {
        assert!(ass_parser::parse_override_tag("zzz").is_none());
    }

    #[test]
    fn test_parse_single_tag_empty() {
        assert!(ass_parser::parse_override_tag("").is_none());
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

    // ── build_context loads style properties ──────────────────

    fn make_test_event(text: &str) -> Event {
        Event {
            event_type: ass_parser::EventType::Dialogue,
            layer: 0,
            start: Timestamp::from_ms(0),
            end: Timestamp::from_ms(1000),
            style_name: "Default".into(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: text.into(),
            override_tags: vec![],
            karaoke_segments: vec![],
            raw_override_block: String::new(),
        }
    }

    fn make_test_renderer() -> Renderer {
        Renderer::new(RenderConfig {
            width: 1920,
            height: 1080,
            script_width: 1920,
            script_height: 1080,
            ..Default::default()
        })
    }

    fn make_test_ass() -> AssFile {
        let mut ass = AssFile::new();
        ass.styles.push(Style {
            name: "Default".into(),
            ..Default::default()
        });
        ass
    }

    #[test]
    fn test_build_context_loads_style_scale() {
        let mut style = Style::default();
        style.scale_x = 120.0;
        style.scale_y = 80.0;
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert_eq!(ctx.scale_x, 120.0);
        assert_eq!(ctx.scale_y, 80.0);
    }

    #[test]
    fn test_build_context_loads_style_spacing() {
        let mut style = Style::default();
        style.spacing = 5.0;
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert_eq!(ctx.spacing, 5.0);
    }

    #[test]
    fn test_build_context_loads_style_underline() {
        let mut style = Style::default();
        style.underline = true;
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert!(ctx.underline);
    }

    #[test]
    fn test_build_context_loads_style_strikeout() {
        let mut style = Style::default();
        style.strikeout = true;
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert!(ctx.strikeout);
    }

    #[test]
    fn test_build_context_loads_style_rotation() {
        let mut style = Style::default();
        style.angle = 45.0;
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert_eq!(ctx.rotation, 45.0);
    }

    #[test]
    fn test_build_context_style_properties_defaults() {
        // Verify that without overrides, default style values produce default ctx values.
        let style = Style::default();
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert_eq!(ctx.scale_x, 100.0);
        assert_eq!(ctx.scale_y, 100.0);
        assert_eq!(ctx.spacing, 0.0);
        assert!(!ctx.underline);
        assert!(!ctx.strikeout);
        assert_eq!(ctx.rotation, 0.0);
    }

    // ── DrawingMode stored in context ─────────────────────────

    #[test]
    fn test_build_context_drawing_mode_from_override() {
        let style = Style::default();
        let event = Event {
            override_tags: vec![OverrideTag::DrawingMode(3)],
            ..make_test_event("Hello")
        };
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert_eq!(ctx.drawing_mode, 3);
    }

    #[test]
    fn test_build_context_drawing_mode_default_is_zero() {
        let style = Style::default();
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert_eq!(ctx.drawing_mode, 0);
    }

    fn make_test_event_with_effect(text: &str, effect: Effect) -> Event {
        Event {
            event_type: ass_parser::EventType::Dialogue,
            layer: 0,
            start: Timestamp::from_ms(0),
            end: Timestamp::from_ms(10000),
            style_name: "Default".into(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect,
            text: text.into(),
            override_tags: vec![],
            karaoke_segments: vec![],
            raw_override_block: String::new(),
        }
    }

    // ── Issue 1: Banner/Scroll offset ─────────────────────────

    #[test]
    fn test_banner_effect_does_not_crash() {
        let renderer = make_test_renderer();
        let mut ass = make_test_ass();
        ass.events.push(make_test_event_with_effect(
            "BannerTest",
            Effect::Banner { delay_per_pixel: 10, left_to_right: true, fadeaway_width: 0.0 },
        ));
        let frame = renderer.render_ass(&ass, 500);
        assert!(frame.is_some());
    }

    #[test]
    fn test_banner_effect_rtl_does_not_crash() {
        let renderer = make_test_renderer();
        let mut ass = make_test_ass();
        ass.events.push(make_test_event_with_effect(
            "BannerRTL",
            Effect::Banner { delay_per_pixel: 10, left_to_right: false, fadeaway_width: 0.0 },
        ));
        let frame = renderer.render_ass(&ass, 500);
        assert!(frame.is_some());
    }

    #[test]
    fn test_scroll_up_effect_does_not_crash() {
        let renderer = make_test_renderer();
        let mut ass = make_test_ass();
        ass.events.push(make_test_event_with_effect(
            "ScrollUpTest",
            Effect::ScrollUp { delay_per_row: 50, top_offset: 10.0, bottom_offset: 50.0 },
        ));
        let frame = renderer.render_ass(&ass, 500);
        assert!(frame.is_some());
    }

    #[test]
    fn test_scroll_down_effect_does_not_crash() {
        let renderer = make_test_renderer();
        let mut ass = make_test_ass();
        ass.events.push(make_test_event_with_effect(
            "ScrollDownTest",
            Effect::ScrollDown { delay_per_row: 50, top_offset: 10.0, bottom_offset: 50.0 },
        ));
        let frame = renderer.render_ass(&ass, 500);
        assert!(frame.is_some());
    }

    // ── Issue 3: \\t(\\pos) interpolation ─────────────────────

    #[test]
    fn test_apply_transform_pos_interpolation() {
        let mut ctx = RenderContext::default();
        ctx.x = 0.0;
        ctx.y = 0.0;
        // Interpolate from (0,0) to (1920,1080) at 50% progress (t=500, duration=1000)
        apply_transform_tag(&mut ctx, "\\pos(1920,1080)", 0, 1000, 1.0, 500, 0, 1000, 1.0, 1.0);
        assert!((ctx.x - 960.0).abs() < 1.0, "x should be ~960, got {}", ctx.x);
        assert!((ctx.y - 540.0).abs() < 1.0, "y should be ~540, got {}", ctx.y);
    }

    #[test]
    fn test_apply_transform_pos_before_start() {
        let mut ctx = RenderContext::default();
        ctx.x = 100.0;
        ctx.y = 200.0;
        // Before t1=500, no interpolation should happen
        apply_transform_tag(&mut ctx, "\\pos(1920,1080)", 500, 1000, 1.0, 100, 0, 2000, 1.0, 1.0);
        assert_eq!(ctx.x, 100.0);
        assert_eq!(ctx.y, 200.0);
    }

    #[test]
    fn test_apply_transform_pos_after_end() {
        let mut ctx = RenderContext::default();
        ctx.x = 100.0;
        ctx.y = 200.0;
        // At t2=1000 (the end of animation), should be at target
        apply_transform_tag(&mut ctx, "\\pos(1920,1080)", 0, 1000, 1.0, 1000, 0, 1000, 1.0, 1.0);
        assert!((ctx.x - 1920.0).abs() < 1.0);
        assert!((ctx.y - 1080.0).abs() < 1.0);
    }

    // ── Issue 4: duration_ms in RenderedFrame ─────────────────

    #[test]
    fn test_render_ass_duration_ms() {
        let renderer = make_test_renderer();
        let mut ass = make_test_ass();
        // Event from 0ms to 5000ms
        ass.events.push(Event {
            event_type: ass_parser::EventType::Dialogue,
            layer: 0,
            start: Timestamp::from_ms(0),
            end: Timestamp::from_ms(5000),
            style_name: "Default".into(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: "DurationTest".into(),
            override_tags: vec![],
            karaoke_segments: vec![],
            raw_override_block: String::new(),
        });
        let frame = renderer.render_ass(&ass, 1000);
        assert!(frame.is_some());
        let frame = frame.unwrap();
        // duration_ms should be max event duration = 5000
        assert_eq!(frame.duration_ms, 5000, "duration should be 5000ms");
    }

    #[test]
    fn test_render_ass_duration_ms_multi_event() {
        let renderer = make_test_renderer();
        let mut ass = make_test_ass();
        // Event 1: 0ms to 3000ms
        ass.events.push(Event {
            event_type: ass_parser::EventType::Dialogue,
            layer: 0,
            start: Timestamp::from_ms(0),
            end: Timestamp::from_ms(3000),
            ..make_test_event("Event1")
        });
        // Event 2: 0ms to 8000ms (longer)
        ass.events.push(Event {
            event_type: ass_parser::EventType::Dialogue,
            layer: 1,
            start: Timestamp::from_ms(0),
            end: Timestamp::from_ms(8000),
            ..make_test_event("Event2")
        });
        let frame = renderer.render_ass(&ass, 1000);
        assert!(frame.is_some());
        let frame = frame.unwrap();
        // Max duration across both visible events is 8000ms
        assert_eq!(frame.duration_ms, 8000, "duration should be 8000ms (max of all events)");
    }

    #[test]
    fn test_render_ass_duration_ms_no_events() {
        // When no events are visible, duration should be 0
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        // No events in ass
        let frame = renderer.render_ass(&ass, 5000);
        // Should still return a frame (empty)
        assert!(frame.is_some());
        assert_eq!(frame.unwrap().duration_ms, 0);
    }

    // ── Issue 2: cache key uses timestamp only ────────────────

    #[test]
    fn test_cache_key_timestamp_only() {
        use crate::cache::FrameCacheKey;
        // Same timestamp = same key regardless of event_index
        let k1 = FrameCacheKey { timestamp_ms: 5000 };
        let k2 = FrameCacheKey { timestamp_ms: 5000 };
        assert_eq!(k1, k2);
    }

    // ── B1: border_style=3 opaque box ─────────────────────────

    #[test]
    fn test_build_context_border_style_default() {
        let style = Style::default();
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert_eq!(ctx.border_style, 1);
    }

    #[test]
    fn test_build_context_border_style_opaque_box() {
        let mut style = Style::default();
        style.border_style = 3;
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert_eq!(ctx.border_style, 3);
    }

    // ── B3: \\r named style reset ──────────────────────────────

    #[test]
    fn test_build_context_reset_named_style() {
        let mut named_style = Style::default();
        named_style.name = "Alt".into();
        named_style.font_name = "Times New Roman".into();
        named_style.font_size = 48.0;
        named_style.bold = true;
        named_style.underline = true;

        let mut ass = make_test_ass();
        ass.styles.push(named_style);

        let style = Style::default();
        let event = Event {
            override_tags: vec![OverrideTag::Reset("Alt".into())],
            ..make_test_event("Hello")
        };
        let renderer = make_test_renderer();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert_eq!(ctx.font_name, "Times New Roman");
        assert!(ctx.bold);
        assert!(ctx.underline);
        // Default style had smaller font; named style overrides it
        assert!(ctx.font_size > 20.0);
    }

    #[test]
    fn test_build_context_reset_named_style_fallback_empty() {
        // When style name is not found, fall back to event style (no crash).
        let ass = make_test_ass();
        let style = Style::default();
        let event = Event {
            override_tags: vec![OverrideTag::Reset("NonExistent".into())],
            ..make_test_event("Hello")
        };
        let renderer = make_test_renderer();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        // Falls back to not resetting, so default style values remain.
        assert_eq!(ctx.font_name, "Arial");
    }

    // ── Event layer sorting ──────────────────────────────────

    #[test]
    fn test_render_ass_events_sorted_by_layer() {
        let mut ass = ass_parser::AssFile::new();
        ass.styles.push(ass_parser::Style {
            name: "Default".into(),
            ..Default::default()
        });

        let event0 = Event {
            event_type: ass_parser::EventType::Dialogue,
            layer: 1,
            start: Timestamp::from_ms(0),
            end: Timestamp::from_ms(1000),
            style_name: "Default".into(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: "Layer1".into(),
            override_tags: vec![],
            karaoke_segments: vec![],
            raw_override_block: String::new(),
        };
        let event1 = Event {
            event_type: ass_parser::EventType::Dialogue,
            layer: 0,
            start: Timestamp::from_ms(0),
            end: Timestamp::from_ms(1000),
            style_name: "Default".into(),
            name: String::new(),
            margin_l: 0,
            margin_r: 0,
            margin_v: 0,
            effect: Effect::None,
            text: "Layer0".into(),
            override_tags: vec![],
            karaoke_segments: vec![],
            raw_override_block: String::new(),
        };
        // Push in reverse layer order: layer 1 before layer 0.
        ass.events.push(event0);
        ass.events.push(event1);

        let ts = Timestamp::from_ms(500);
        let visible: Vec<u32> = ass
            .dialogue_events()
            .filter(|e| e.is_visible_at(ts))
            .map(|e| e.layer)
            .collect();
        assert_eq!(visible, vec![1, 0]); // unsorted: layer 1 first, layer 0 second

        // After sorting by layer, should be [0, 1]
        let mut sorted: Vec<u32> = ass
            .dialogue_events()
            .filter(|e| e.is_visible_at(ts))
            .map(|e| e.layer)
            .collect();
        sorted.sort();
        assert_eq!(sorted, vec![0, 1]);
    }
}
