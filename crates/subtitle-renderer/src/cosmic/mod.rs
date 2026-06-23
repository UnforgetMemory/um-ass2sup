//! Cosmic-text font backend — alternative to fontdb + rustybuzz + ttf-parser.
//!
//! Provides bundled font resolution, shaping, and rasterization through
//! the [`cosmic_text`] crate.

pub mod effects;
pub mod rasterizer;
pub mod resolver;
pub mod shaper;
pub mod spans;

pub use rasterizer::rasterize_cosmic_glyph;
pub use resolver::FontCosmicResolver;
pub use shaper::{CosmicShapedGlyph, CosmicShaper};
pub use spans::parse_spans;
