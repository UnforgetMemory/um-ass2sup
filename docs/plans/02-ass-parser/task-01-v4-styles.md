# Task 2.1 — V4+ Styles 22 fields

## Status: ✅ COMPLETED (manual integration, 2026-06-17)

## Goal

Strong-type every V4+ style field; introduce a `types` module that replaces raw `u8`/`u32` with intent-revealing enums and newtypes.

## Files changed

| File | Change |
|------|--------|
| `crates/ass-parser/src/types.rs` (new) | `StyleName` newtype, `BorderStyle` enum, `Alignment` enum, `Margins` struct, `Encoding` newtype |
| `crates/ass-parser/src/style.rs` | `Style` refactored to use new types; `to_ass_string()` for round-trip; `raw_alignment: u8` preserves the original raw value (needed for the V008 validator range check) |
| `crates/ass-parser/src/srt.rs` | `srt_default_style()` uses new types |
| `crates/ass-parser/tests/test_style.rs` | Field access updated to new types |

## New types

```rust
pub struct StyleName(pub String);
pub enum BorderStyle { OutlineAndShadow = 1, OpaqueBox = 3 }
pub enum Alignment { BottomLeft=1, BottomCenter=2, BottomRight=3,
                    MiddleLeft=4, MiddleCenter=5, MiddleRight=6,
                    TopLeft=7, TopCenter=8, TopRight=9 }
pub struct Margins { pub left: u32, pub right: u32, pub vertical: u32 }
pub struct Encoding(pub u8);
```

## Verification gates

- [x] 22 fields preserved (V4+ spec)
- [x] `BorderStyle::from_u8` returns `None` for non-{1, 3} values
- [x] `Alignment::from_u8` returns `None` for non-{1..=9} values
- [x] `Style::to_ass_string()` round-trips via `Style::parse_from_line()`
- [x] `raw_alignment` preserves the source value (1-255) so out-of-range values
      like 15 can be detected by the subtitle-validator's V008 check
- [x] `cargo test -p ass-parser` — 128/128 pass
- [x] `cargo clippy --workspace --all-targets -- -D warnings` — 0 warnings
