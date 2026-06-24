//! Type definitions for PGS encoding (re-exported from domain modules).
//!
//! This file exists for backward compatibility. New code should import
//! directly from `crate::domain::{segment, palette, composition}`.

pub use crate::domain::composition::{CompositionState, ObjectComposition, WindowDef};
pub use crate::domain::palette::PaletteEntry;
pub use crate::domain::segment::{
    OdsPayload, PcsPayload, PdsPayload, Segment, SegmentPayload, SegmentType, SupFile, WdsPayload,
};
pub use crate::domain::timing::frame_rate_code;
