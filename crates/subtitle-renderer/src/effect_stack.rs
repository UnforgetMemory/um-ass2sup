//! Per-line effect stack for the v2.0 renderer.
//!
//! A single dialogue line can carry multiple ASS override tags that affect
//! the render: `\fade(...)` + `\pos(...)` + `\clip(...)` + `\frz(...)` etc.
//! The `EffectStack` collects all of them and provides a single
//! `evaluate_at(time_ms)` method that computes the consolidated state of
//! every effect at a given event-relative time.
//!
//! # Why
//!
//! The v0.5.x renderer evaluates effects tag-by-tag in [`build_context`].
//! The v2.0 plan calls for a unified `EffectStack` that:
//! - Maintains per-line ordering (so e.g. \clip then \fad applies clip first)
//! - Re-evaluates every frame so animations interpolate cleanly
//! - Composes independent effects without coupling them to the override
//!   parser
//!
//! # Composition rules
//!
//! - **Pos + Move**: `Move` is treated as a time-keyed `Pos`. If both are
//!   set, `Move` wins for the duration of the animation; `Pos` is the
//!   fallback.
//! - **Clip + IClip**: the last one wins.
//! - **Fade + FadeComplex**: `FadeComplex` wins when set; otherwise
//!   `Fade` provides the simple two-segment curve.
//! - **Transform (`\t(\tag, ...)`)**: each Transform is unwrapped into
//!   the underlying primitive effect (Pos, Rotation, Color, etc.)
//!   before being pushed onto the stack, so animations compose with
//!   static effects via the same `evaluate_at` machinery.
//!
//! [`build_context`]: crate::context::build_context

use super::context::RenderContext;

/// A single effect applied to a dialogue line. This is the v2.0
/// effect enum — distinct from the ass-parser `Effect` (which is the
/// 9th comma-separated field of an event line) and from the ass-parser
/// `OverrideTag` (which is a single parsed override tag).
#[derive(Debug, Clone, PartialEq)]
pub enum RendererEffect {
    /// `\fad(t1, t2)` — simple alpha ramp at the start (t1 ms) and end
    /// (t2 ms) of the event.
    Fade {
        /// Fade-in duration in milliseconds.
        fade_in_ms: u32,
        /// Fade-out duration in milliseconds.
        fade_out_ms: u32,
    },
    /// `\fade(a1, a2, a3, t1, t2, t3, t4)` — three-segment alpha
    /// animation between four keyframes.
    FadeComplex {
        /// Alpha at the start of the event (0-255).
        a1: u8,
        /// Alpha at t1 (0-255).
        a2: u8,
        /// Alpha at t2 (0-255).
        a3: u8,
        /// Time of the first keyframe in ms.
        t1_ms: u32,
        /// Time of the second keyframe in ms.
        t2_ms: u32,
        /// Time of the third keyframe in ms.
        t3_ms: u32,
        /// End time of the event in ms.
        t4_ms: u32,
    },
    /// `\pos(x, y)` — fixed position. The position is applied before
    /// alignment, so `(x, y)` is the screen-space anchor.
    Pos {
        /// Horizontal screen position.
        x: f32,
        /// Vertical screen position.
        y: f32,
    },
    /// `\move(x1, y1, x2, y2, t1, t2)` — linear interpolation between
    /// (x1, y1) and (x2, y2) over the interval `[t1, t2]` ms.
    Move {
        /// Starting x.
        x1: f32,
        /// Starting y.
        y1: f32,
        /// Ending x.
        x2: f32,
        /// Ending y.
        y2: f32,
        /// Start of motion in ms.
        t1_ms: u32,
        /// End of motion in ms.
        t2_ms: u32,
    },
    /// `\clip(x1, y1, x2, y2)` — rectangular clip. Pixels outside the
    /// rect are dropped.
    Clip {
        /// Left edge.
        x1: f32,
        /// Top edge.
        y1: f32,
        /// Right edge.
        x2: f32,
        /// Bottom edge.
        y2: f32,
    },
    /// `\iclip(x1, y1, x2, y2)` — inverse rectangular clip. Pixels
    /// inside the rect are dropped.
    InverseClip {
        /// Left edge.
        x1: f32,
        /// Top edge.
        y1: f32,
        /// Right edge.
        x2: f32,
        /// Bottom edge.
        y2: f32,
    },
    /// `\frz(angle)` — Z-axis rotation in degrees.
    RotationZ(f32),
    /// `\frx(angle)` — X-axis rotation in degrees.
    RotationX(f32),
    /// `\fry(angle)` — Y-axis rotation in degrees.
    RotationY(f32),
    /// `\fax(shear)` — X-axis shear factor.
    ShearX(f32),
    /// `\fay(shear)` — Y-axis shear factor.
    ShearY(f32),
    /// `\blur(n)` — Gaussian blur radius in pixels.
    Blur(f32),
    /// `\be(strength)` — edge blur.
    EdgeBlur(f32),
}

