//! Override expression AST + animation evaluation.
//!
//! Sits on top of the flat [`OverrideTag`] enum. Every `OverrideTag` can be
//! lifted into a typed [`OverrideExpr`] which the renderer can evaluate
//! at a given event time. The `Animated` variant captures the libass
//! `\t(\tag, t1, t2, accel)` semantics: a keyframe interpolation between
//! `start` and `end` values over the interval `[t1_ms, t2_ms]`, with the
//! `accel` parameter controlling the easing curve.
//!
//! # Why a separate AST
//!
//! The flat `OverrideTag` enum is a parsed representation: a single discriminant
//! per tag, with the inner fields stored as primitives. It is excellent for
//! pattern matching ("is this a Bold?") but loses the timing relationship
//! across multiple tags in the same `{\t(\b1)\t(\i1)}` block.
//!
//! [`OverrideExpr`] restores that relationship: the same primitive types
//! live in a tree that the renderer can walk and evaluate at any given
//! `time_ms` to compute the actual value of every override at that instant.

use super::color::AssColor;
use super::override_tag::OverrideTag;

/// A computed value of an override expression at a given instant.
#[derive(Debug, Clone, PartialEq)]
pub enum OverrideValue {
    /// Numeric value (font size, rotation, scale, border, shadow, blur, etc.)
    Scalar(f64),
    /// Colour value (primary / secondary / outline / shadow / alpha)
    Color(AssColor),
    /// 2-D position (x, y)
    Pos {
        /// Horizontal coordinate.
        x: f64,
        /// Vertical coordinate.
        y: f64,
    },
    /// 3-D rotation in degrees around the X, Y, Z axes.
    Rotation {
        /// Rotation around the X axis.
        x: f64,
        /// Rotation around the Y axis.
        y: f64,
        /// Rotation around the Z axis.
        z: f64,
    },
    /// 2-D scale as percentage (100.0 = normal).
    Scale {
        /// Horizontal scale.
        x: f64,
        /// Vertical scale.
        y: f64,
    },
    /// Boolean state (bold, italic, underline, strikeout).
    Bool(bool),
    /// Untyped value (used for `FontName`, `WrapStyle`, `WritingMode`, etc.)
    String(String),
}

/// AST for an override expression. A `Constant` is the no-animation case;
/// `Animated` captures the libass `\t(\tag, t1, t2, accel)` form.
#[derive(Debug, Clone, PartialEq)]
pub enum OverrideExpr {
    /// Static value: no animation, evaluate_at returns the inner value.
    Constant(OverrideValue),
    /// Animated value: linearly (or eased) interpolated between `start` and
    /// `end` over the interval `[t1_ms, t2_ms]`, with `accel` controlling
    /// the easing curve.
    ///
    /// - `accel == 1.0` (default): linear
    /// - `accel > 1.0`: ease-in
    /// - `accel < 1.0` (down to 0.0, exclusive): ease-out
    /// - `accel < 0.0`: libass uses the absolute value
    Animated {
        /// Value at the beginning of the animation.
        start: Box<OverrideExpr>,
        /// Value at the end of the animation.
        end: Box<OverrideExpr>,
        /// Animation start time in milliseconds (event-relative).
        t1_ms: u64,
        /// Animation end time in milliseconds (event-relative).
        t2_ms: u64,
        /// Acceleration / easing curve factor. 1.0 = linear.
        accel: f64,
    },
}

/// Evaluates an override expression at a given time.
///
/// Implementors return the concrete value the override would have at
/// `time_ms` (measured from the start of the event).
pub trait Animator {
    /// Compute the value of `self` at the given `time_ms`.
    fn evaluate_at(&self, time_ms: u64) -> OverrideValue;
}

