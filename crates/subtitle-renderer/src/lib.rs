pub mod font;

mod context;
pub mod effects;
pub mod karaoke;
mod renderer;
pub mod transform;

pub use context::{RenderConfig, RenderContext, RenderedFrame};
pub use karaoke::{KaraokePhase, KaraokeRenderer, SyllableState};
pub use renderer::{
    Renderer, RendererError, alignment_to_pos, parse_font_name, strip_override_blocks,
};
pub use transform::AffineTransform;
