pub mod database;
pub mod discovery;
pub mod error;
pub mod index;
pub mod rasterizer;
pub mod registry;
pub mod shaper;
pub mod telemetry;
/// Font subsystem for the subtitle renderer.
///
/// This module is responsible for font discovery, indexing, glyph
/// shaping, and rasterization. It is organised into the following
/// sub-modules:
///
/// * [`types`]        — Pure domain types (FontId, FontWeight, …)
/// * [`error`]        — Domain error types
/// * [`discovery`]    — Font file discovery (directories, system)
/// * [`telemetry`]    — Font loading metrics and diagnostics
/// * [`index`]        — Font face index / database
/// * [`database`]     — High-level font database
/// * [`shaper`]       — Glyph shaping (harfbuzz/rustybuzz)
/// * [`rasterizer`]   — Glyph rasterization
/// * [`registry`]     — Central font registry
pub mod types;

// Re-export the most common types at the module level for convenience.
pub use error::FontError;
pub use types::*;