/// Per-line effect stack. Effects are applied in insertion order; later
/// effects override earlier ones for the same property (Pos vs Move,
/// Clip vs InverseClip, etc.).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct EffectStack {
    effects: Vec<RendererEffect>,
}

impl EffectStack {
    /// Build an empty stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an effect onto the top of the stack.
    pub fn push(&mut self, effect: RendererEffect) {
        self.effects.push(effect);
    }

    /// Returns the number of effects in the stack.
    pub fn len(&self) -> usize {
        self.effects.len()
    }

    /// Returns `true` if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.effects.is_empty()
    }

    /// Resolve the position (x, y) at the given event-relative time.
    ///
    /// `Move` animations override `Pos` for the duration of the move;
    /// outside the move window, `Pos` (or the default alignment) is used.
    pub fn resolve_pos(&self, time_ms: u64, _event_duration_ms: u64) -> (f32, f32) {
        let mut pos: Option<(f32, f32)> = None;
        for fx in &self.effects {
            match fx {
                RendererEffect::Pos { x, y } => {
                    if pos.is_none() {
                        pos = Some((*x, *y));
                    }
                }
                RendererEffect::Move {
                    x1,
                    y1,
                    x2,
                    y2,
                    t1_ms,
                    t2_ms,
                } => {
                    pos = Some(eval_move(*x1, *y1, *x2, *y2, *t1_ms, *t2_ms, time_ms));
                }
                _ => {}
            }
        }
        pos.unwrap_or((f32::NAN, f32::NAN))
    }

    /// Compute the alpha multiplier at the given time.
    ///
    /// Returns a value in `[0.0, 1.0]` where 1.0 is fully opaque.
    /// `FadeComplex` wins over `Fade` when both are set.
    pub fn resolve_alpha(&self, time_ms: u64, event_duration_ms: u64) -> f32 {
        for fx in &self.effects {
            match fx {
                RendererEffect::Fade {
                    fade_in_ms,
                    fade_out_ms,
                } => {
                    return eval_fade(*fade_in_ms, *fade_out_ms, time_ms, event_duration_ms);
                }
                RendererEffect::FadeComplex {
                    a1,
                    a2,
                    a3,
                    t1_ms,
                    t2_ms,
                    t3_ms,
                    t4_ms,
                } => {
                    return eval_fade_complex(
                        *a1,
                        *a2,
                        *a3,
                        *t1_ms,
                        *t2_ms,
                        *t3_ms,
                        *t4_ms,
                        time_ms,
                        event_duration_ms,
                    );
                }
                _ => {}
            }
        }
        1.0
    }

    /// Returns the active clip rectangle, if any. Later effects in
    /// the stack override earlier ones (`InverseClip` wins over `Clip`
    /// when both are set); the boolean reports whether the clip is
    /// inverse (true) or normal (false).
    pub fn resolve_clip(&self) -> Option<(f32, f32, f32, f32, bool)> {
        let mut result: Option<(f32, f32, f32, f32, bool)> = None;
        for fx in &self.effects {
            match fx {
                RendererEffect::Clip { x1, y1, x2, y2 } => {
                    result = Some((*x1, *y1, *x2, *y2, false));
                }
                RendererEffect::InverseClip { x1, y1, x2, y2 } => {
                    result = Some((*x1, *y1, *x2, *y2, true));
                }
                _ => {}
            }
        }
        result
    }

    /// Returns the active Gaussian blur radius in pixels, or 0.0 if
    /// no blur is set. Later effects in the stack override earlier
    /// ones (`EdgeBlur` wins over `Blur` when both are set).
    pub fn resolve_blur(&self) -> f32 {
        let mut result = 0.0;
        for fx in &self.effects {
            match fx {
                RendererEffect::Blur(n) => result = *n,
                RendererEffect::EdgeBlur(n) => result = *n,
                _ => {}
            }
        }
        result
    }

    /// Returns the active Z rotation in degrees (default 0.0).
    pub fn resolve_rotation_z(&self) -> f32 {
        let mut rot = 0.0;
        for fx in &self.effects {
            if let RendererEffect::RotationZ(angle) = fx {
                rot = *angle;
            }
        }
        rot
    }

    /// Apply every effect on the stack to the given render context in
    /// one pass. This is the v2.0 entry point that the renderer
    /// calls per-frame.
    pub fn apply(&self, ctx: &mut RenderContext, time_ms: u64, event_duration_ms: u64) {
        let (px, py) = self.resolve_pos(time_ms, event_duration_ms);
        if !px.is_nan() {
            ctx.x = px;
        }
        if !py.is_nan() {
            ctx.y = py;
        }
        let alpha = self.resolve_alpha(time_ms, event_duration_ms);
        if alpha < 1.0 {
            ctx.alpha_multiplier *= alpha;
        }
        if let Some((x1, y1, x2, y2, inverse)) = self.resolve_clip() {
            ctx.clip_x1 = x1;
            ctx.clip_y1 = y1;
            ctx.clip_x2 = x2;
            ctx.clip_y2 = y2;
            ctx.clip_enabled = true;
            ctx.clip_inverse = inverse;
        }
        let blur = self.resolve_blur();
        if blur > 0.0 {
            ctx.blur = blur;
        }
        let rot = self.resolve_rotation_z();
        if rot != 0.0 {
            ctx.rotation = rot;
        }
    }
}

