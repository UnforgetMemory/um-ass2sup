#![allow(missing_docs)]

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ColorError {
    #[error("empty color buffer")]
    EmptyBuffer,
    #[error("invalid dimensions: {width}×{height}")]
    InvalidDimensions { width: u32, height: u32 },
    #[error("palette is full (max {0} colors)")]
    PaletteFull(u8),
    #[error("color space mismatch: expected {expected:?}, got {got:?}")]
    ColorSpaceMismatch {
        expected: crate::color::ColorSpace,
        got: crate::color::ColorSpace,
    },
    #[error("quantization failed")]
    QuantizeError,
}
