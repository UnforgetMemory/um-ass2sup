pub mod cache;
mod context;
pub mod effect_stack;
pub mod effects;
mod font;
pub mod karaoke;
mod rasterizer;
mod renderer;
mod shaper;
pub mod transform;

#[cfg(feature = "cosmic-text")]
pub mod font_cosmic;

pub use cache::{make_frame_key, FrameCache, FrameCacheKey};
pub use context::{RenderConfig, RenderContext, RenderedFrame};
pub use effect_stack::{EffectStack, RendererEffect};
pub use effects::{apply_gaussian_blur, apply_shadow, composite_over};
pub use font::{FontError, FontInfo, FontManager};
pub use karaoke::{KaraokePhase, KaraokeRenderer, SyllableState};
pub use renderer::{alignment_to_pos, strip_override_blocks, Renderer, RendererError};
pub use shaper::{GlyphBBox, ShapedGlyph, ShapedText, Shaper};
pub use transform::AffineTransform;

#[cfg(feature = "cosmic-text")]
pub use font_cosmic::{AssFallback, FallbackChain, FontResolver};
