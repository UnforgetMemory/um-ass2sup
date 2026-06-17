# Task 2.3 — Override tag parser

## Status: ✅ COMPLETED (2026-06-17)

## Goal

Replace the existing flat `OverrideTag` enum with a richer `OverrideExpr` AST that
distinguishes constants from animated expressions, enabling the renderer to apply
keyframes for the `\t(\tag, t1, t2, accel)` family.

## Files changed

| File | Change |
|------|--------|
| `crates/ass-parser/src/override_expr.rs` (new) | `OverrideValue` enum (Scalar/Color/Pos/Rotation/Scale/Bool/String), `OverrideExpr` enum (Constant/Animated), `Animator` trait, `lift_to_expr()` adapter, `ease()` function, `interpolate()` for all variants |
| `crates/ass-parser/src/lib.rs` | `pub mod override_expr;` and re-exports |

## AST design

```rust
pub enum OverrideValue {
    Scalar(f64),
    Color(AssColor),
    Pos { x: f64, y: f64 },
    Rotation { x: f64, y: f64, z: f64 },
    Scale { x: f64, y: f64 },
    Bool(bool),
    String(String),
}

pub enum OverrideExpr {
    Constant(OverrideValue),
    Animated {
        start: Box<OverrideExpr>,
        end: Box<OverrideExpr>,
        t1_ms: u64,
        t2_ms: u64,
        accel: f64,
    },
}

pub trait Animator {
    fn evaluate_at(&self, time_ms: u64) -> OverrideValue;
}
```

## Adapter

`lift_to_expr(tag: &OverrideTag) -> OverrideExpr` lifts the flat `OverrideTag`
enum into the typed AST:

- Static tags (`Pos`, `FontName`, `FontSize`, `Bold`, `Italic`, `Underline`,
  `Strikeout`, `BoldWeight`, `PrimaryColor`, `SecondaryColor`, `OutlineColor`,
  `ShadowColor`, `Alpha`, `Rotation`, `Scale`, `Spacing`, `Blur`,
  `GaussianBlur`, `Border`, `BorderX`, `BorderY`, `Shadow`, `ShadowX`,
  `ShadowY`, `BaselineOffset`, `Shear`, `Origin`) become `Constant`.
- Time-bearing tags (`Move`, `Fade`, `FadeComplex`, `Transform`) become
  `Animated`.
- Tags that carry renderer state (`Clip`, `Karaoke`, `Drawing`, etc.) become
  `Constant(String::new())` placeholders so the renderer can read the
  underlying `OverrideTag` separately.

## Verification gates

- [x] 19 new unit tests in `override_expr::tests` (lift + evaluate + animation + ease)
- [x] `cargo test -p ass-parser` — 147/147 pass
- [x] `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings
- [x] `cargo fmt --check` — clean
- [x] No breaking change to existing `OverrideTag` API
