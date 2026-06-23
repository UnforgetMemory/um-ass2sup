//! Post-processing effects — re-exported from the cosmic::effects sub-modules.
//!
//! Provides blur, shadow, and alpha compositing operations used by the
//! subtitle renderer to implement ASS override tags like `\be`, `\bord`,
//! `\shad`, and `\fad`.

pub use crate::cosmic::effects::blur::apply_gaussian_blur;
pub use crate::cosmic::effects::composite::composite_over;
pub use crate::cosmic::effects::shadow::apply_shadow;
