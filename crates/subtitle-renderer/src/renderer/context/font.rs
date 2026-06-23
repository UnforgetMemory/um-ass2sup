//! Font tag handler: `\fn`, `\fs`, `\fs+N/-N`, `\b`, `\i`, `\u`, `\s`.
use crate::context::RenderContext;
use ass_core::OverrideTag;

pub fn apply(tag: &OverrideTag, ctx: &mut RenderContext) {
    match tag {
        OverrideTag::FontName(name) => ctx.font_name = name.clone(),
        OverrideTag::FontSize(fs) => ctx.font_size = *fs as f32,
        OverrideTag::FontSizeRelative(delta) => {
            ctx.font_size += *delta as f32;
        }
        OverrideTag::Bold(b) => ctx.bold = *b,
        OverrideTag::BoldWeight(w) => ctx.bold = *w >= 700,
        OverrideTag::Italic(i) => ctx.italic = *i,
        OverrideTag::Underline(u) => ctx.underline = *u,
        OverrideTag::Strikeout(s) => ctx.strikeout = *s,
        _ => {}
    }
}
