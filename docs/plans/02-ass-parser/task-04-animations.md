# Task 2.4 — `\t(\tag, t1, t2, accel)` animations (Deferred)

## Status: ⏳ DEFERRED (depends on Task 2.3)

## Goal (planned)

Parse and represent `\t` animation tags, including:

- Linear interpolation between start and end values
- Acceleration curve (third numeric parameter, default 1.0)
- Nested `\t()` inside `\t()` for compound animations
- The 4-argument form: `\t(\tag, t1, t2, accel)`
- The 3-argument form: `\t(\tag, t1, t2)` (accel = 1.0)

## Why deferred

This task depends on the `OverrideExpr` AST from Task 2.3. With the OverrideExpr
not yet in place, there is no clean place to hang the animation evaluation.

## Follow-up

After Task 2.3 lands:

1. Add `Animated { tag: Box<OverrideExpr>, start_ms, end_ms, accel: f32 }` variant to `OverrideExpr`
2. Implement `Animator::evaluate(time_ms)` for Animated
3. Test fixtures for every libass animation curve
4. Visual diff against libass binary output

## Tracking issue

Tracked together with Task 2.3 (`sprint1-followup/override-expr`).
