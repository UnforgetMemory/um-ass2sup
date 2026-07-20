#![warn(missing_docs)]
#![doc = "libass-based ASS to SUP conversion backend."]

pub mod domain;
pub mod infra;

// ── Re-exports for external consumers (ass2sup-cli) ──────────────

pub use domain::composer::compose_frame;
pub use domain::error::AssError;
pub use domain::frame::{AssEventInfo, AssImageData, CroppedFrame, ImageType, RgbaFrame};
pub use domain::pipeline::ConversionConfig;
pub use domain::renderer::{extract_font_families, AssRenderer};
pub use domain::timeline::generate_timestamps;
pub use infra::pgs_adapter::{
    create_pipeline, encode_bdn, encode_sup, frame_accurate_pts, quantize_frame,
};
pub use infra::vendor::{composite_over, crop_to_tight_bbox};

/// Compute a content hash for a quantized frame (indices + palette + x + y).
pub use domain::pipeline::hash_quantized;
