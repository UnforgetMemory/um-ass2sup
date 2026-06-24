//! Application service layer — orchestration of the conversion pipeline.
//!
//! [`ConvertService`] wires together parsing, validation, font loading,
//! rendering, quantisation, and PGS / BDN / SRT encoding into a single
//! callable unit.

pub mod batch;
pub mod check;
pub mod convert;
pub mod srt;

pub use batch::convert_batch;
pub use check::run_check;
pub use convert::{convert_file, convert_to_bdn, ConversionPipeline, ConversionStats};
pub use srt::convert_to_srt;
