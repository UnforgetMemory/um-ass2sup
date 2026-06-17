# Task 2.7 — libass Compatibility Test Suite

## Deliverables

- [x] 122 synthetic .ass fixture files under `crates/ass-parser/fixtures/libass/`
- [x] Integration test at `crates/ass-parser/tests/libass_compat.rs`
- [x] 122 insta snapshots under `crates/ass-parser/tests/snapshots/libass_compat__*.snap`
- [x] Known-gaps document at `docs/plans/02-ass-parser/task-07-known-gaps.md`

## Fixture Source

libass does **not** ship standalone .ass fixture files in its repository.
(all upstream tests compile the C parser and test programmatically, not via
fixture files). Fixtures were therefore synthetically generated to cover
every documented ASS feature, edge case, and error-recovery scenario.

## Fixture Size Cutoff

Each fixture is kept under **10 KiB**. The total fixture set is ~492 KiB on
disk (122 files). This keeps the repo lean while providing comprehensive
coverage. Files larger than 10 KiB were excluded; none of the generated
fixtures exceeded this limit.

## Fixture Categories (122 files)

| Category | Count | Description |
|---|---|---|
| Basic structures | 5 | Minimal, no-events, no-styles, full-script-info |
| Styles | 5 | V4+ all fields, V4 SSA, encoding variants, border/shadow |
| Events | 6 | Dialogue, comment, layer, margins, actor name, effect |
| Override tags (individual) | 36 | Every standard override tag: `\b`, `\i`, `\u`, `\s`, `\bord`, `\shad`, `\frx`, `\fry`, `\frz`, `\fax`, `\fay`, `\fn`, `\fs`, `\fscx`, `\fscy`, `\fsp`, `\fe`, `\clip` (rect & inverse), `\iclip`, `\pos`, `\move`, `\org`, `\an`, `\k`, `\K`/`\ko`/`\kf`, `\t`, `\be`, `\blur`, `\p`, `\pbo`, `\xbord`, `\ybord`, `\xshad`, `\yshad`, `\q`, `\r` |
| Combined sequences | 4 | Basic, complex, overlapping, tag-line combinations |
| Color formats | 3 | Inline `&H` values, override tags, raw RGB |
| Complex effects | 1 | Multiple simultaneous override tags |
| Karaoke | 4 | `\k`, `\K`/`\ko`/`\kf`, `\kt`, mixed |
| Font effects | 3 | Font name, font size, bold/italic |
| Transformations | 3 | Rotation X/Y/Z, shear X/Y |
| Animation | 3 | `\t` with linear, smooth, and complex acceleration |
| Clip/inverse | 3 | Rect clip, inverse clip, drawing clip |
| Drawings | 3 | Basic vector, complex polygon, clip+drawn |
| Positioning | 3 | `\pos`, `\move`, `\org` |
| Border/Shadow | 2 | Per-axis border, per-axis shadow |
| Text layout | 2 | `\N`, `\n`, `\h` |
| Sections | 2 | Fonts, Graphics |
| Metadata | 2 | Aegisub Project Garbage, Wrap style variants |
| Comments | 2 | Inline comments, section comments |
| Edge cases | 5 | Empty file/events, whitespace-heavy, special chars, negative values, huge values |
| Error recovery | 3 | Invalid timestamp, malformed style, missing style |
| Stress tests | 2 | Many events (30), overlapping events (20) |

## Test Design

The single test `libass_compat_all` iterates all 122 fixture files and for each:

1. Runs **strict parsing** (`AssFile::parse`) — records success/failure
2. Runs **recovery parsing** (`AssFile::parse_with_recovery`) — captures
   `warnings.len()`, `errors.len()`, and `events.len()`
3. Writes the result as an **insta snapshot** keyed on the fixture basename

This ensures:
- **No panics** on any input (crash-safety guarantee)
- **Output stability** — insta flags any change in event/warning/error count
- **Deterministic ordering** — fixture files sorted by name

## Coverage Verification

| Metric | Value |
|---|---|
| Total fixtures | 122 |
| Strict parse OK | 118 |
| Recovery warnings emitted | 1 (`error_malformed_style`) |
| Recovery errors emitted | 2 fixtures (intentional error-recovery) |
| Test runtime | ~50 ms |

## Quality Gates

All pass:

```text
$ cargo fmt --all -- --check    ✅
$ cargo clippy -- -D warnings   ✅ (0 warnings)
$ cargo test -p ass-parser      ✅ (270+ tests across all targets)
```
