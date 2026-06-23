//! Transform tag handler: `\t(args,tags)`.
use crate::context::RenderContext;
use ass_core::OverrideTag;

pub fn apply(tag: &OverrideTag, _ctx: &mut RenderContext) {
    if let OverrideTag::Transform { .. } = tag {
        tracing::trace!("transform tag present, animation will interpolate");
    }
}