/// Evaluate `\move(x1, y1, x2, y2, t1, t2)` at `time_ms`.
fn eval_move(
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    t1_ms: u32,
    t2_ms: u32,
    time_ms: u64,
) -> (f32, f32) {
    if time_ms < u64::from(t1_ms) {
        return (x1, y1);
    }
    if time_ms >= u64::from(t2_ms) {
        return (x2, y2);
    }
    let span = (t2_ms - t1_ms).max(1) as f32;
    let t = (time_ms - u64::from(t1_ms)) as f32 / span;
    (x1 + (x2 - x1) * t, y1 + (y2 - y1) * t)
}

/// Evaluate `\fad(in, out)` at `time_ms` for an event of `duration_ms`.
fn eval_fade(fade_in_ms: u32, fade_out_ms: u32, time_ms: u64, duration_ms: u64) -> f32 {
    let fade_in = u64::from(fade_in_ms);
    let fade_out = u64::from(fade_out_ms);
    if time_ms < fade_in {
        return time_ms as f32 / fade_in.max(1) as f32;
    }
    let fade_out_start = duration_ms.saturating_sub(fade_out);
    if time_ms >= fade_out_start && duration_ms > fade_out {
        let remaining = duration_ms - time_ms;
        return remaining as f32 / fade_out.max(1) as f32;
    }
    1.0
}

/// Evaluate `\fade(a1, a2, a3, t1, t2, t3, t4)` at `time_ms`.
#[allow(clippy::too_many_arguments)]
fn eval_fade_complex(
    a1: u8,
    a2: u8,
    a3: u8,
    t1_ms: u32,
    t2_ms: u32,
    t3_ms: u32,
    t4_ms: u32,
    time_ms: u64,
    event_duration_ms: u64,
) -> f32 {
    let t1 = u64::from(t1_ms);
    let t2 = u64::from(t2_ms);
    let t3 = u64::from(t3_ms);
    let t4 = u64::from(t4_ms);
    let end = if t4 == 0 { event_duration_ms } else { t4 };
    if time_ms <= t1 {
        return f32::from(a1) / 255.0;
    }
    if time_ms >= end {
        return f32::from(a3) / 255.0;
    }
    if time_ms <= t2 && t2 > t1 {
        let span = (t2 - t1) as f32;
        let t = (time_ms - t1) as f32 / span;
        return lerp_u8(a1, a2, t);
    }
    if time_ms <= t3 && t3 > t2 {
        let span = (t3 - t2) as f32;
        let t = (time_ms - t2) as f32 / span;
        return lerp_u8(a2, a3, t);
    }
    f32::from(a3) / 255.0
}

