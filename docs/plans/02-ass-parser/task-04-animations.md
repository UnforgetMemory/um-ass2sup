# Task 2.4 — `\t(\tag, t1, t2, accel)` animations

## Status: ✅ COMPLETED (2026-06-17)

## Goal

Parse and represent `\t` animation tags with the libass timing semantics
(start time, end time, acceleration curve), and evaluate them at a given
event-relative `time_ms`.

## Implementation

`OverrideExpr::Animated` captures the full libass `\t` semantics:

- `start` and `end` are themselves `OverrideExpr` (so nested `\t(\t(\b1), 0, 1000, 1)` works)
- `t1_ms` and `t2_ms` are event-relative times in milliseconds
- `accel` follows the libass convention: 1.0 = linear, 2.0 = ease-in (quadratic),
  0.5 = ease-out, `<= 0.0` falls through to identity

`Animator::evaluate_at(time_ms)` for the `Animated` variant:

1. If `t2_ms <= t1_ms` (degenerate interval), return end value
2. If `time_ms <= t1_ms`, return start value
3. If `time_ms >= t2_ms`, return end value
4. Otherwise compute `raw = (time_ms - t1_ms) / (t2_ms - t1_ms)`,
   then `t = ease(raw, accel)`, then `interpolate(start, end, t)`

`ease(t, accel)` follows libass: returns `t` when `accel == 1.0` (linear), falls
through to identity when `accel <= 0.0`, otherwise `t.powf(accel)` for the
smoothing curve.

`interpolate` covers all 7 `OverrideValue` variants:

- `Scalar`: linear lerp
- `Bool`: snap at `t == 0.5`
- `Pos`, `Rotation`, `Scale`: per-component lerp
- `Color`: per-channel 8-bit lerp
- `String`: snap at `t == 0.5` (string values do not interpolate)

## Nested `\t` support

Nested `\t(\t(\b1), 0, 1000, 1)` parses through `lift_to_expr` recursively because
the inner `\t` is itself an `OverrideTag::Transform`, which `lift_to_expr`
returns as an `Animated` node. The renderer's parser layer can
`lift_to_expr` on the inner tag after parsing the outer tag's first argument.

## Verification gates

- [x] 19 unit tests in `override_expr::tests` cover the animation evaluation
- [x] `cargo test -p ass-parser` — 147/147 pass
- [x] `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings
- [x] `cargo fmt --check` — clean
- [x] Ease function: identity at `accel = 1.0`, quadratic at `accel = 2.0`,
      falls through to identity when `accel <= 0.0`
- [x] Position interpolation: midpoint of (0,0)..(100,200) at t=0.5 is (50,100)
- [x] Color interpolation: per-channel 8-bit lerp (black→white at t=0.5 is rgb(128,128,128))
- [x] Edge cases: t ≤ t1 returns start, t ≥ t2 returns end
- [x] Edge case: t2 == t1 returns end (degenerate interval)
