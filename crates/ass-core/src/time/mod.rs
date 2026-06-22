//! Core time types for ASS/SSA/SRT subtitle timing.
//!
//! This module provides:
//! - [`Timestamp`]: millisecond-precision time value
//! - [`Fps`]: rational frame rate (no floating point)
//! - Conversion functions: ms ↔ 90kHz PTS, ASS timecode, SRT timecode, etc.

mod convert;
mod fps;
mod timestamp;

pub use convert::{
    ms_to_90khz, ms_to_ass_timecode, ms_to_frame_timecode, ms_to_srt_timecode, ms_to_ttml_timecode,
    snap_to_frame,
};
pub use fps::Fps;
pub use timestamp::Timestamp;
