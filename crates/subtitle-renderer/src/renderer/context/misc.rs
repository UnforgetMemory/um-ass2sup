//! Misc tag handler: `\an`, `\a`, `\q`, `\fe`, `\!`, `\p`, `\pbo`, `\writing_mode`, unknown.
use crate::context::RenderContext;
use ass_core::OverrideTag;

pub fn apply(tag: &OverrideTag, ctx: &mut RenderContext) {
    match tag {
        OverrideTag::AlignmentNumpad(a) => ctx.alignment = *a,
        OverrideTag::AlignmentVsfilter(a) => ctx.alignment = *a,
        OverrideTag::WrapStyle(v) => ctx.wrap_style = *v,
        OverrideTag::WritingMode(m) => ctx.writing_mode = *m,
        OverrideTag::Charset(v) => ctx.charset = *v,
        OverrideTag::AnimationSkip => ctx.animation_skip = true,
        OverrideTag::BaselineOffset(v) => ctx.baseline_offset = *v,
        OverrideTag::DrawingMode(v) => ctx.drawing_mode = *v,
        OverrideTag::Unknown(s) => tracing::warn!(tag = %s, "unrecognized override tag"),
        _ => {}
    }
}
