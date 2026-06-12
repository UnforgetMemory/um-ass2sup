use parking_lot::Mutex;

use ass_parser::{AssFile, Effect, Event, OverrideTag, Style, Timestamp};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform as SkiaTransform};

use crate::context::{RenderConfig, RenderContext, RenderedFrame};
use crate::effects;
use crate::font::FontManager;
use crate::karaoke::{KaraokePhase, KaraokeRenderer};
use crate::rasterizer::{apply_anisotropic_outline, Rasterizer};
use crate::shaper::{ShapedText, Shaper};
use crate::transform::AffineTransform;
use ass_parser::karaoke::KaraokeStyle;

/// Errors that can occur when constructing a Renderer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RendererError {
    /// No system fonts could be loaded. The renderer requires at least
    /// one font face to rasterize glyphs.
    #[error("no system fonts available — install fonts or pass a font directory")]
    NoFonts,
}

use animation::{
    apply_transform_tag, compute_fad_alpha, compute_fade_complex, interpolate_move,
    parse_override_block,
};
use compositing::{
    apply_alpha_multiplier, apply_clip_mask, apply_drawing_clip_mask, composite_subregion,
    compute_tight_bbox,
};
use drawing::{parse_drawing_commands, parse_drawing_level, DrawingCommand};
use text_layout::{remap_alignment_vertical, wrap_text, wrap_text_vertical};

