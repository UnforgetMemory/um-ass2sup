//! Parse ASS event text into styled text spans.
//!
//! Walks raw ASS text containing `{override blocks}`, tracks the active
//! override tags, and emits `(text, cosmic_text::AttrsOwned)` segments.
//!
//! Mapped attributes:
//! - `family`    ← `\fn` (FontName)
//! - `weight`    ← `\b` / `\bN` (Bold / BoldWeight)
//! - `style`     ← `\i` (Italic)
//! - `font_size` ← `\fs` / `\fs±N` (FontSize / FontSizeRelative)

use ass_parser::{parse_override_tag, OverrideTag, Style};
use cosmic_text::{Attrs, AttrsOwned, Family, Metrics, Weight};

/// Font attributes accumulated while walking override blocks.
#[derive(Debug, Clone, Default)]
struct Active {
    font_name: String,
    font_size: f64,
    bold: bool,
    italic: bool,
    /// Snapshot of style-level defaults for `\r` resets.
    base: Base,
}

#[derive(Debug, Clone, Default)]
struct Base {
    font_name: String,
    font_size: f64,
    bold: bool,
    italic: bool,
}

impl Active {
    fn from_style(style: &Style) -> Self {
        let base = Base {
            font_name: style.font_name.clone(),
            font_size: style.font_size,
            bold: style.bold,
            italic: style.italic,
        };
        Self {
            font_name: base.font_name.clone(),
            font_size: base.font_size,
            bold: base.bold,
            italic: base.italic,
            base,
        }
    }

    fn apply(&mut self, tag: &OverrideTag) {
        match tag {
            OverrideTag::ResetAll | OverrideTag::Reset(_) => {
                self.font_name = self.base.font_name.clone();
                self.font_size = self.base.font_size;
                self.bold = self.base.bold;
                self.italic = self.base.italic;
            }
            OverrideTag::FontName(name) => self.font_name = name.clone(),
            OverrideTag::FontSize(size) => self.font_size = *size,
            OverrideTag::Bold(on) => self.bold = *on,
            OverrideTag::BoldWeight(w) => self.bold = *w > 0,
            OverrideTag::Italic(on) => self.italic = *on,
            _ => {}
        }
    }

    fn to_attrs(&self) -> AttrsOwned {
        let mut attrs = Attrs::new();
        if !self.font_name.is_empty() {
            attrs = attrs.family(Family::Name(&self.font_name));
        }
        attrs = attrs.weight(if self.bold {
            Weight::BOLD
        } else {
            Weight::NORMAL
        });
        attrs = attrs.style(if self.italic {
            cosmic_text::Style::Italic
        } else {
            cosmic_text::Style::Normal
        });
        let metrics = Metrics::new(self.font_size as f32, self.font_size as f32 * 1.2);
        attrs = attrs.metrics(metrics);
        AttrsOwned::new(&attrs)
    }
}

/// Split an override block into individual `\tag` strings.
///
/// Respects parenthesis nesting so `\t(\pos(100,200),0,3000,1)` is kept intact.
fn split_tags(s: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut depth = 0usize;
    for (i, c) in s.char_indices() {
        match c {
            '(' => depth += 1,
            ')' if depth > 0 => depth = depth.saturating_sub(1),
            '\\' if depth == 0 => {
                if i > start {
                    parts.push(&s[start..i]);
                }
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    if start < s.len() {
        parts.push(&s[start..]);
    }
    parts
}

/// Parse raw ASS event text into styled text spans.
///
/// Walks `text` and splits it at override block boundaries. Each plain-text
/// segment is paired with the active attributes at that position.  Attributes
/// are seeded from `style` and updated by tags inside `{...}` blocks.
pub fn parse_spans(text: &str, style: &Style) -> Vec<(String, AttrsOwned)> {
    let mut out = Vec::new();
    let mut active = Active::from_style(style);
    let mut buf = String::new();
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' {
            if !buf.is_empty() {
                out.push((std::mem::take(&mut buf), active.to_attrs()));
            }
            // Collect block content, tracking nested braces.
            let mut depth = 1;
            let mut block = String::new();
            while depth > 0 {
                match chars.next() {
                    Some('{') => depth += 1,
                    Some('}') => depth -= 1,
                    Some(ch) if depth > 0 => block.push(ch),
                    None => break,
                    _ => {}
                }
            }
            // Parse and apply tags.
            for part in split_tags(&block) {
                if let Some(tag) = parse_override_tag(part) {
                    active.apply(&tag);
                }
            }
        } else {
            buf.push(c);
        }
    }

    if !buf.is_empty() {
        out.push((buf, active.to_attrs()));
    }

    out
}
