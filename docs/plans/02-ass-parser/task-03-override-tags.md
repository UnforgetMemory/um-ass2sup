# Task 2.3 — Override tag parser (Deferred)

## Status: ⏳ DEFERRED (worker session returned without writing code; tracked as a follow-up)

## Goal (planned)

Replace the existing flat `OverrideTag` enum with a richer `OverrideExpr` AST that
distinguishes constants from animated expressions, enabling the renderer to apply
keyframes for the `\t(\tag, t1, t2, accel)` family.

## Why deferred

The override-tag-impl session was created but exited without writing any code. The
existing `OverrideTag` enum in `crates/ass-parser/src/override_tag.rs` continues to
work and the rest of the workspace compiles + passes tests against it. The plan was
to add an `OverrideExpr` layer on top; that work is unchanged.

## Follow-up

Re-dispatch this task in Sprint 1.5 or Sprint 1.6:

1. Add `OverrideExpr` enum: `Scalar(f64) | Color(AssColor) | Animated { tag, start, end, accel } | Transform(...)`
2. Add `Animator` trait: `fn evaluate(time_ms) -> Value`
3. Map every existing `OverrideTag` variant to a corresponding `OverrideExpr` node
4. Add the libass tag-list unit tests (50+)
5. Wire `\t(\tag, t1, t2, accel)` including nested `\t` inside `\t`
6. Visual diff against libass binary output (no execution, just structural compare)

## Tracking issue

Open against the dev-2026-06-17 branch with label `sprint1-followup/override-expr`.
