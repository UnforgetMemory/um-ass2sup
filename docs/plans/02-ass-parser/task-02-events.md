# Task 2.2 — Events 10 fields + Style references

## Status: ✅ COMPLETED (manual integration, 2026-06-17)

## Goal

Strong-type every Dialogue event field; replace the loose `event.style_name: String` with the new `StyleName` newtype so style references can be resolved at parse time.

## Files changed

| File | Change |
|------|--------|
| `crates/ass-parser/src/types.rs` | `StyleName` newtype (created in Task 2.1) |
| `crates/ass-parser/src/event.rs` | `Event::style_name: String` → `Event::style: StyleName` |
| `crates/ass-parser/src/lib.rs` | `find_style(name: impl AsRef<str>)`; `StyleName` is `AsRef<str>` |
| `crates/ass-parser/tests/test_event.rs` | `e.style_name` → `e.style` |
| `crates/ass-parser/tests/test_ass_parser.rs` | `event.style_name` → `event.style` |
| `crates/ass-parser/tests/test_fixtures.rs` | `e.style_name` → `e.style` |
| `crates/subtitle-renderer/src/renderer/mod.rs` | Call sites updated to `event.style` |
| `crates/subtitle-validator/src/rules.rs` | `event.style_name` → `event.style` |
| `crates/subtitle-validator/tests/test_edge_cases.rs` | Updated |
| `crates/ass2sup-cli/src/lib.rs` | Call site updated |

## Verification gates

- [x] 10 fields preserved (Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text)
- [x] `event.style` is `StyleName` (newtype over `String`)
- [x] `StyleName` supports `==` against `&str` (e.g., `event.style == "Default"`)
- [x] `find_style()` accepts both `&str` and `&StyleName`
- [x] All downstream crates (subtitle-renderer, subtitle-validator, ass2sup-cli) compile and pass tests
- [x] `cargo test --workspace` — 0 failures