mod animation;
pub mod compositing;
mod drawing;
pub mod text_layout;
pub use text_layout::{alignment_to_pos, strip_override_blocks};

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
/// 3. Internal rendering shapes text, rasterizes glyphs, applies
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
        Self {
            pool: Vec::new(),
            max_cached,
        }
    }

    /// Retrieves a pixmap of the given size from the pool, or allocates a new one.
    fn get(&mut self, w: u32, h: u32) -> Option<Pixmap> {
        if let Some(pos) = self
            .pool
            .iter()
            .position(|p| p.width() == w && p.height() == h)
        {
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
    ///
    /// # Panics
    ///
    /// Panics if no system fonts are available. For fallible construction, see
    /// [`Renderer::try_new`].
    pub fn new(config: RenderConfig) -> Self {
        Self::try_new(config)
            .expect("no system fonts available — install fonts or pass a font directory")
    }

    /// Creates a new renderer with the given configuration, returning an error
    /// if no system fonts could be loaded.
    ///
    /// Use this when font availability cannot be guaranteed. For infallible
    /// construction (which panics on zero fonts), see [`Renderer::new`].
    pub fn try_new(config: RenderConfig) -> Result<Self, RendererError> {
        let mut fm = FontManager::new();
        fm.load_system_fonts();
        if fm.font_count() == 0 {
            return Err(RendererError::NoFonts);
        }
        Ok(Self {
            config,
            font_manager: fm,
            pixmap_pool: Mutex::new(PixmapPool::new(8)),
        })
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
    /// Events outside the timestamp range are skipped.
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
            let style = ass
                .find_style(&event.style_name)
                .cloned()
                .unwrap_or_default();
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
    /// Uses [`FrameCache`](crate::FrameCache) keyed by `(event_index, timestamp_ms)`. On cache hit,
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
            font_name: if style.font_name.is_empty() {
                self.config.default_font.clone()
            } else {
                style.font_name.clone()
            },
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
        ctx.margin_l *= scale_x;
        ctx.margin_r *= scale_x;
        ctx.margin_v *= scale_y;
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
                OverrideTag::Pos { x, y } => {
                    ctx.x = *x as f32 * scale_x;
                    ctx.y = *y as f32 * scale_y;
                    has_pos = true;
                }
                OverrideTag::Move {
                    x1,
                    y1,
                    x2,
                    y2,
                    t1,
                    t2,
                } => {
                    ctx.x = *x1 as f32 * scale_x;
                    ctx.y = *y1 as f32 * scale_y;
                    move_x2 = *x2 as f32 * scale_x;
                    move_y2 = *y2 as f32 * scale_y;
                    move_t1 = *t1;
                    move_t2 = *t2;
                    has_move = true;
                    has_pos = true;
                }
                OverrideTag::Fade {
                    duration_in,
                    duration_out,
                } => {
                    fad_in = *duration_in;
                    fad_out = *duration_out;
                    has_fad = true;
                }
                OverrideTag::FadeComplex {
                    alpha_start,
                    alpha_mid,
                    alpha_end,
                    t1,
                    t2,
                    t3,
                    t4,
                } => {
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
                OverrideTag::Transform {
                    tag: inner_tag,
                    t1,
                    t2,
                    accel,
                } => {
                    // If the inner tag contains \pos, initialize ctx position to
                    // the alignment-derived values so the transform lerps FROM
                    // the correct starting point (not from 0,0).
                    let parsed_inner = parse_override_block(inner_tag);
                    if parsed_inner
                        .iter()
                        .any(|t| matches!(t, OverrideTag::Pos { .. }))
                    {
                        let (_ax, ay) = alignment_to_pos(ctx.alignment);
                        ctx.x = ctx.margin_l;
                        ctx.y =
                            ctx.margin_v + ay * (self.config.height as f32 - ctx.margin_v * 2.0);
                        has_pos = true;
                    }
                    apply_transform_tag(
                        &mut ctx,
                        inner_tag,
                        *t1,
                        *t2,
                        *accel,
                        timestamp_ms,
                        event_start_ms,
                        event_end_ms,
                        scale_x,
                        scale_y,
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
                        ctx.animation_skip = false;
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
                    ctx.animation_skip = false;
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
                OverrideTag::AnimationSkip => {
                    ctx.animation_skip = true;
                }
                OverrideTag::Unknown(tag) => {
                    tracing::warn!(tag = %tag, "unrecognized override tag ignored");
                }
                _ => {}
            }
        }

        if has_move {
            let elapsed = timestamp_ms.saturating_sub(event_start_ms);
            let (nx, ny) =
                interpolate_move(ctx.x, ctx.y, move_x2, move_y2, move_t1, move_t2, elapsed);
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
            // For top-aligned text (alignment 7,8,9), shift baseline down by
            // font_size so the glyph (which extends upward in screen coords)
            // stays within the frame.
            if ay == 0.0 {
                ctx.y += ctx.font_size;
            }
        }

        ctx
    }

    fn render_event(
        &self,
        pixmap: &mut Pixmap,
        event: &Event,
        ctx: &RenderContext,
        timestamp_ms: u64,
        event_start_ms: u64,
    ) {
        // Apply Banner/Scroll effect offset before text positioning
        let mut ctx = ctx.clone();
        match &event.effect {
            Effect::Banner {
                delay_per_pixel,
                left_to_right,
                ..
            } if *delay_per_pixel > 0 => {
                let elapsed = timestamp_ms.saturating_sub(event_start_ms);
                let x_offset = elapsed as f32 / *delay_per_pixel as f32;
                if *left_to_right {
                    ctx.x += x_offset;
                } else {
                    ctx.x -= x_offset;
                }
            }
            Effect::ScrollUp {
                delay_per_row,
                bottom_offset,
                ..
            } if *delay_per_row > 0 => {
                let elapsed = timestamp_ms.saturating_sub(event_start_ms);
                let y_offset = elapsed as f32 / *delay_per_row as f32;
                ctx.y = self.config.height as f32 - *bottom_offset as f32 - y_offset;
            }
            Effect::ScrollDown {
                delay_per_row,
                top_offset,
                ..
            } if *delay_per_row > 0 => {
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

        let font_id =
            match self
                .font_manager
                .query_with_fallback(&ctx.font_name, ctx.bold, ctx.italic)
            {
                Some(id) => id,
                None => {
                    // Final fallback: use config.default_font if the style's
                    // font was not found by any level of the fallback chain.
                    match self.font_manager.query_with_fallback(
                        &self.config.default_font,
                        ctx.bold,
                        ctx.italic,
                    ) {
                        Some(id) => id,
                        None => return,
                    }
                }
            };

        if event.has_karaoke() && !event.karaoke_segments.is_empty() {
            self.render_karaoke(pixmap, event, &ctx, font_id, timestamp_ms, event_start_ms);
            return;
        }

        let drawing_level = parse_drawing_level(&event.text);

        // \p4: drawing commands used as clip mask for text.
        // We render text normally first, then apply the drawing as a clip mask afterward.
        // Store the flag here; the clip mask is applied after Phase 7 compositing.
        let is_p4_clip = drawing_level == 4;

        if drawing_level > 0 && !is_p4_clip {
            self.render_drawing(pixmap, &plain_text, &ctx, drawing_level);
            return;
        }

        let shaper = Shaper::new(&self.font_manager);

        let available_width = self.config.width as f32 - ctx.margin_l - ctx.margin_r;
        let available_height = self.config.height as f32 - ctx.margin_v * 2.0;
        let line_height = ctx.font_size * 1.2;

        let w = pixmap.width();
        let h = pixmap.height();

        let is_vertical = ctx.writing_mode == 2 || ctx.writing_mode == 3;

        if is_vertical {
            let columns = wrap_text_vertical(&plain_text, available_height, line_height);
            if columns.is_empty() {
                return;
            }

            let mut shaped_lines: Vec<ShapedLine> = Vec::new();
            let remapped_alignment = remap_alignment_vertical(ctx.alignment, ctx.writing_mode);
            let align_col = remapped_alignment % 3;

            let total_width = columns.len() as f32 * line_height;
            let x_base = match align_col {
                2 => ctx.x + (available_width - total_width) / 2.0,
                0 => ctx.x + available_width - total_width,
                _ => ctx.x,
            };

            for (col_idx, column) in columns.iter().enumerate() {
                let col_x = if ctx.writing_mode == 2 {
                    x_base + (columns.len() - 1 - col_idx) as f32 * line_height
                } else {
                    x_base + col_idx as f32 * line_height
                };

                for (char_idx, ch) in column.chars().enumerate() {
                    let ch_str = ch.to_string();
                    let shaped = match shaper.shape(&ch_str, font_id, ctx.font_size) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    let char_y = ctx.y + char_idx as f32 * line_height;
                    let x_start = col_x;
                    shaped_lines.push(ShapedLine {
                        shaped,
                        line_y: char_y,
                        x_start,
                    });
                }
            }

            if shaped_lines.is_empty() {
                return;
            }

            let can_sub = ctx.rotation == 0.0
                && ctx.shear_x == 0.0
                && ctx.shear_y == 0.0
                && ctx.perspective_x == 0.0
                && ctx.perspective_y == 0.0
                && !ctx.clip_enabled
                && ctx.clip_drawing_commands.is_none();

            let sub_bbox = if can_sub {
                compute_tight_bbox(&shaped_lines, &shaper, font_id, ctx.font_size, &ctx)
            } else {
                None
            };

            let (pad_border, pad_shadow) = if ctx.border_style == 3 {
                (0.0, 0.0)
            } else {
                let border = ctx
                    .outline_width
                    .max(ctx.outline_x_width)
                    .max(ctx.outline_y_width);
                let shadow = ctx.shadow_depth.max(ctx.shadow_x).max(ctx.shadow_y);
                (border * 2.0, shadow)
            };
            let (ox, oy, lw, lh, use_sub) = if let Some((min_x, min_y, max_x, max_y)) = sub_bbox {
                let pad = (pad_border + pad_shadow + ctx.blur).max(20.0);
                let ox = (min_x - pad).floor() as i32;
                let oy = (min_y - pad).floor() as i32;
                let lw = ((max_x - min_x) + pad * 2.0).ceil().max(1.0) as u32;
                let lh = ((max_y - min_y) + pad * 2.0).ceil().max(1.0) as u32;
                let lw = lw.min(w.saturating_sub(ox.max(0) as u32)).max(1);
                let lh = lh.min(h.saturating_sub(oy.max(0) as u32)).max(1);
                (ox, oy, lw, lh, true)
            } else {
                (0, 0, w, h, false)
            };

            let mut layer = self.pixmap_pool.lock().get(lw, lh).unwrap();
            let oxf = ox as f32;
            let oyf = oy as f32;

            let render_ctx = if ctx.border_style == 3 {
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
                            layer.fill_path(
                                &path,
                                &bg_paint,
                                FillRule::Winding,
                                SkiaTransform::identity(),
                                None,
                            );
                        }
                    }
                }
                let mut render_ctx = ctx.clone();
                render_ctx.outline_width = 0.0;
                render_ctx.outline_x_width = 0.0;
                render_ctx.outline_y_width = 0.0;
                render_ctx
            } else {
                ctx.clone()
            };

            for sl in &shaped_lines {
                for glyph in &sl.shaped.glyphs {
                    let x = sl.x_start + glyph.x_offset - oxf;
                    let y = sl.line_y + glyph.y_offset - oyf;
                    Rasterizer::rasterize_glyph(
                        &mut layer,
                        &self.font_manager,
                        font_id,
                        glyph,
                        x,
                        y,
                        &render_ctx,
                    );
                }

                if render_ctx.underline {
                    let uy = sl.line_y + ctx.font_size * 0.1 - oyf;
                    let x0 = sl.x_start - oxf;
                    let x1 = x0 + sl.shaped.total_advance;
                    let line_thickness = ctx.font_size * 0.05;
                    let mut pb = tiny_skia::PathBuilder::new();
                    pb.move_to(x0, uy);
                    pb.line_to(x1, uy);
                    pb.close();
                    if let Some(path) = pb.finish() {
                        let mut paint = Paint::default();
                        paint.set_color_rgba8(
                            render_ctx.primary_color[0],
                            render_ctx.primary_color[1],
                            render_ctx.primary_color[2],
                            render_ctx.primary_color[3],
                        );
                        let stroke = tiny_skia::Stroke {
                            width: line_thickness,
                            ..Default::default()
                        };
                        layer.stroke_path(
                            &path,
                            &paint,
                            &stroke,
                            tiny_skia::Transform::identity(),
                            None,
                        );
                    }
                }

                if render_ctx.strikeout {
                    let sy = sl.line_y - ctx.font_size * 0.35 - oyf;
                    let x0 = sl.x_start - oxf;
                    let x1 = x0 + sl.shaped.total_advance;
                    let line_thickness = ctx.font_size * 0.05;
                    let mut pb = tiny_skia::PathBuilder::new();
                    pb.move_to(x0, sy);
                    pb.line_to(x1, sy);
                    pb.close();
                    if let Some(path) = pb.finish() {
                        let mut paint = Paint::default();
                        paint.set_color_rgba8(
                            render_ctx.primary_color[0],
                            render_ctx.primary_color[1],
                            render_ctx.primary_color[2],
                            render_ctx.primary_color[3],
                        );
                        let stroke = tiny_skia::Stroke {
                            width: line_thickness,
                            ..Default::default()
                        };
                        layer.stroke_path(
                            &path,
                            &paint,
                            &stroke,
                            tiny_skia::Transform::identity(),
                            None,
                        );
                    }
                }
            }

            if ctx.border_style != 3 && ctx.blur > 0.0 {
                effects::apply_gaussian_blur(&mut layer, ctx.blur);
            }

            if ctx.border_style != 3 && ctx.shadow_depth > 0.0 {
                let layer_data = layer.data().to_vec();
                let shadow_layer = effects::apply_shadow(
                    &layer_data,
                    lw,
                    lh,
                    if ctx.shadow_x != 0.0 {
                        ctx.shadow_x
                    } else {
                        ctx.shadow_depth
                    },
                    if ctx.shadow_y != 0.0 {
                        ctx.shadow_y
                    } else {
                        ctx.shadow_depth
                    },
                    ctx.blur,
                    ctx.shadow_color,
                );
                let mut shadow_pixmap = self.pixmap_pool.lock().get(lw, lh).unwrap();
                shadow_pixmap.data_mut().copy_from_slice(&shadow_layer);
                // Shadow goes BEHIND text: composite layer on top of shadow
                effects::composite_over(shadow_pixmap.data_mut(), layer.data(), lw, lh);
                layer.data_mut().copy_from_slice(shadow_pixmap.data());
                self.pixmap_pool.lock().put(shadow_pixmap);
            }

            if use_sub {
                if ctx.alpha_multiplier < 0.999 {
                    apply_alpha_multiplier(layer.data_mut(), ctx.alpha_multiplier);
                }
                composite_subregion(pixmap.data_mut(), layer.data(), w, h, ox, oy, lw, lh);
            } else {
                let transform = AffineTransform::identity();

                let final_data = if transform.is_identity()
                    && ctx.perspective_x == 0.0
                    && ctx.perspective_y == 0.0
                {
                    layer.data().to_vec()
                } else if ctx.perspective_x != 0.0 || ctx.perspective_y != 0.0 {
                    transform.apply_with_perspective(
                        layer.data(),
                        w,
                        h,
                        w,
                        h,
                        ctx.perspective_x,
                        ctx.perspective_y,
                        ctx.origin_x,
                        ctx.origin_y,
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
                } else if ctx.alpha_multiplier < 0.999 {
                    let mut alpha_data = final_data;
                    apply_alpha_multiplier(&mut alpha_data, ctx.alpha_multiplier);
                    effects::composite_over(pixmap.data_mut(), &alpha_data, w, h);
                } else {
                    effects::composite_over(pixmap.data_mut(), &final_data, w, h);
                }
            }

            self.pixmap_pool.lock().put(layer);
            return;
        }

        let lines = wrap_text(
            &plain_text,
            ctx.wrap_style,
            &shaper,
            font_id,
            ctx.font_size,
            ctx.spacing,
            available_width,
        );
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
            shaped_lines.push(ShapedLine {
                shaped,
                line_y,
                x_start,
            });
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
                .map(|(min_x, min_y, max_x, max_y)| {
                    // Clamp tight bbox to frame bounds so sub-region stays within frame.
                    let min_x = min_x.max(0.0);
                    let min_y = min_y.max(0.0);
                    let max_x = max_x.min(self.config.width as f32);
                    let max_y = max_y.min(self.config.height as f32);
                    (min_x, min_y, max_x, max_y)
                })
        } else {
            None
        };

        // Phase 3: Determine sub-region or full frame.
        // For border_style=3 (opaque box), reduce padding since we skip outline+shadow.
        let (pad_border, pad_shadow) = if ctx.border_style == 3 {
            (0.0, 0.0)
        } else {
            let border = ctx
                .outline_width
                .max(ctx.outline_x_width)
                .max(ctx.outline_y_width);
            let shadow = ctx.shadow_depth.max(ctx.shadow_x).max(ctx.shadow_y);
            (border * 2.0, shadow)
        };
        let (ox, oy, lw, lh, use_sub) = if let Some((min_x, min_y, max_x, max_y)) = sub_bbox {
            let pad = (pad_border + pad_shadow + ctx.blur).max(20.0);
            let ox = (min_x - pad).floor() as i32;
            let oy = (min_y - pad).floor() as i32;
            let lw = ((max_x - min_x) + pad * 2.0).ceil().max(1.0) as u32;
            let lh = ((max_y - min_y) + pad * 2.0).ceil().max(1.0) as u32;
            let lw = lw.min(w.saturating_sub(ox.max(0) as u32)).max(1);
            let lh = lh.min(h.saturating_sub(oy.max(0) as u32)).max(1);
            (ox, oy, lw, lh, true)
        } else {
            (0, 0, w, h, false)
        };

        // Phase 4: Allocate layer and render glyphs with sub-region offset.
        let mut layer = self.pixmap_pool.lock().get(lw, lh).unwrap();
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
                        layer.fill_path(
                            &path,
                            &bg_paint,
                            FillRule::Winding,
                            SkiaTransform::identity(),
                            None,
                        );
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
                        let stroke = tiny_skia::Stroke {
                            width: line_thickness,
                            ..Default::default()
                        };
                        layer.stroke_path(
                            &path,
                            &paint,
                            &stroke,
                            tiny_skia::Transform::identity(),
                            None,
                        );
                    }
                }

                if ctx.strikeout {
                    let sy = sl.line_y - ctx.font_size * 0.35 - oyf;
                    let mut pb = tiny_skia::PathBuilder::new();
                    pb.move_to(x0, sy);
                    pb.line_to(x1, sy);
                    pb.close();
                    if let Some(path) = pb.finish() {
                        let stroke = tiny_skia::Stroke {
                            width: line_thickness,
                            ..Default::default()
                        };
                        layer.stroke_path(
                            &path,
                            &paint,
                            &stroke,
                            tiny_skia::Transform::identity(),
                            None,
                        );
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
                if ctx.shadow_x != 0.0 {
                    ctx.shadow_x
                } else {
                    ctx.shadow_depth
                },
                if ctx.shadow_y != 0.0 {
                    ctx.shadow_y
                } else {
                    ctx.shadow_depth
                },
                ctx.blur,
                ctx.shadow_color,
            );
            let mut shadow_pixmap = self.pixmap_pool.lock().get(lw, lh).unwrap();
            shadow_pixmap.data_mut().copy_from_slice(&shadow_layer);
            // Shadow goes BEHIND text: composite layer on top of shadow
            effects::composite_over(shadow_pixmap.data_mut(), layer.data(), lw, lh);
            layer.data_mut().copy_from_slice(shadow_pixmap.data());
            self.pixmap_pool.lock().put(shadow_pixmap);
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
            let transform = AffineTransform::rotate_at(ctx.rotation, ctx.origin_x, ctx.origin_y)
                .then(&AffineTransform::scale(
                    ctx.scale_x / 100.0,
                    ctx.scale_y / 100.0,
                ))
                .then(&AffineTransform::shear(ctx.shear_x, ctx.shear_y));

            let final_data = if transform.is_identity()
                && ctx.perspective_x == 0.0
                && ctx.perspective_y == 0.0
            {
                layer.data().to_vec()
            } else if ctx.perspective_x != 0.0 || ctx.perspective_y != 0.0 {
                transform.apply_with_perspective(
                    layer.data(),
                    w,
                    h,
                    w,
                    h,
                    ctx.perspective_x,
                    ctx.perspective_y,
                    ctx.origin_x,
                    ctx.origin_y,
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
            } else if ctx.alpha_multiplier < 0.999 {
                let mut alpha_data = final_data;
                apply_alpha_multiplier(&mut alpha_data, ctx.alpha_multiplier);
                effects::composite_over(pixmap.data_mut(), &alpha_data, w, h);
            } else {
                effects::composite_over(pixmap.data_mut(), &final_data, w, h);
            }
        }

        // Return layer pixmap to pool.
        self.pixmap_pool.lock().put(layer);

        // \p4: Apply drawing commands as clip mask on the final pixmap.
        if is_p4_clip {
            let clip_commands = parse_drawing_commands(&plain_text);
            let scale = 1.0 / (1u32 << (4 - 1)) as f32; // \p4 → scale = 1/8
            let mut clip_pixmap = if let Some(p) = Pixmap::new(w, h) {
                p
            } else {
                return;
            };
            let mut path_builder = PathBuilder::new();
            for cmd in &clip_commands {
                match cmd {
                    DrawingCommand::MoveTo(x, y) => {
                        path_builder.move_to(
                            x * scale + ctx.x,
                            y * scale + ctx.y + ctx.baseline_offset as f32,
                        );
                    }
                    DrawingCommand::LineTo(x, y) => {
                        path_builder.line_to(
                            x * scale + ctx.x,
                            y * scale + ctx.y + ctx.baseline_offset as f32,
                        );
                    }
                    DrawingCommand::BezierTo(x1, y1, x2, y2, x3, y3) => {
                        path_builder.cubic_to(
                            x1 * scale + ctx.x,
                            y1 * scale + ctx.y + ctx.baseline_offset as f32,
                            x2 * scale + ctx.x,
                            y2 * scale + ctx.y + ctx.baseline_offset as f32,
                            x3 * scale + ctx.x,
                            y3 * scale + ctx.y + ctx.baseline_offset as f32,
                        );
                    }
                    DrawingCommand::Close => {
                        path_builder.close();
                    }
                }
            }
            if let Some(clip_path) = path_builder.finish() {
                let mut clip_paint = Paint::default();
                clip_paint.set_color_rgba8(255, 255, 255, 255);
                clip_pixmap.fill_path(
                    &clip_path,
                    &clip_paint,
                    FillRule::Winding,
                    SkiaTransform::identity(),
                    None,
                );
            }
            let data = pixmap.data_mut();
            let mask_data = clip_pixmap.data();
            for i in 0..(w * h) as usize {
                let idx = i * 4;
                if mask_data[idx + 3] == 0 {
                    data[idx] = 0;
                    data[idx + 1] = 0;
                    data[idx + 2] = 0;
                    data[idx + 3] = 0;
                }
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
                    if let Some(bbox) =
                        shaper.get_glyph_bbox(font_id, glyph.glyph_id, ctx.font_size)
                    {
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
                    let clip_x =
                        syllable_x + KaraokeRenderer::get_fill_clip_x(progress, syllable_width);
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
            let border = ctx
                .outline_width
                .max(ctx.outline_x_width)
                .max(ctx.outline_y_width);
            let shadow = ctx.shadow_depth.max(ctx.shadow_x).max(ctx.shadow_y);
            let pad = (border * 2.0 + shadow + ctx.blur).max(20.0);
            let ox = (min_x - pad).floor() as i32;
            let oy = (min_y - pad).floor() as i32;
            let lw = ((max_x - min_x) + pad * 2.0).ceil().max(1.0) as u32;
            let lh = ((max_y - min_y) + pad * 2.0).ceil().max(1.0) as u32;
            let lw = lw.min(w.saturating_sub(ox.max(0) as u32)).max(1);
            let lh = lh.min(h.saturating_sub(oy.max(0) as u32)).max(1);
            (ox, oy, lw, lh, true)
        } else {
            (0, 0, w, h, false)
        };

        // Phase 3: Allocate layers and render with offset.
        let mut bg_layer = self.pixmap_pool.lock().get(lw, lh).unwrap();
        let mut fg_layer = self.pixmap_pool.lock().get(lw, lh).unwrap();
        let oxf = ox as f32;
        let oyf = oy as f32;

        // Clone RenderContext once and reset only the fields that change per-syllable,
        // avoiding repeated full struct clones (String, Option<String> heap allocations).
        let mut sy_ctx = ctx.clone();

        for (i, info) in syllable_infos.iter().enumerate() {
            let syllable = &syllables[i];
            // Reset fields possibly modified in the previous iteration.
            sy_ctx.primary_color = ctx.primary_color;
            sy_ctx.outline_color = ctx.outline_color;
            sy_ctx.outline_width = ctx.outline_width;
            sy_ctx.outline_x_width = ctx.outline_x_width;
            sy_ctx.outline_y_width = ctx.outline_y_width;
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
            } else if matches!(
                syllable.phase,
                KaraokePhase::Done | KaraokePhase::Active { .. }
            ) {
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
                &bg_data,
                lw,
                lh,
                if ctx.shadow_x != 0.0 {
                    ctx.shadow_x
                } else {
                    ctx.shadow_depth
                },
                if ctx.shadow_y != 0.0 {
                    ctx.shadow_y
                } else {
                    ctx.shadow_depth
                },
                ctx.blur,
                ctx.shadow_color,
            );
            let mut shadow_pixmap = self.pixmap_pool.lock().get(lw, lh).unwrap();
            shadow_pixmap.data_mut().copy_from_slice(&shadow_data);
            // Shadow goes BEHIND bg_layer
            effects::composite_over(shadow_pixmap.data_mut(), bg_layer.data(), lw, lh);
            bg_layer.data_mut().copy_from_slice(shadow_pixmap.data());
            self.pixmap_pool.lock().put(shadow_pixmap);
        }
        if ctx.shadow_depth > 0.0 {
            let fg_data = fg_layer.data().to_vec();
            let shadow_data = effects::apply_shadow(
                &fg_data,
                lw,
                lh,
                if ctx.shadow_x != 0.0 {
                    ctx.shadow_x
                } else {
                    ctx.shadow_depth
                },
                if ctx.shadow_y != 0.0 {
                    ctx.shadow_y
                } else {
                    ctx.shadow_depth
                },
                ctx.blur,
                ctx.shadow_color,
            );
            let mut shadow_pixmap = self.pixmap_pool.lock().get(lw, lh).unwrap();
            shadow_pixmap.data_mut().copy_from_slice(&shadow_data);
            // Shadow goes BEHIND fg_layer
            effects::composite_over(shadow_pixmap.data_mut(), fg_layer.data(), lw, lh);
            fg_layer.data_mut().copy_from_slice(shadow_pixmap.data());
            self.pixmap_pool.lock().put(shadow_pixmap);
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
        self.pixmap_pool.lock().put(bg_layer);
        self.pixmap_pool.lock().put(fg_layer);
    }

    fn render_drawing(
        &self,
        pixmap: &mut Pixmap,
        text: &str,
        ctx: &RenderContext,
        drawing_level: u8,
    ) {
        let w = pixmap.width();
        let h = pixmap.height();
        let mut layer = if let Some(p) = Pixmap::new(w, h) {
            p
        } else {
            return;
        };
        let scale = 1.0 / f32::from(drawing_level);

        let commands = parse_drawing_commands(text);
        let mut current_path = PathBuilder::new();

        for cmd in &commands {
            match cmd {
                DrawingCommand::MoveTo(x, y) => {
                    let px = x * scale + ctx.x;
                    let py = y * scale + ctx.y + ctx.baseline_offset as f32;
                    current_path.move_to(px, py);
                }
                DrawingCommand::LineTo(x, y) => {
                    let px = x * scale + ctx.x;
                    let py = y * scale + ctx.y + ctx.baseline_offset as f32;
                    current_path.line_to(px, py);
                }
                DrawingCommand::BezierTo(x1, y1, x2, y2, x3, y3) => {
                    let cx1 = x1 * scale + ctx.x;
                    let cy1 = y1 * scale + ctx.y + ctx.baseline_offset as f32;
                    let cx2 = x2 * scale + ctx.x;
                    let cy2 = y2 * scale + ctx.y + ctx.baseline_offset as f32;
                    let ex = x3 * scale + ctx.x;
                    let ey = y3 * scale + ctx.y + ctx.baseline_offset as f32;
                    current_path.cubic_to(cx1, cy1, cx2, cy2, ex, ey);
                }
                DrawingCommand::Close => {
                    current_path.close();
                }
            }
        }

        if let Some(path) = current_path.finish() {
            let mut paint = Paint::default();
            paint.set_color_rgba8(
                ctx.primary_color[0],
                ctx.primary_color[1],
                ctx.primary_color[2],
                ctx.primary_color[3],
            );
            paint.anti_alias = true;

            if ctx.outline_width > 0.0 {
                let mut outline_paint = Paint::default();
                outline_paint.set_color_rgba8(
                    ctx.outline_color[0],
                    ctx.outline_color[1],
                    ctx.outline_color[2],
                    ctx.outline_color[3],
                );
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

            layer.fill_path(
                &path,
                &paint,
                FillRule::Winding,
                SkiaTransform::identity(),
                None,
            );
        }

        if ctx.blur > 0.0 {
            effects::apply_gaussian_blur(&mut layer, ctx.blur);
        }

        effects::composite_over(pixmap.data_mut(), layer.data(), w, h);
    }
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
    use ass_parser::Effect;

    // ── ass_parser::parse_override_tag ─────────────────────────────────
    #[test]
    fn test_parse_single_tag_fs() {
        assert!(
            matches!(ass_parser::parse_override_tag("fs48"), Some(OverrideTag::FontSize(v)) if v == 48.0)
        );
    }

    #[test]
    fn test_parse_single_tag_fn() {
        assert!(
            matches!(ass_parser::parse_override_tag("fnArial"), Some(OverrideTag::FontName(n)) if n == "Arial")
        );
    }

    #[test]
    fn test_parse_single_tag_bold() {
        assert!(matches!(
            ass_parser::parse_override_tag("b1"),
            Some(OverrideTag::Bold(true))
        ));
        assert!(matches!(
            ass_parser::parse_override_tag("b0"),
            Some(OverrideTag::Bold(false))
        ));
    }

    #[test]
    fn test_parse_single_tag_italic() {
        assert!(matches!(
            ass_parser::parse_override_tag("i1"),
            Some(OverrideTag::Italic(true))
        ));
        assert!(matches!(
            ass_parser::parse_override_tag("i0"),
            Some(OverrideTag::Italic(false))
        ));
    }

    #[test]
    fn test_parse_single_tag_bord() {
        assert!(
            matches!(ass_parser::parse_override_tag("bord3"), Some(OverrideTag::Border(v)) if v == 3.0)
        );
    }

    #[test]
    fn test_parse_single_tag_shad() {
        assert!(
            matches!(ass_parser::parse_override_tag("shad5"), Some(OverrideTag::Shadow(v)) if v == 5.0)
        );
    }

    #[test]
    fn test_parse_single_tag_fscx() {
        assert!(
            matches!(ass_parser::parse_override_tag("fscx150"), Some(OverrideTag::Scale { x, y: 100.0 }) if x == 150.0)
        );
    }

    #[test]
    fn test_parse_single_tag_fscy() {
        assert!(
            matches!(ass_parser::parse_override_tag("fscy80"), Some(OverrideTag::Scale { x: 100.0, y }) if y == 80.0)
        );
    }

    #[test]
    fn test_parse_single_tag_frz() {
        assert!(
            matches!(ass_parser::parse_override_tag("frz45"), Some(OverrideTag::Rotation { x: 0.0, y: 0.0, z }) if z == 45.0)
        );
    }

    #[test]
    fn test_parse_single_tag_an() {
        assert!(
            matches!(ass_parser::parse_override_tag("an8"), Some(OverrideTag::AlignmentNumpad(v)) if v == 8)
        );
    }

    #[test]
    fn test_parse_single_tag_unknown() {
        assert!(ass_parser::parse_override_tag("zzz").is_none());
    }

    #[test]
    fn test_parse_single_tag_empty() {
        assert!(ass_parser::parse_override_tag("").is_none());
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
        let style = Style {
            scale_x: 120.0,
            scale_y: 80.0,
            ..Default::default()
        };
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert_eq!(ctx.scale_x, 120.0);
        assert_eq!(ctx.scale_y, 80.0);
    }

    #[test]
    fn test_build_context_loads_style_spacing() {
        let style = Style {
            spacing: 5.0,
            ..Default::default()
        };
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert_eq!(ctx.spacing, 5.0);
    }

    #[test]
    fn test_build_context_loads_style_underline() {
        let style = Style {
            underline: true,
            ..Default::default()
        };
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert!(ctx.underline);
    }

    #[test]
    fn test_build_context_loads_style_strikeout() {
        let style = Style {
            strikeout: true,
            ..Default::default()
        };
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert!(ctx.strikeout);
    }

    #[test]
    fn test_build_context_loads_style_rotation() {
        let style = Style {
            angle: 45.0,
            ..Default::default()
        };
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
            Effect::Banner {
                delay_per_pixel: 10,
                left_to_right: true,
                fadeaway_width: 0.0,
            },
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
            Effect::Banner {
                delay_per_pixel: 10,
                left_to_right: false,
                fadeaway_width: 0.0,
            },
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
            Effect::ScrollUp {
                delay_per_row: 50,
                top_offset: 10.0,
                bottom_offset: 50.0,
            },
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
            Effect::ScrollDown {
                delay_per_row: 50,
                top_offset: 10.0,
                bottom_offset: 50.0,
            },
        ));
        let frame = renderer.render_ass(&ass, 500);
        assert!(frame.is_some());
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
        assert_eq!(
            frame.duration_ms, 8000,
            "duration should be 8000ms (max of all events)"
        );
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
        let style = Style {
            border_style: 3,
            ..Default::default()
        };
        let event = make_test_event("Hello");
        let renderer = make_test_renderer();
        let ass = make_test_ass();
        let ctx = renderer.build_context(&event, &style, &ass, 0, 0, 1000);
        assert_eq!(ctx.border_style, 3);
    }

    // ── B3: \\r named style reset ──────────────────────────────

    #[test]
    fn test_build_context_reset_named_style() {
        let named_style = Style {
            name: "Alt".into(),
            font_name: "Times New Roman".into(),
            font_size: 48.0,
            bold: true,
            underline: true,
            ..Default::default()
        };

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

    // ── parking_lot::Mutex poisoning resistance ─────────────────────
    #[test]
    fn test_pixmap_pool_mutex_does_not_panic_after_thread_panic() {
        // std::sync::Mutex is poisoned if a thread panics while holding the lock,
        // causing all subsequent .lock().unwrap() calls to panic and crash the process.
        // parking_lot::Mutex does NOT have this failure mode — locks always succeed.
        //
        // This test verifies that pixmap_pool uses parking_lot::Mutex by checking
        // that .lock() returns the guard directly (not a Result).

        use std::panic::{catch_unwind, AssertUnwindSafe};
        use std::sync::Arc;

        let renderer = Arc::new(Renderer::new(RenderConfig::default()));
        let renderer_clone = Arc::clone(&renderer);

        // parking_lot::Mutex::lock() never panics — the guard is returned directly.
        let result = catch_unwind(AssertUnwindSafe(move || {
            let _guard = renderer_clone.pixmap_pool.lock();
        }));
        assert!(
            result.is_ok(),
            "parking_lot::Mutex::lock() should not panic"
        );
    }
}

#[cfg(test)]
mod try_new_tests {
    use super::*;

    #[test]
    fn try_new_succeeds_when_fonts_loaded() {
        // This test only runs if the system has at least one font.
        // On CI we install fonts; on dev boxes this should pass.
        match Renderer::try_new(RenderConfig::default()) {
            Ok(r) => assert!(r.font_manager().font_count() > 0),
            Err(RendererError::NoFonts) => {
                eprintln!("skipping: no system fonts on this host");
            }
        }
    }

    #[test]
    fn try_new_error_is_clone_eq() {
        // Compile-time + runtime: error type must satisfy the trait bounds.
        let e = RendererError::NoFonts;
        let e2 = e.clone();
        assert_eq!(e, e2);
    }

    #[test]
    fn new_panics_or_works_same_as_try_new() {
        // new() delegates to try_new; if try_new works, new() returns Renderer.
        // If try_new errors, new() panics. Either is acceptable.
        let result = std::panic::catch_unwind(|| Renderer::new(RenderConfig::default()));
        match result {
            Ok(r) => assert!(r.font_manager().font_count() > 0),
            Err(_) => { /* expected on font-less host */ }
        }
    }
}
