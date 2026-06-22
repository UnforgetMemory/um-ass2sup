//! Shared utility functions for override tag parsing.

use crate::AssColor;

/// Split `\`-delimited tags respecting nested parentheses.
pub fn split_tags(s: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    for ch in s.chars() {
        match ch {
            '(' => {
                cur.push(ch);
                depth += 1;
            }
            ')' if depth > 0 => {
                cur.push(ch);
                depth -= 1;
            }
            '\\' if depth == 0 => {
                if !cur.is_empty() {
                    parts.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        parts.push(cur);
    }
    parts
}

/// Split by commas NOT inside parentheses.
pub fn split_args(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&s[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&s[start..]);
    parts
}

/// Parse hex u8 from ASS color alpha string (e.g. `&H80&`).
pub fn parse_hex_u8(s: &str) -> Result<u8, std::num::ParseIntError> {
    let s = s
        .trim()
        .trim_start_matches('&')
        .trim_start_matches(['H', 'h'])
        .trim_end_matches('&');
    u8::from_str_radix(s, 16)
}

/// Parse ASS color from inline tag string (e.g. `&HFF0000&`).
pub fn parse_ass_color(s: &str) -> Result<AssColor, ()> {
    let s = s
        .trim()
        .trim_start_matches('&')
        .trim_start_matches(['H', 'h'])
        .trim_end_matches('&');
    if !s.is_ascii() || s.len() < 6 {
        return Err(());
    }
    let hex = if s.len() >= 8 { &s[s.len() - 8..] } else { s };
    let p = |r: &str| u8::from_str_radix(r, 16).map_err(|_| ());
    if hex.len() == 8 {
        Ok(AssColor::new(
            p(&hex[0..2])?,
            p(&hex[2..4])?,
            p(&hex[4..6])?,
            p(&hex[6..8])?,
        ))
    } else {
        Ok(AssColor::new(
            0,
            p(&hex[0..2])?,
            p(&hex[2..4])?,
            p(&hex[4..6])?,
        ))
    }
}

/// Strip optional parentheses from a tag argument value.
pub fn strip_parens(s: &str) -> &str {
    let s = s.trim();
    if s.starts_with('(') && s.ends_with(')') {
        &s[1..s.len().max(2) - 1]
    } else {
        s
    }
}

/// Extract parenthesized body, tolerating missing closing `)`.
pub fn paren_body<'a>(s: &'a str, prefix: &str) -> &'a str {
    let inner = s.strip_prefix(prefix).unwrap_or(s);
    inner.trim_end_matches(')').trim()
}

/// Parse numeric f64 args from a parenthesized tag.
pub fn nums_f64(s: &str, prefix: &str) -> Vec<f64> {
    paren_body(s, prefix)
        .split(',')
        .filter_map(|n| n.trim().parse().ok())
        .collect()
}

/// Parse numeric u64 args from a parenthesized tag.
pub fn nums_u64(s: &str, prefix: &str) -> Vec<u64> {
    paren_body(s, prefix)
        .split(',')
        .filter_map(|n| n.trim().parse().ok())
        .collect()
}
