//! Border/shadow/blur tags: `\bord`, `\xbord`, `\ybord`, `\shad`, `\xshad`, `\yshad`, `\be`, `\blur`.
use super::util::strip_parens;
use crate::OverrideTag;

pub fn parse(s: &str) -> Option<OverrideTag> {
    // Border
    if let Some(w) = s
        .strip_prefix("bord")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::Border { x: w, y: w });
    }
    if let Some(w) = s
        .strip_prefix("xbord")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::BorderX(w));
    }
    if let Some(w) = s
        .strip_prefix("ybord")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::BorderY(w));
    }
    // Shadow
    if let Some(d) = s
        .strip_prefix("shad")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::Shadow { x: d, y: d });
    }
    if let Some(d) = s
        .strip_prefix("xshad")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::ShadowX(d));
    }
    if let Some(d) = s
        .strip_prefix("yshad")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::ShadowY(d));
    }
    // Blur
    if let Some(v) = s
        .strip_prefix("be")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::Blur(v));
    }
    if let Some(v) = s
        .strip_prefix("blur")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::GaussianBlur(v));
    }
    None
}
