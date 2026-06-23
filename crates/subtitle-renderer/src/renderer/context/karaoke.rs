//! Karaoke tag handler: `\k`, `\kf`, `\K`, `\ko`, `\kt`.
use crate::context::RenderContext;
use ass_core::OverrideTag;

pub fn apply(tag: &OverrideTag, _ctx: &mut RenderContext) {
    if matches!(tag, OverrideTag::Karaoke { .. }) {
        tracing::trace!("karaoke tag present");
    }
}
