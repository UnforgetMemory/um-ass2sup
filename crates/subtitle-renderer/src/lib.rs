pub mod cache;
mod context;
pub mod effects;
mod font;
pub mod karaoke;
mod rasterizer;
mod renderer;
mod shaper;
pub mod transform;

pub use cache::{make_frame_key, FrameCache, FrameCacheKey};
pub use context::{RenderConfig, RenderContext, RenderedFrame};
pub use effects::{apply_gaussian_blur, apply_shadow, composite_over};
pub use font::{FontError, FontInfo, FontManager};
pub use karaoke::{KaraokePhase, KaraokeRenderer, SyllableState};
pub use renderer::{alignment_to_pos, strip_override_blocks, Renderer, RendererError};
pub use shaper::{GlyphBBox, ShapedGlyph, ShapedText, Shaper};
pub use transform::AffineTransform;
