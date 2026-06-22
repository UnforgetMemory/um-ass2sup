//! Clip tags: `\clip`, `\iclip` (rect, vector, and drawing variants).
use super::util::{nums_f64, paren_body};
use crate::OverrideTag;

pub fn parse(s: &str) -> Option<OverrideTag> {
    if s.starts_with("clip(") {
        let inner = paren_body(s, "clip(");
        if inner.trim() == "@" {
            return Some(OverrideTag::ClipDrawingCurrent);
        }
        let n = nums_f64(s, "clip(");
        if n.len() >= 4 {
            return Some(OverrideTag::Clip {
                x1: n[0],
                y1: n[1],
                x2: n[2],
                y2: n[3],
            });
        }
        if let Some(comma) = inner.find(',') {
            if let Ok(scale) = inner[..comma].trim().parse::<f32>() {
                return Some(OverrideTag::ClipDrawing {
                    scale,
                    commands: inner[comma + 1..].trim().to_string(),
                });
            }
        }
    }
    None
}

pub fn parse_inverse(s: &str) -> Option<OverrideTag> {
    if s.starts_with("iclip(") {
        let inner = paren_body(s, "iclip(");
        if inner.trim() == "@" {
            return Some(OverrideTag::ClipInverseDrawingCurrent);
        }
        let n = nums_f64(s, "iclip(");
        if n.len() >= 4 {
            return Some(OverrideTag::ClipInverse {
                x1: n[0],
                y1: n[1],
                x2: n[2],
                y2: n[3],
            });
        }
        if let Some(comma) = inner.find(',') {
            if let Ok(scale) = inner[..comma].trim().parse::<f32>() {
                return Some(OverrideTag::ClipInverseDrawing {
                    scale,
                    commands: inner[comma + 1..].trim().to_string(),
                });
            }
        }
    }
    None
}
