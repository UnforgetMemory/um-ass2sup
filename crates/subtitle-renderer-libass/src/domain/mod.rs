//! Domain types and logic for ASS-to-SUP conversion.

/// Frame composition: blend libass layers onto an RGBA canvas.
pub mod composer;
/// Domain error types.
pub mod error;
/// Font file cache persisted to disk.
pub mod font_cache;
/// Value objects for images, frames, and event metadata.
pub mod frame;
/// Pipeline orchestration: ASS parse → render → quantize → encode.
pub mod pipeline;
/// Rendering bridge: libass track management and frame rasterization.
pub mod renderer;
/// Timeline management: event scheduling and frame-accurate time mapping.
pub mod timeline;
