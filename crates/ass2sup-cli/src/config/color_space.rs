//! Colour-space selection helpers.
//!
//! Thin wrappers that map CLI argument strings to upstream quantizer types.

use color_quantizer::color::tonemap::ToneMapOperator;
use color_quantizer::color::ColorSpace;

/// Parse a CLI `--color-space` value.
pub fn parse_color_space(s: &str) -> ColorSpace {
    match s {
        "bt709" => ColorSpace::Bt709,
        "bt2020" => ColorSpace::Bt2020,
        _ => ColorSpace::Srgb,
    }
}

/// Parse a CLI `--tonemap` value.
pub fn parse_tonemap(s: &str) -> ToneMapOperator {
    match s {
        "hable" => ToneMapOperator::Hable,
        "reinhard" => ToneMapOperator::Reinhard,
        "aces" => ToneMapOperator::Aces,
        _ => ToneMapOperator::Reinhard,
    }
}

/// Parse a CLI `--dither` value.
pub fn parse_dither(s: &str) -> color_quantizer::DitherMethod {
    match s {
        "none" => color_quantizer::DitherMethod::None,
        "ordered" => color_quantizer::DitherMethod::Ordered,
        _ => color_quantizer::DitherMethod::FloydSteinberg,
    }
}