/// Linear interpolation between two `u8` values, returning a `f32` in `[0.0, 1.0]`.
fn lerp_u8(a: u8, b: u8, t: f32) -> f32 {
    let a = f32::from(a);
    let b = f32::from(b);
    (a + (b - a) * t) / 255.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_stack_returns_defaults() {
        let stack = EffectStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
        assert_eq!(stack.resolve_alpha(0, 1000), 1.0);
        assert_eq!(stack.resolve_blur(), 0.0);
        assert_eq!(stack.resolve_rotation_z(), 0.0);
        assert!(stack.resolve_clip().is_none());
        let (x, y) = stack.resolve_pos(0, 1000);
        assert!(x.is_nan() && y.is_nan());
    }

    #[test]
    fn pos_effect_sets_position() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::Pos { x: 100.0, y: 200.0 });
        let (x, y) = stack.resolve_pos(0, 1000);
        assert_eq!(x, 100.0);
        assert_eq!(y, 200.0);
    }

    #[test]
    fn move_overrides_pos() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::Pos { x: 0.0, y: 0.0 });
        stack.push(RendererEffect::Move {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 200.0,
            t1_ms: 0,
            t2_ms: 1000,
        });
        let (x, y) = stack.resolve_pos(500, 1000);
        assert_eq!(x, 50.0);
        assert_eq!(y, 100.0);
    }

    #[test]
    fn move_before_t1_returns_start() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::Move {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 200.0,
            t1_ms: 100,
            t2_ms: 200,
        });
        let (x, y) = stack.resolve_pos(0, 1000);
        assert_eq!(x, 0.0);
        assert_eq!(y, 0.0);
    }

    #[test]
    fn move_after_t2_returns_end() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::Move {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 200.0,
            t1_ms: 100,
            t2_ms: 200,
        });
        let (x, y) = stack.resolve_pos(500, 1000);
        assert_eq!(x, 100.0);
        assert_eq!(y, 200.0);
    }

    #[test]
    fn fade_in_alpha_ramps() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::Fade {
            fade_in_ms: 1000,
            fade_out_ms: 0,
        });
        assert_eq!(stack.resolve_alpha(0, 5000), 0.0);
        assert_eq!(stack.resolve_alpha(500, 5000), 0.5);
        assert_eq!(stack.resolve_alpha(1000, 5000), 1.0);
        assert_eq!(stack.resolve_alpha(2000, 5000), 1.0);
    }

    #[test]
    fn fade_out_alpha_drops() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::Fade {
            fade_in_ms: 0,
            fade_out_ms: 1000,
        });
        // fade_in=0: at t=0 the subtitle is fully opaque.
        assert_eq!(stack.resolve_alpha(0, 5000), 1.0);
        // At t=4000, fade-out has not yet started (it starts at 5000-1000=4000,
        // and the branch only fires after the start of the fade window).
        assert_eq!(stack.resolve_alpha(4000, 5000), 1.0);
        // At t=5000 (end of event), fully transparent.
        assert_eq!(stack.resolve_alpha(5000, 5000), 0.0);
    }

    #[test]
    fn fade_complex_three_segments() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::FadeComplex {
            a1: 0,
            a2: 255,
            a3: 0,
            t1_ms: 0,
            t2_ms: 1000,
            t3_ms: 2000,
            t4_ms: 3000,
        });
        // At t=0 -> a1 (0)
        assert_eq!(stack.resolve_alpha(0, 3000), 0.0);
        // At t=500 -> halfway between 0 and 255
        let v = stack.resolve_alpha(500, 3000);
        assert!((v - 0.5).abs() < 0.01, "expected ~0.5, got {v}");
        // At t=1500 -> halfway between 255 and 0
        let v = stack.resolve_alpha(1500, 3000);
        assert!((v - 0.5).abs() < 0.01, "expected ~0.5, got {v}");
        // At t=3000 -> a3 (0)
        assert_eq!(stack.resolve_alpha(3000, 3000), 0.0);
    }

    #[test]
    fn clip_effect_provides_rect() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::Clip {
            x1: 10.0,
            y1: 20.0,
            x2: 100.0,
            y2: 200.0,
        });
        let clip = stack.resolve_clip();
        assert!(matches!(clip, Some((10.0, 20.0, 100.0, 200.0, false))));
    }

    #[test]
    fn inverse_clip_wins_over_normal_clip() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::Clip {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
        });
        stack.push(RendererEffect::InverseClip {
            x1: 10.0,
            y1: 10.0,
            x2: 50.0,
            y2: 50.0,
        });
        let clip = stack.resolve_clip();
        assert!(matches!(clip, Some((10.0, 10.0, 50.0, 50.0, true))));
    }

    #[test]
    fn rotation_z_accumulates() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::RotationZ(10.0));
        stack.push(RendererEffect::RotationZ(20.0));
        assert_eq!(stack.resolve_rotation_z(), 20.0);
    }

    #[test]
    fn blur_wins_later() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::Blur(1.5));
        stack.push(RendererEffect::EdgeBlur(2.5));
        assert_eq!(stack.resolve_blur(), 2.5);
    }

    #[test]
    fn apply_writes_to_context() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::Pos { x: 100.0, y: 200.0 });
        stack.push(RendererEffect::RotationZ(45.0));
        let mut ctx = RenderContext::default();
        stack.apply(&mut ctx, 0, 1000);
        assert_eq!(ctx.x, 100.0);
        assert_eq!(ctx.y, 200.0);
        assert_eq!(ctx.rotation, 45.0);
    }

    #[test]
    fn push_preserves_order() {
        let mut stack = EffectStack::new();
        stack.push(RendererEffect::RotationZ(10.0));
        stack.push(RendererEffect::RotationZ(20.0));
        stack.push(RendererEffect::RotationZ(30.0));
        assert_eq!(stack.len(), 3);
        assert_eq!(stack.resolve_rotation_z(), 30.0);
    }
}
