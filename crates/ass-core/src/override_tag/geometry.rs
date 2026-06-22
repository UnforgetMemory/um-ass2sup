//! Geometry tags: scale, rotation, shear, spacing.
use super::util::strip_parens;
use crate::OverrideTag;

pub fn parse(s: &str) -> Option<OverrideTag> {
    // Scale X
    if let Some(x) = s
        .strip_prefix("fscx")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::Scale { x, y: 100.0 });
    }
    // Scale Y
    if let Some(y) = s
        .strip_prefix("fscy")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::Scale { x: 100.0, y });
    }
    // Scale reset (libass: \fsc → reset both to style defaults)
    if s == "fsc" || s == "fsc()" {
        return Some(OverrideTag::ScaleReset);
    }
    // Rotation
    if let Some(z) = s
        .strip_prefix("frz")
        .or_else(|| s.strip_prefix("fr"))
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::Rotation { x: 0.0, y: 0.0, z });
    }
    if let Some(x) = s
        .strip_prefix("frx")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::Rotation { x, y: 0.0, z: 0.0 });
    }
    if let Some(y) = s
        .strip_prefix("fry")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::Rotation { x: 0.0, y, z: 0.0 });
    }
    // Shear
    if let Some(x) = s
        .strip_prefix("fax")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::Shear { x, y: 0.0 });
    }
    if let Some(y) = s
        .strip_prefix("fay")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::Shear { x: 0.0, y });
    }
    // Spacing
    if let Some(v) = s
        .strip_prefix("fsp")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::Spacing(v));
    }
    None
}
