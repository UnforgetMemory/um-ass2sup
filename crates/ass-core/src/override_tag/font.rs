//! Font tags: `\fn`, `\fs`, `\b`, `\i`, `\u`, `\s`, `\fe`, `\q`, `\p`, `\pbo`, `\r`, `\writing_mode`.
use super::util::strip_parens;
use crate::OverrideTag;

pub fn parse(s: &str) -> Option<OverrideTag> {
    // Font name
    if let Some(name) = s.strip_prefix("fn") {
        let name = name.trim();
        if name == "0" || name == "(0)" {
            return Some(OverrideTag::FontName(String::new())); // \fn0 → style default
        }
        return Some(OverrideTag::FontName(name.to_string()));
    }
    // Font size
    if let Some(size) = s.strip_prefix("fs") {
        if let Ok(v) = size.parse::<f64>() {
            return Some(OverrideTag::FontSize(v));
        }
        if let Some(rel) = size.strip_prefix('+').or_else(|| size.strip_prefix('-')) {
            if let Ok(v) = rel.parse::<isize>() {
                let multiplier = if size.starts_with('+') { 1 } else { -1 };
                return Some(OverrideTag::FontSizeRelative(v * multiplier));
            }
        }
    }
    // Bold
    if s == "b1" || s == "b0" || s == "b-1" {
        return Some(OverrideTag::Bold(s == "b1" || s == "b-1"));
    }
    if let Some(w) = s.strip_prefix("b") {
        if let Ok(v) = w.parse::<u32>() {
            return Some(OverrideTag::BoldWeight(v));
        }
    }
    // Italic
    if s == "i1" || s == "i0" || s == "i-1" {
        return Some(OverrideTag::Italic(s == "i1"));
    }
    // Underline
    if s == "u1" || s == "u0" || s == "u-1" {
        return Some(OverrideTag::Underline(s == "u1"));
    }
    // Strikeout
    if s == "s1" || s == "s0" || s == "s-1" {
        return Some(OverrideTag::Strikeout(s == "s1"));
    }
    // Reset
    if s.starts_with('r') && !s.starts_with("reset") {
        if s == "r" {
            return Some(OverrideTag::ResetAll);
        }
        return Some(OverrideTag::Reset(s[1..].to_string()));
    }
    // Alignment (numpad)
    if let Some(an) = s.strip_prefix("an") {
        if let Ok(v) = an.parse::<u8>() {
            if (1..=9).contains(&v) {
                return Some(OverrideTag::AlignmentNumpad(v));
            }
        }
    }
    // Alignment (legacy, with VSFilter quirks)
    if s.starts_with('a') && !s.starts_with("an") {
        if let Ok(v) = s[1..].parse::<u8>() {
            let mapped = if v == 4 || v == 8 { 5 } else { v };
            return Some(OverrideTag::AlignmentVsfilter(mapped));
        }
    }
    // Wrap style
    if let Some(v) = s
        .strip_prefix("q")
        .and_then(|r| strip_parens(r).parse::<u8>().ok())
    {
        if v <= 3 {
            return Some(OverrideTag::WrapStyle(v));
        }
    }
    // Drawing mode
    if s.starts_with('p') && !s.starts_with("pos") && !s.starts_with("pbo") {
        if let Ok(v) = s[1..].parse::<i32>() {
            return Some(OverrideTag::DrawingMode(if v < 0 { 0 } else { v as u8 }));
        }
    }
    // Baseline offset
    if let Some(v) = s
        .strip_prefix("pbo")
        .and_then(|r| strip_parens(r).parse::<f64>().ok())
    {
        return Some(OverrideTag::BaselineOffset(v));
    }
    // Charset
    if let Some(v) = s
        .strip_prefix("fe")
        .and_then(|r| strip_parens(r).parse::<u8>().ok())
    {
        return Some(OverrideTag::Charset(v));
    }
    // Writing mode
    if s.starts_with("writing_mode(") {
        if let Ok(v) = super::util::paren_body(s, "writing_mode(").parse::<u8>() {
            return Some(OverrideTag::WritingMode(v));
        }
    }
    None
}
