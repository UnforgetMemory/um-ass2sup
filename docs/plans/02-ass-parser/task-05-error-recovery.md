# Task 2.5 — Error recovery (degraded mode)

## Status: ✅ COMPLETED (2026-06-17)

## Goal

Single-line errors do not abort the whole parse. The parser returns a usable AST
plus a structured warning list so callers can surface problems to the user without
losing the rest of the file.

## Files changed

| File | Change |
|------|--------|
| `crates/ass-parser/src/error.rs` | New `ParseWarning` enum (`InvalidField`, `UnknownSection`, `SrtBlockSkipped`) |
| `crates/ass-parser/src/lib.rs` | `AssFile.warnings: Vec<ParseWarning>` field; `parse_with_recovery()` entry point that populates it |
| `crates/ass-parser/tests/test_lenient.rs` | 17 new tests covering corrupted inputs |
| `crates/ass-parser/src/effect.rs` | `Effect` now implements `Display` |

## Verification gates

- [x] `parse_with_recovery()` returns a non-empty `AssFile` for every input the
      strict parser rejects
- [x] `AssFile.warnings` lists the specific recoverable issues
- [x] No panics on any input
- [x] Pre-existing issue: `StyleName == &str` comparison fixed
- [x] 17 new lenient-mode tests pass
- [x] `cargo test -p ass-parser` — 128/128 pass