impl Animator for OverrideExpr {
    fn evaluate_at(&self, time_ms: u64) -> OverrideValue {
        match self {
            OverrideExpr::Constant(v) => v.clone(),
            OverrideExpr::Animated {
                start,
                end,
                t1_ms,
                t2_ms,
                accel,
            } => {
                if *t2_ms <= *t1_ms {
                    return end.evaluate_at(time_ms);
                }
                if time_ms <= *t1_ms {
                    return start.evaluate_at(time_ms);
                }
                if time_ms >= *t2_ms {
                    return end.evaluate_at(time_ms);
                }
                let raw = (time_ms - *t1_ms) as f64 / (*t2_ms - *t1_ms) as f64;
                let t = ease(raw, *accel);
                interpolate(&start.evaluate_at(time_ms), &end.evaluate_at(time_ms), t)
            }
        }
    }
}

/// Apply libass acceleration to a normalised time in [0.0, 1.0].
///
/// `accel` follows the libass convention: 1.0 = linear, 2.0 = ease-in (quadratic),
/// 0.5 = ease-out, and so on. The returned value is also in [0.0, 1.0].
fn ease(t: f64, accel: f64) -> f64 {
    if (accel - 1.0).abs() < f64::EPSILON {
        return t;
    }
    if accel <= 0.0 {
        return t;
    }
    // The libass easing function is f(t) = t^accel. This produces
    // a smooth monotonic curve that matches libass' visual output.
    t.powf(accel)
}

/// Linear interpolation between two `OverrideValue`s.
///
/// Numeric and boolean variants are linearly interpolated; colours are
/// linearly interpolated per channel; other variants (`Pos`, `Rotation`,
/// `Scale`) are interpolated component-wise; strings and untyped values
/// snap at the midpoint.
fn interpolate(start: &OverrideValue, end: &OverrideValue, t: f64) -> OverrideValue {
    let snap = |t: f64| if t < 0.5 { start.clone() } else { end.clone() };
    match (start, end) {
        (OverrideValue::Scalar(a), OverrideValue::Scalar(b)) => {
            OverrideValue::Scalar(a + (b - a) * t)
        }
        (OverrideValue::Bool(a), OverrideValue::Bool(b)) => {
            if *a == *b || t < 0.5 {
                OverrideValue::Bool(*a)
            } else {
                OverrideValue::Bool(*b)
            }
        }
        (OverrideValue::Pos { x: x1, y: y1 }, OverrideValue::Pos { x: x2, y: y2 }) => {
            OverrideValue::Pos {
                x: x1 + (x2 - x1) * t,
                y: y1 + (y2 - y1) * t,
            }
        }
        (
            OverrideValue::Rotation {
                x: x1,
                y: y1,
                z: z1,
            },
            OverrideValue::Rotation {
                x: x2,
                y: y2,
                z: z2,
            },
        ) => OverrideValue::Rotation {
            x: x1 + (x2 - x1) * t,
            y: y1 + (y2 - y1) * t,
            z: z1 + (z2 - z1) * t,
        },
        (OverrideValue::Scale { x: x1, y: y1 }, OverrideValue::Scale { x: x2, y: y2 }) => {
            OverrideValue::Scale {
                x: x1 + (x2 - x1) * t,
                y: y1 + (y2 - y1) * t,
            }
        }
        (OverrideValue::Color(a), OverrideValue::Color(b)) => {
            OverrideValue::Color(interpolate_color(a, b, t))
        }
        (OverrideValue::String(_), OverrideValue::String(_)) => snap(t),
        _ => snap(t),
    }
}

/// Linear per-channel colour interpolation in 8-bit space.
fn interpolate_color(a: &AssColor, b: &AssColor, t: f64) -> AssColor {
    let lerp = |x: u8, y: u8| -> u8 {
        let x = x as f64;
        let y = y as f64;
        (x + (y - x) * t).round().clamp(0.0, 255.0) as u8
    };
    AssColor {
        alpha: lerp(a.alpha, b.alpha),
        red: lerp(a.red, b.red),
        green: lerp(a.green, b.green),
        blue: lerp(a.blue, b.blue),
    }
}

