# Task 2.6 — SRT → ASS upgrade

## Status: ✅ COMPLETED (2026-06-17)

## Goal

Provide a first-class SRT → ASS upgrade path so `ass2sup input.srt -o output.sup`
"just works" without requiring the user to pre-convert SRT to ASS by hand.

## Files changed

| File | Change |
|------|--------|
| `crates/ass-parser/src/srt.rs` | New `SrtFile` struct; `AssFile::from_srt(srt)` upgrade path |
| `crates/ass-parser/src/event.rs` | `to_ass_time()` → `as_ass_time()` (consistent naming) |
| `crates/ass-parser/src/event.rs` | `Event::style_name: String` → `Event::style: StyleName` |
| `crates/ass-parser/src/srt.rs` | `SrtEvent::style_name` → `SrtEvent::style` (for consistency) |

## Round-trip guarantee

`SRT → AssFile::from_srt → to_srt() → SRT` preserves all events (timestamps + text).
This is the same self-check that the `ass2sup in.srt --to-srt -o out.srt && diff in.srt out.srt`
command has always relied on; the new code path makes it part of the unit-test suite.

## Verification gates

- [x] `AssFile::from_srt()` constructs a valid `AssFile` from any `SrtFile`
- [x] Round-trip `SRT → ASS → SRT` produces an equivalent event list
- [x] `srt_default_style()` produces a valid `Style` (added `raw_alignment: 2`)
- [x] 10 new tests covering SRT parse + upgrade + round-trip
- [x] `cargo test -p ass-parser` — 128/128 pass
