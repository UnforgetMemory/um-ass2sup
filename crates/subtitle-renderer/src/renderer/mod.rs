use ass_core::{
    Alignment, AssColor, BorderStyle, Event, FontEncoding, Margins, Style, SubtitleDocument,
};
use tiny_skia::Pixmap;

use crate::context::{RenderConfig, RenderContext, RenderedFrame};
use crate::renderer::font_registry_renderer::FontRegistryRenderResources;

use parking_lot::Mutex;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RendererError {
    #[error("no system fonts available — install fonts or pass a font directory")]
    NoFonts,
}

mod animation;
mod build_context;
pub(crate) mod drawing;
pub mod font_registry_karaoke;
pub mod font_registry_renderer;
pub mod layout_font_registry;
pub mod text_layout;
pub use text_layout::{alignment_to_pos, strip_override_blocks};

pub(crate) struct PixmapPool {
    pool: Vec<Pixmap>,
    max_cached: usize,
}

impl PixmapPool {
    pub(crate) fn new(max_cached: usize) -> Self {
        Self {
            pool: Vec::new(),
            max_cached,
        }
    }

    pub(crate) fn get(&mut self, w: u32, h: u32) -> Option<Pixmap> {
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

    pub(crate) fn put(&mut self, p: Pixmap) {
        if self.pool.len() < self.max_cached {
            self.pool.push(p);
        }
    }
}

pub struct Renderer {
    config: RenderConfig,
    pixmap_pool: Mutex<PixmapPool>,
    font_registry_render: Mutex<FontRegistryRenderResources>,
}

impl Renderer {
    pub fn new(config: RenderConfig) -> Self {
        Self {
            config,
            pixmap_pool: Mutex::new(PixmapPool::new(8)),
            font_registry_render: Mutex::new(FontRegistryRenderResources::new()),
        }
    }

    pub fn load_system_fonts(&self) -> usize {
        self.font_registry_render
            .lock()
            .registry
            .lock()
            .load_system_fonts()
    }

    pub fn load_user_fonts_dir(&self, dir: &std::path::Path) -> usize {
        self.font_registry_render
            .lock()
            .registry
            .lock()
            .load_user_fonts_dir(dir)
    }

    pub fn load_user_font_data(
        &self,
        data: Vec<u8>,
    ) -> Result<crate::font::types::FontId, crate::font::error::FontError> {
        self.font_registry_render
            .lock()
            .registry
            .lock()
            .load_user_font_data(data)
    }

    pub fn render_ass(&self, doc: &SubtitleDocument, timestamp_ms: u64) -> Option<RenderedFrame> {
        self.render_ass_inner(doc, timestamp_ms, &mut self.font_registry_render.lock())
    }

    fn render_ass_inner(
        &self,
        doc: &SubtitleDocument,
        timestamp_ms: u64,
        resources: &mut FontRegistryRenderResources,
    ) -> Option<RenderedFrame> {
        let fn_start = std::time::Instant::now();
        let mut pixmap = self
            .pixmap_pool
            .lock()
            .get(self.config.width, self.config.height)
            .or_else(|| Pixmap::new(self.config.width, self.config.height))?;

        let mut events: Vec<&Event> = doc
            .events
            .iter()
            .filter(|e| e.event_type == ass_core::EventType::Dialogue)
            .collect();
        events.retain(|e| e.start_ms <= timestamp_ms && timestamp_ms < e.end_ms);
        events.sort_by_key(|e| e.layer);

        let duration_ms = events
            .iter()
            .map(|e| e.end_ms.saturating_sub(e.start_ms))
            .max()
            .unwrap_or(0);

        for event in events {
            let event_start = std::time::Instant::now();
            let event_start_ms = event.start_ms;
            let event_end_ms = event.end_ms;

            let style = doc
                .styles
                .iter()
                .find(|s| s.name.as_str() == event.style.as_str())
                .cloned()
                .unwrap_or_else(|| Style {
                    name: event.style.clone(),
                    font_name: String::new(),
                    font_size: 0.0,
                    primary_color: AssColor::WHITE,
                    secondary_color: AssColor::WHITE,
                    outline_color: AssColor::BLACK,
                    shadow_color: AssColor::BLACK,
                    bold: false,
                    italic: false,
                    underline: false,
                    strikeout: false,
                    scale_x: 100.0,
                    scale_y: 100.0,
                    spacing: 0.0,
                    angle: 0.0,
                    border_style: BorderStyle::OutlineAndShadow,
                    outline: 0.0,
                    shadow: 0.0,
                    alignment: Alignment::BottomCenter,
                    margins: Margins::default(),
                    encoding: FontEncoding::default(),
                });
            let ctx = self.build_context(
                event,
                &style,
                doc,
                timestamp_ms,
                event_start_ms,
                event_end_ms,
            );

            crate::renderer::font_registry_renderer::render_event_font_registry(
                &mut pixmap,
                event,
                &ctx,
                &self.config,
                timestamp_ms,
                event_start_ms,
                resources,
            );

            let elapsed = event_start.elapsed();
            if elapsed.as_millis() > 500 {
                tracing::warn!(
                    timestamp_ms,
                    style = %event.style,
                    elapsed_ms = elapsed.as_millis() as u64,
                    "render_ass: SLOW event (>500ms)"
                );
            }
        }

        let frame = RenderedFrame {
            pts_ms: timestamp_ms,
            duration_ms,
            width: self.config.width,
            height: self.config.height,
            bitmap: pixmap.data().to_vec(),
        };
        self.pixmap_pool.lock().put(pixmap);
        tracing::trace!(
            timestamp_ms,
            total_us = fn_start.elapsed().as_micros() as u64,
            "render_ass: exit"
        );
        Some(frame)
    }

    pub fn build_context(
        &self,
        event: &Event,
        style: &Style,
        doc: &SubtitleDocument,
        timestamp_ms: u64,
        event_start_ms: u64,
        event_end_ms: u64,
    ) -> RenderContext {
        build_context::build_context(
            &self.config,
            event,
            style,
            doc,
            timestamp_ms,
            event_start_ms,
            event_end_ms,
        )
    }
}
