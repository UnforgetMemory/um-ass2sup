//! Effect tag handler: `\fad`, `\fade` — fade-in/fade-out alpha interpolation.
//!
//! These override tags control the `alpha_multiplier` field of `RenderContext`,
//! which is applied during compositing to create fade-in/fade-out effects.

use crate::context::RenderContext;
use ass_core::OverrideTag;

/// Apply fade tags to the render context.
///
/// Called from `build_context` after all override tags are dispatched.
/// Both `\fad(duration_in, duration_out)` and
/// `\fade(a1,a2,a3,t1,t2,t3,t4)` are handled here.
pub fn apply_fade(
    event: &ass_core::Event,
    ctx: &mut RenderContext,
    timestamp_ms: u64,
    event_start_ms: u64,
) {
    let elapsed = timestamp_ms.saturating_sub(event_start_ms);
    let event_duration = event.end_ms.saturating_sub(event.start_ms);

    for to in &event.override_tags {
        match &to.tag {
            OverrideTag::Fade {
                duration_in,
                duration_out,
            } => {
                ctx.alpha_multiplier =
                    compute_fad_alpha(elapsed, event_duration, *duration_in, *duration_out);
            }
            OverrideTag::FadeComplex {
                alpha_start,
                alpha_mid,
                alpha_end,
                t1,
                t2,
                t3,
                t4,
            } => {
                let total = event_duration;
                let t1 = *t1;
                let t2 = if *t2 == 0 { total } else { *t2 };
                let t3 = *t3;
                let t4 = if *t4 == 0 { total } else { *t4 };
                ctx.alpha_multiplier = compute_fade_complex(
                    elapsed,
                    *alpha_start,
                    *alpha_mid,
                    *alpha_end,
                    t1,
                    t2,
                    t3,
                    t4,
                );
            }
            _ => {}
        }
    }
}

/// Compute alpha multiplier for `\fad(duration_in, duration_out)`.
///
/// Returns a value in `[0.0, 1.0]`:
/// - 0.0 = fully transparent
/// - 1.0 = fully opaque
fn compute_fad_alpha(elapsed: u64, total_duration: u64, fade_in: u64, fade_out: u64) -> f32 {
    if fade_in > 0 && elapsed < fade_in {
        return elapsed as f32 / fade_in as f32;
    }
    if fade_out > 0 && elapsed > total_duration.saturating_sub(fade_out) {
        let remaining = total_duration.saturating_sub(elapsed);
        return remaining as f32 / fade_out as f32;
    }
    1.0
}

/// Compute alpha multiplier for `\fade(a1,a2,a3,t1,t2,t3,t4)`.
#[allow(clippy::too_many_arguments)]
fn compute_fade_complex(
    elapsed: u64,
    alpha_start: u8,
    alpha_mid: u8,
    alpha_end: u8,
    t1: u64,
    t2: u64,
    t3: u64,
    t4: u64,
) -> f32 {
    let a1 = f32::from(255 - alpha_start) / 255.0;
    let a2 = f32::from(255 - alpha_mid) / 255.0;
    let a3 = f32::from(255 - alpha_end) / 255.0;

    #[allow(clippy::if_not_else)]
    if elapsed <= t1 {
        return a1;
    }
    if elapsed <= t2 {
        let t = (elapsed - t1) as f32 / (t2 - t1).max(1) as f32;
        return a1 + (a2 - a1) * t;
    }
    if elapsed <= t3 {
        return a2;
    }
    if elapsed <= t4 {
        let t = (elapsed - t3) as f32 / (t4 - t3).max(1) as f32;
        return a2 + (a3 - a2) * t;
    }
    a3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fad_alpha_fade_in_progress() {
        let a = compute_fad_alpha(500, 5000, 1000, 1000);
        assert!((a - 0.5).abs() < 0.001);
    }

    #[test]
    fn fad_alpha_fade_in_complete() {
        let a = compute_fad_alpha(1000, 5000, 1000, 1000);
        assert!((a - 1.0).abs() < 0.001);
    }

    #[test]
    fn fad_alpha_fade_out_progress() {
        let a = compute_fad_alpha(4750, 5000, 1000, 1000);
        assert!((a - 0.25).abs() < 0.001);
    }

    #[test]
    fn fad_alpha_middle_opaque() {
        let a = compute_fad_alpha(2500, 5000, 1000, 1000);
        assert!((a - 1.0).abs() < 0.001);
    }
}
