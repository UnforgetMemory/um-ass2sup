//! Renderer backend abstraction (v2.0).
//!
//! The v2.0 plan defines a `RendererBackend` trait that abstracts the
//! CPU and GPU rendering paths behind a single interface. The existing
//! `Renderer` (CPU-only, tiny-skia) continues to be the default; the GPU
//! path (vello) is a follow-up implementation behind the `vello` cargo
//! feature.
//!
//! # Why
//!
//! The current renderer emits pixel bitmaps via tiny-skia. For large ASS
//! files (> 5000 events), this is the bottleneck. The v2.0 plan
//! targets a 10x speedup via GPU rendering using vello (pure Rust,
//! wgpu-backed). The backend trait provides the seam: the renderer
//! composes glyphs, effects, and bitmap output through a single
//! `RendererBackend` so adding a GPU implementation is a drop-in
//! addition rather than a rewrite.

/// 2D point in script coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// Horizontal coordinate.
    pub x: f32,
    /// Vertical coordinate.
    pub y: f32,
}

impl Point {
    /// Build a new point.
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// Axis-aligned rectangle in script coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rect {
    /// Left edge.
    pub x: f32,
    /// Top edge.
    pub y: f32,
    /// Width.
    pub width: f32,
    /// Height.
    pub height: f32,
}

impl Rect {
    /// Build a new rectangle.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

/// Colour in the renderer's working space (sRGB-encoded RGBA, 8-bit
/// channels). The GPU and CPU backends agree on this format.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    /// Red channel (0-255).
    pub r: u8,
    /// Green channel (0-255).
    pub g: u8,
    /// Blue channel (0-255).
    pub b: u8,
    /// Alpha channel (0-255; 255 = fully opaque).
    pub a: u8,
}

impl Color {
    /// Opaque black.
    pub const BLACK: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    /// Opaque white.
    pub const WHITE: Color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    /// Fully transparent.
    pub const TRANSPARENT: Color = Color {
        r: 0,
        g: 0,
        b: 0,
        a: 0,
    };

    /// Build a colour from RGBA components.
    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }
}

/// Identifier for a glyph within a font face. The exact representation
/// is backend-specific (CPU: ttf-parser glyph ID; GPU: cosmic-text
/// cache key); the renderer passes it through opaquely.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphId(pub u32);

/// Minimal glyph descriptor: position + colour + identifier. The
/// backend handles font lookup and rasterization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Glyph {
    /// Glyph identifier (font-face-scoped).
    pub id: GlyphId,
    /// Position of the glyph's anchor point.
    pub pos: Point,
    /// Glyph fill colour.
    pub color: Color,
}

/// Effect type for `apply_effect`. Mirrors the renderer-side
/// [`crate::effect_stack::RendererEffect`] but exposes only the GPU
/// fast path: the simpler cases (FillRect, Blur) that vello can
/// handle directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BackendEffect {
    /// Fill a rectangle with a solid colour.
    FillRect {
        /// The rectangle.
        rect: Rect,
        /// The fill colour.
        color: Color,
    },
    /// Apply a Gaussian blur with the given radius (pixels).
    Blur {
        /// The region to blur.
        rect: Rect,
        /// Blur radius.
        radius: f32,
    },
}

/// Output of a render pass: width, height, RGBA bitmap (8-bit per
/// channel, row-major). The renderer can then quantize this to a PGS
/// palette.
#[derive(Debug, Clone)]
pub struct RenderedBitmap {
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
    /// RGBA byte array, row-major, length = width * height * 4.
    pub data: Vec<u8>,
}

impl RenderedBitmap {
    /// Build an empty bitmap of the given size.
    pub fn empty(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            data: vec![0; (width * height * 4) as usize],
        }
    }
}

/// Renderer backend abstraction. CPU and GPU implementations both
/// satisfy this trait so the renderer can dispatch transparently.
pub trait RendererBackend: Send + Sync {
    /// Draw a single glyph at the given position with the given colour.
    fn draw_glyph(&mut self, glyph: &Glyph);

    /// Fill a rectangle with a solid colour.
    fn fill_rect(&mut self, rect: Rect, color: Color);

    /// Apply a post-processing effect (blur, etc.).
    fn apply_effect(&mut self, effect: BackendEffect);

    /// Finalize the current frame and return the rendered bitmap.
    fn finalize(&mut self) -> RenderedBitmap;
}

/// Backend dispatch strategy. Decides CPU vs GPU based on event count.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BackendPolicy {
    /// Always use the CPU backend.
    #[default]
    CpuOnly,
    /// Always use the GPU backend (if compiled in).
    GpuOnly,
    /// Hybrid: use CPU for small tasks and GPU for large ones.
    Hybrid,
}

impl BackendPolicy {
    /// Pick a backend for the given event count, given whether a GPU
    /// implementation is available.
    pub fn select(&self, event_count: usize, gpu_available: bool) -> &'static str {
        match self {
            BackendPolicy::CpuOnly => "cpu",
            BackendPolicy::GpuOnly if gpu_available => "gpu",
            BackendPolicy::GpuOnly => "cpu",
            BackendPolicy::Hybrid if gpu_available && event_count >= 100 => "gpu",
            BackendPolicy::Hybrid => "cpu",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn point_construction() {
        let p = Point::new(10.0, 20.0);
        assert_eq!(p.x, 10.0);
        assert_eq!(p.y, 20.0);
    }

    #[test]
    fn rect_construction() {
        let r = Rect::new(0.0, 0.0, 100.0, 200.0);
        assert_eq!(r.x, 0.0);
        assert_eq!(r.width, 100.0);
        assert_eq!(r.height, 200.0);
    }

    #[test]
    fn color_constants() {
        assert_eq!(Color::BLACK.r, 0);
        assert_eq!(Color::WHITE.r, 255);
        assert_eq!(Color::TRANSPARENT.a, 0);
    }

    #[test]
    fn empty_bitmap_has_correct_size() {
        let b = RenderedBitmap::empty(10, 20);
        assert_eq!(b.width, 10);
        assert_eq!(b.height, 20);
        assert_eq!(b.data.len(), 10 * 20 * 4);
    }

    #[test]
    fn cpu_only_always_picks_cpu() {
        assert_eq!(BackendPolicy::CpuOnly.select(0, false), "cpu");
        assert_eq!(BackendPolicy::CpuOnly.select(10000, true), "cpu");
    }

    #[test]
    fn gpu_only_picks_gpu_when_available() {
        assert_eq!(BackendPolicy::GpuOnly.select(0, true), "gpu");
        assert_eq!(BackendPolicy::GpuOnly.select(0, false), "cpu");
    }

    #[test]
    fn hybrid_uses_gpu_only_for_large_event_counts() {
        // 99 events -> cpu
        assert_eq!(BackendPolicy::Hybrid.select(99, true), "cpu");
        // 100 events -> gpu
        assert_eq!(BackendPolicy::Hybrid.select(100, true), "gpu");
        // 5000 events -> gpu
        assert_eq!(BackendPolicy::Hybrid.select(5000, true), "gpu");
        // 5000 events without gpu -> cpu
        assert_eq!(BackendPolicy::Hybrid.select(5000, false), "cpu");
    }

    #[test]
    fn default_policy_is_cpu_only() {
        assert_eq!(BackendPolicy::default(), BackendPolicy::CpuOnly);
    }

    #[test]
    fn glyph_id_round_trip() {
        let id = GlyphId(42);
        assert_eq!(id.0, 42);
    }
}