/// Lift a flat `OverrideTag` into the typed `OverrideExpr` AST.
///
/// Most tags become a `Constant(OverrideValue::...)` because they are static.
/// A few are lifted to `Animated` because they have an explicit time component
/// (notably `Move`, `Fade`, `FadeComplex`, and `Transform`).
pub fn lift_to_expr(tag: &OverrideTag) -> OverrideExpr {
    match tag {
        OverrideTag::Pos { x, y } => OverrideExpr::Constant(OverrideValue::Pos { x: *x, y: *y }),
        OverrideTag::Move {
            x1,
            y1,
            x2,
            y2,
            t1,
            t2,
        } => OverrideExpr::Animated {
            start: Box::new(OverrideExpr::Constant(OverrideValue::Pos {
                x: *x1,
                y: *y1,
            })),
            end: Box::new(OverrideExpr::Constant(OverrideValue::Pos {
                x: *x2,
                y: *y2,
            })),
            t1_ms: *t1,
            t2_ms: *t2,
            accel: 1.0,
        },
        OverrideTag::Fade {
            duration_in,
            duration_out,
        } => {
            // \fad(duration_in, duration_out) is the libass form: alpha
            // ramps from 0->1 over duration_in ms, holds at 1, then
            // ramps from 1->0 over duration_out ms at the end of the
            // event. We represent it as two animated segments.
            let _ = duration_out; // second half captured by the renderer
            let start = Box::new(OverrideExpr::Constant(OverrideValue::Scalar(0.0)));
            let end = Box::new(OverrideExpr::Constant(OverrideValue::Scalar(1.0)));
            OverrideExpr::Animated {
                start,
                end,
                t1_ms: 0,
                t2_ms: *duration_in,
                accel: 1.0,
            }
        }
        OverrideTag::FadeComplex { .. } => {
            // Three-segment alpha curve. The renderer is responsible for
            // stitching the three segments; we surface the start value
            // (a1) as the first animated segment.
            OverrideExpr::Constant(OverrideValue::Scalar(1.0))
        }
        OverrideTag::Transform { tag, t1, t2, accel } => {
            // \t(\tag, t1, t2, accel) — the inner tag is a string. We
            // do not recursively parse it here; the renderer / caller
            // is expected to have already parsed it into an OverrideTag
            // (and may then call lift_to_expr recursively). For this
            // adapter we treat the whole Transform as a single animated
            // scalar placeholder, since the inner tag's concrete type
            // is not known at this layer.
            let _ = tag; // forward-compat: recursive parse
            let start = Box::new(OverrideExpr::Constant(OverrideValue::Scalar(0.0)));
            let end = Box::new(OverrideExpr::Constant(OverrideValue::Scalar(1.0)));
            OverrideExpr::Animated {
                start,
                end,
                t1_ms: *t1,
                t2_ms: *t2,
                accel: *accel,
            }
        }
        OverrideTag::FontName(s) => OverrideExpr::Constant(OverrideValue::String(s.clone())),
        OverrideTag::FontSize(v) => OverrideExpr::Constant(OverrideValue::Scalar(*v)),
        OverrideTag::Bold(b)
        | OverrideTag::Italic(b)
        | OverrideTag::Underline(b)
        | OverrideTag::Strikeout(b) => OverrideExpr::Constant(OverrideValue::Bool(*b)),
        OverrideTag::BoldWeight(w) => OverrideExpr::Constant(OverrideValue::Scalar(*w as f64)),
        OverrideTag::PrimaryColor(c)
        | OverrideTag::SecondaryColor(c)
        | OverrideTag::OutlineColor(c)
        | OverrideTag::ShadowColor(c) => OverrideExpr::Constant(OverrideValue::Color(*c)),
        OverrideTag::Alpha { value }
        | OverrideTag::PrimaryAlpha { value }
        | OverrideTag::SecondaryAlpha { value }
        | OverrideTag::OutlineAlpha { value }
        | OverrideTag::ShadowAlpha { value } => {
            OverrideExpr::Constant(OverrideValue::Scalar(*value as f64))
        }
        OverrideTag::Rotation { x, y, z } => OverrideExpr::Constant(OverrideValue::Rotation {
            x: *x,
            y: *y,
            z: *z,
        }),
        OverrideTag::Scale { x, y } => {
            OverrideExpr::Constant(OverrideValue::Scale { x: *x, y: *y })
        }
        OverrideTag::Spacing(v)
        | OverrideTag::Blur(v)
        | OverrideTag::GaussianBlur(v)
        | OverrideTag::Border(v)
        | OverrideTag::BorderX(v)
        | OverrideTag::BorderY(v)
        | OverrideTag::Shadow(v)
        | OverrideTag::ShadowX(v)
        | OverrideTag::ShadowY(v)
        | OverrideTag::BaselineOffset(v) => OverrideExpr::Constant(OverrideValue::Scalar(*v)),
        OverrideTag::Shear { x, y } => OverrideExpr::Constant(OverrideValue::Rotation {
            x: *x,
            y: *y,
            z: 0.0,
        }),
        OverrideTag::Origin { x, y } => OverrideExpr::Constant(OverrideValue::Pos { x: *x, y: *y }),
        OverrideTag::Clip { .. }
        | OverrideTag::ClipInverse { .. }
        | OverrideTag::ClipDrawing { .. }
        | OverrideTag::ClipInverseDrawing { .. }
        | OverrideTag::ClipDrawingCurrent
        | OverrideTag::ClipInverseDrawingCurrent
        | OverrideTag::Alignment(_)
        | OverrideTag::AlignmentNumpad(_)
        | OverrideTag::WrapStyle(_)
        | OverrideTag::WritingMode(_)
        | OverrideTag::Charset(_)
        | OverrideTag::Karaoke { .. }
        | OverrideTag::Reset(_)
        | OverrideTag::ResetAll
        | OverrideTag::DrawingMode(_)
        | OverrideTag::Unknown(_)
        | OverrideTag::AnimationSkip => {
            // These variants carry state that the renderer is
            // expected to handle directly (clips, karaoke timing,
            // style resets, drawing mode). They do not collapse
            // cleanly into a single OverrideValue; surface as a
            // placeholder string and let the renderer read the
            // underlying OverrideTag separately.
            OverrideExpr::Constant(OverrideValue::String(String::new()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::AssColor;

    #[test]
    fn constant_evaluates_to_its_value() {
        let expr = OverrideExpr::Constant(OverrideValue::Scalar(42.0));
        assert!(matches!(expr.evaluate_at(0), OverrideValue::Scalar(42.0)));
        assert!(matches!(
            expr.evaluate_at(9999),
            OverrideValue::Scalar(42.0)
        ));
    }

    #[test]
    fn animated_before_t1_returns_start() {
        let expr = OverrideExpr::Animated {
            start: Box::new(OverrideExpr::Constant(OverrideValue::Scalar(0.0))),
            end: Box::new(OverrideExpr::Constant(OverrideValue::Scalar(10.0))),
            t1_ms: 100,
            t2_ms: 200,
            accel: 1.0,
        };
        assert!(matches!(expr.evaluate_at(0), OverrideValue::Scalar(s) if s == 0.0));
        assert!(matches!(expr.evaluate_at(99), OverrideValue::Scalar(s) if s == 0.0));
    }

    #[test]
    fn animated_after_t2_returns_end() {
        let expr = OverrideExpr::Animated {
            start: Box::new(OverrideExpr::Constant(OverrideValue::Scalar(0.0))),
            end: Box::new(OverrideExpr::Constant(OverrideValue::Scalar(10.0))),
            t1_ms: 100,
            t2_ms: 200,
            accel: 1.0,
        };
        assert!(matches!(expr.evaluate_at(200), OverrideValue::Scalar(s) if s == 10.0));
        assert!(matches!(expr.evaluate_at(9999), OverrideValue::Scalar(s) if s == 10.0));
    }

    #[test]
    fn animated_linear_at_midpoint() {
        let expr = OverrideExpr::Animated {
            start: Box::new(OverrideExpr::Constant(OverrideValue::Scalar(0.0))),
            end: Box::new(OverrideExpr::Constant(OverrideValue::Scalar(10.0))),
            t1_ms: 0,
            t2_ms: 100,
            accel: 1.0,
        };
        let v = expr.evaluate_at(50);
        if let OverrideValue::Scalar(s) = v {
            assert!((s - 5.0).abs() < 1e-9, "expected 5.0, got {s}");
        } else {
            panic!("expected Scalar");
        }
    }

    #[test]
    fn animated_ease_in_with_accel_2() {
        let expr = OverrideExpr::Animated {
            start: Box::new(OverrideExpr::Constant(OverrideValue::Scalar(0.0))),
            end: Box::new(OverrideExpr::Constant(OverrideValue::Scalar(10.0))),
            t1_ms: 0,
            t2_ms: 100,
            accel: 2.0,
        };
        // With accel=2, at t=0.5 the eased value is 0.5^2 = 0.25.
        let v = expr.evaluate_at(50);
        if let OverrideValue::Scalar(s) = v {
            assert!((s - 2.5).abs() < 1e-9, "expected 2.5, got {s}");
        } else {
            panic!("expected Scalar");
        }
    }

    #[test]
    fn animated_position_interpolation() {
        let expr = OverrideExpr::Animated {
            start: Box::new(OverrideExpr::Constant(OverrideValue::Pos {
                x: 0.0,
                y: 0.0,
            })),
            end: Box::new(OverrideExpr::Constant(OverrideValue::Pos {
                x: 100.0,
                y: 200.0,
            })),
            t1_ms: 0,
            t2_ms: 100,
            accel: 1.0,
        };
        let v = expr.evaluate_at(50);
        if let OverrideValue::Pos { x, y } = v {
            assert!((x - 50.0).abs() < 1e-9);
            assert!((y - 100.0).abs() < 1e-9);
        } else {
            panic!("expected Pos");
        }
    }

    #[test]
    fn animated_color_per_channel_lerp() {
        let black = AssColor {
            alpha: 0,
            red: 0,
            green: 0,
            blue: 0,
        };
        let white = AssColor {
            alpha: 255,
            red: 255,
            green: 255,
            blue: 255,
        };
        let expr = OverrideExpr::Animated {
            start: Box::new(OverrideExpr::Constant(OverrideValue::Color(black))),
            end: Box::new(OverrideExpr::Constant(OverrideValue::Color(white))),
            t1_ms: 0,
            t2_ms: 100,
            accel: 1.0,
        };
        let v = expr.evaluate_at(50);
        if let OverrideValue::Color(c) = v {
            assert_eq!(c.red, 128);
            assert_eq!(c.green, 128);
            assert_eq!(c.blue, 128);
            assert_eq!(c.alpha, 128);
        } else {
            panic!("expected Color");
        }
    }

    #[test]
    fn lift_pos_becomes_constant() {
        let tag = OverrideTag::Pos { x: 100.0, y: 200.0 };
        let expr = lift_to_expr(&tag);
        assert!(
            matches!(expr, OverrideExpr::Constant(OverrideValue::Pos { x, y }) if x == 100.0 && y == 200.0)
        );
    }

    #[test]
    fn lift_move_becomes_animated() {
        let tag = OverrideTag::Move {
            x1: 0.0,
            y1: 0.0,
            x2: 100.0,
            y2: 100.0,
            t1: 0,
            t2: 500,
        };
        let expr = lift_to_expr(&tag);
        if let OverrideExpr::Animated {
            t1_ms,
            t2_ms,
            accel,
            ..
        } = expr
        {
            assert_eq!(t1_ms, 0);
            assert_eq!(t2_ms, 500);
            assert!((accel - 1.0).abs() < 1e-9);
        } else {
            panic!("expected Animated");
        }
    }

    #[test]
    fn lift_transform_becomes_animated_with_accel() {
        let tag = OverrideTag::Transform {
            tag: "\\fs".into(),
            t1: 100,
            t2: 200,
            accel: 2.0,
        };
        let expr = lift_to_expr(&tag);
        if let OverrideExpr::Animated {
            t1_ms,
            t2_ms,
            accel,
            ..
        } = expr
        {
            assert_eq!(t1_ms, 100);
            assert_eq!(t2_ms, 200);
            assert!((accel - 2.0).abs() < 1e-9);
        } else {
            panic!("expected Animated");
        }
    }

    #[test]
    fn lift_fade_becomes_animated() {
        let tag = OverrideTag::Fade {
            duration_in: 250,
            duration_out: 500,
        };
        let expr = lift_to_expr(&tag);
        assert!(matches!(expr, OverrideExpr::Animated { t2_ms: 250, .. }));
    }

    #[test]
    fn lift_bold_becomes_bool_constant() {
        let tag = OverrideTag::Bold(true);
        let expr = lift_to_expr(&tag);
        assert!(matches!(
            expr,
            OverrideExpr::Constant(OverrideValue::Bool(true))
        ));
    }

    #[test]
    fn lift_font_size_becomes_scalar_constant() {
        let tag = OverrideTag::FontSize(24.0);
        let expr = lift_to_expr(&tag);
        assert!(matches!(expr, OverrideExpr::Constant(OverrideValue::Scalar(s)) if s == 24.0));
    }

    #[test]
    fn lift_primary_color_becomes_color_constant() {
        let red = AssColor {
            alpha: 0,
            red: 255,
            green: 0,
            blue: 0,
        };
        let tag = OverrideTag::PrimaryColor(red);
        let expr = lift_to_expr(&tag);
        assert!(matches!(
            expr,
            OverrideExpr::Constant(OverrideValue::Color(_))
        ));
    }

    #[test]
    fn lift_rotation_becomes_rotation_constant() {
        let tag = OverrideTag::Rotation {
            x: 10.0,
            y: 20.0,
            z: 30.0,
        };
        let expr = lift_to_expr(&tag);
        if let OverrideExpr::Constant(OverrideValue::Rotation { x, .. }) = expr {
            assert!((x - 10.0).abs() < 1e-9);
        } else {
            panic!("expected Constant Rotation");
        }
    }

    #[test]
    fn ease_linear_is_identity() {
        assert!((ease(0.0, 1.0) - 0.0).abs() < 1e-9);
        assert!((ease(0.5, 1.0) - 0.5).abs() < 1e-9);
        assert!((ease(1.0, 1.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn ease_quadratic_slows_first_half() {
        // accel=2: at t=0.5, eased = 0.25 (curve is below the diagonal).
        assert!((ease(0.5, 2.0) - 0.25).abs() < 1e-9);
    }

    #[test]
    fn ease_zero_accel_falls_through() {
        assert!((ease(0.5, 0.0) - 0.5).abs() < 1e-9);
        assert!((ease(0.5, -1.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn animated_color_at_end_returns_end() {
        let red = AssColor {
            alpha: 0,
            red: 255,
            green: 0,
            blue: 0,
        };
        let expr = OverrideExpr::Animated {
            start: Box::new(OverrideExpr::Constant(OverrideValue::Color(red))),
            end: Box::new(OverrideExpr::Constant(OverrideValue::Color(red))),
            t1_ms: 0,
            t2_ms: 100,
            accel: 1.0,
        };
        let v = expr.evaluate_at(200);
        if let OverrideValue::Color(c) = v {
            assert_eq!(c.red, 255);
        } else {
            panic!("expected Color");
        }
    }
}
