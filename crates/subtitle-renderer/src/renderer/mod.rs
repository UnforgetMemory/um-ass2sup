use ass_parser::{AssFile, Event, Style, Timestamp};
use tiny_skia::Pixmap;

use crate::context::{RenderConfig, RenderContext, RenderedFrame};
use crate::renderer::cosmic::CosmicRenderResources;

use parking_lot::Mutex;

/// Errors that can occur when constructing a Renderer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RendererError {
    /// No system fonts could be loaded. The renderer requires at least
    /// one font face to rasterize glyphs.
    #[error("no system fonts available — install fonts or pass a font directory")]
    NoFonts,
}

mod animation;
mod build_context;
pub mod compositing;
pub mod context;
pub mod cosmic;
pub mod cosmic_karaoke;
pub(crate) mod drawing;
mod layout;
pub mod text_layout;
pub use text_layout::alignment_to_pos;

/// Reusable pixmap buffer pool to reduce allocations across events.
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

/// ASS subtitle renderer that produces RGBA bitmaps using cosmic-text.
pub struct Renderer {
    config: RenderConfig,
    pixmap_pool: Mutex<PixmapPool>,
    cosmic_render: Mutex<CosmicRenderResources>,
}

impl Renderer {
    /// Creates a new renderer with the given configuration.
    pub fn new(config: RenderConfig) -> Self {
        Self {
            config,
            pixmap_pool: Mutex::new(PixmapPool::new(8)),
            cosmic_render: Mutex::new(CosmicRenderResources::new()),
        }
    }

    /// Returns the cosmic render resources for font loading.
    pub fn cosmic_render(&self) -> parking_lot::MutexGuard<'_, CosmicRenderResources> {
        self.cosmic_render.lock()
    }

    /// Renders all visible dialogue events at the given timestamp to an RGBA frame.
    pub fn render_ass(&self, ass: &AssFile, timestamp_ms: u64) -> Option<RenderedFrame> {
        self.render_ass_cosmic_inner(ass, timestamp_ms, &mut self.cosmic_render.lock())
    }

    /// Cosmic-text render loop.
    fn render_ass_cosmic_inner(
        &self,
        ass: &AssFile,
        timestamp_ms: u64,
        cosmic: &mut CosmicRenderResources,
    ) -> Option<RenderedFrame> {
        let fn_start = std::time::Instant::now();
        let ts = Timestamp::from_ms(timestamp_ms);
        let mut pixmap = self
            .pixmap_pool
            .lock()
            .get(self.config.width, self.config.height)
            .or_else(|| Pixmap::new(self.config.width, self.config.height))?;

        let mut events: Vec<&Event> = ass.dialogue_events().collect();
        events.retain(|e| e.start <= ts && ts < e.end);
        events.sort_by_key(|e| e.layer);

        let duration_ms = events
            .iter()
            .map(|e| e.end.as_ms().saturating_sub(e.start.as_ms()))
            .max()
            .unwrap_or(0);

        tracing::trace!(
            timestamp_ms,
            visible_events = events.len(),
            "render_ass: events filtered"
        );

        for event in events {
            let event_start = std::time::Instant::now();
            let event_start_ms = event.start.as_ms();
            let event_end_ms = event.end.as_ms();

            let style = ass
                .find_style(&event.style_name)
                .cloned()
                .unwrap_or_default();
            let ctx = self.build_context(
                event,
                &style,
                ass,
                timestamp_ms,
                event_start_ms,
                event_end_ms,
            );

            crate::renderer::cosmic::render_event_cosmic(
                &mut pixmap,
                event,
                &ctx,
                &self.config,
                timestamp_ms,
                event_start_ms,
                cosmic,
            );

            let elapsed = event_start.elapsed();
            if elapsed.as_millis() > 500 {
                tracing::warn!(
                    timestamp_ms,
                    style = %event.style_name,
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
        ass: &AssFile,
        timestamp_ms: u64,
        event_start_ms: u64,
        event_end_ms: u64,
    ) -> RenderContext {
        build_context::build_context(
            &self.config,
            event,
            style,
            ass,
            timestamp_ms,
            event_start_ms,
            event_end_ms,
        )
    }
}
