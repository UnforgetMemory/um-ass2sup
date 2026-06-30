//! Color tags: `\1c`/`\c`, `\2c`-`\4c`, `\1a`-`\4a`, `\alpha`.
use super::util::{parse_ass_color, parse_hex_u8};
use crate::OverrideTag;

/// Parse \c (primary color) or \3c (outline color) tag.
pub fn parse(s: &str) -> Option<OverrideTag> {
    // Colour aliases (with \c as alias for \1c)
    for (prefix, variant) in [
        ("1c", "primary"),
        ("2c", "secondary"),
        ("3c", "outline"),
        ("4c", "shadow"),
        ("c", "primary"),
    ] {
        if let Some(color_str) = s.strip_prefix(prefix) {
            if let Ok(color) = parse_ass_color(color_str) {
                return Some(match variant {
                    "primary" => OverrideTag::PrimaryColor(color),
                    "secondary" => OverrideTag::SecondaryColor(color),
                    "outline" => OverrideTag::OutlineColor(color),
                    "shadow" => OverrideTag::ShadowColor(color),
                    _ => unreachable!(),
                });
            }
        }
    }
    // Global alpha
    if let Some(val) = s.strip_prefix("alpha") {
        if let Ok(v) = parse_hex_u8(val) {
            return Some(OverrideTag::Alpha { value: v });
        }
    }
    // Per-channel alpha
    for (prefix, variant) in [
        ("1a", "primary"),
        ("2a", "secondary"),
        ("3a", "outline"),
        ("4a", "shadow"),
    ] {
        if let Some(val) = s.strip_prefix(prefix) {
            if let Ok(v) = parse_hex_u8(val) {
                return Some(match variant {
                    "primary" => OverrideTag::PrimaryAlpha { value: v },
                    "secondary" => OverrideTag::SecondaryAlpha { value: v },
                    "outline" => OverrideTag::OutlineAlpha { value: v },
                    "shadow" => OverrideTag::ShadowAlpha { value: v },
                    _ => unreachable!(),
                });
            }
        }
    }
    None
}
