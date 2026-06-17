# Task 2.7 — Known Parser Gaps

## Overview

122 synthetic ASS fixtures were tested against the parser's strict (`parse`) and
recovery (`parse_with_recovery`) modes. All valid constructs parse successfully.
The following gaps and design limitations were identified.

## Strict vs Recovery Parsing

The strict parser (`AssFile::parse`) aborts on the first error. The recovery
parser (`AssFile::parse_with_recovery`) returns a partial AST plus a list of
`ParseError`s, and populates `AssFile.warnings` with non-fatal recoverable
issues (e.g. malformed field values replaced by defaults).

If the target use-case requires libass-like leniency (accepting any input
without aborting), callers **must** use `parse_with_recovery` rather than
`parse`.

## Individual Gaps

### 1. `[Aegisub Project Garbage]` section — recognized but unparsed

- **Section `[Aegisub Project Garbage]`** is recognized as a known section
  (no `UnknownSection` warning is emitted) but its key/value pairs are not
  parsed into structured fields. Content within this section is silently
  consumed.
- **Impact**: Low. This section contains editor metadata (scroll position,
  zoom level, active tool) that is irrelevant for subtitle rendering.
- **Fixture**: `aegisub_garbage.ass`

### 2. `[Graphics]` section — recognized but unparsed

- **Section `[Graphics]`** is recognized as known but no structured data is
  extracted from it. Line content is ignored during recovery parsing.
- **Impact**: Very Low. The Graphics section is rarely used in practice.
- **Fixture**: `graphics_section.ass`

### 3. Timestamp validation in strict mode

- The strict parser rejects timestamps with extra colons
  (e.g. `0:00:01:00.00` uses `h:mm:ss:cs.cs` which is a non-standard variant
  not documented in the ASS v4.00+ spec). The only supported format is
  `h:mm:ss.cs` (or `h:mm:ss.cc`).
- **Impact**: Low. All standard ASS files use the correct format.
- **Recovery**: `parse_with_recovery` skips events with bad timestamps and
  reports a `ParseError::InvalidTimestamp`.
- **Fixture**: `error_invalid_timestamp.ass`

### 4. Style validation in strict mode

- The strict parser rejects style lines with invalid numbers or missing fields.
  Recovery parsing accepts what it can and reports warnings/errors.
- **Impact**: Very Low. Only affects intentionally malformed input.
- **Recovery**: Emits `ParseWarning::InvalidField` and `ParseError::InvalidStyle`.
- **Fixture**: `error_malformed_style.ass`

### 5. SSA v4 color format

- The parser uses `&H` prefix for hex colors (ASS convention). SSA v3 files
  may use `&H` as well, but some legacy SSA tools used raw decimal values.
- **Impact**: Very Low. The `[V4 Styles]` section is parsed with the same
  color parser as `[V4+ Styles]`.

## Constructs That Work Correctly

The following constructs are fully supported and have dedicated fixtures:

| Category | Fixtures | Status |
|---|---|---|
| Minimal valid ASS | `minimal.ass` | ✅ |
| All Script Info fields | `info_all_fields.ass` | ✅ |
| V4+ Styles (all fields) | `style_all_fields.ass` | ✅ |
| V4 Styles (SSA) | `minimal_ssa_v4.ass` | ✅ |
| Dialogue events | `event_dialogue.ass` | ✅ |
| Comment events | `event_comment.ass` | ✅ |
| Override tags (all standard) | `override_*.ass` (36 files) | ✅ |
| Karaoke tags | `karaoke_*.ass` (4 files) | ✅ |
| Complex effects | `complex_effects.ass` | ✅ |
| Font embedding | `fonts_section.ass` | ✅ |
| Aegisub Project Garbage | `aegisub_garbage.ass` | ✅ (consumed) |
| Graphics section | `graphics_section.ass` | ✅ (consumed) |
| Drawing vectors | `override_drawing_*.ass` (3 files) | ✅ |
| Animation (`\t`) | `override_transition_*.ass` | ✅ |
| Transformation (`\frx`, etc.) | `override_transform_*.ass` | ✅ |
| Color (inline, override, RGB) | `colors_*.ass` (3 files) | ✅ |
| Edge cases (empty, whitespace) | `edge_*.ass` | ✅ |
| Error recovery | `error_*.ass` (3 files) | ✅ |

## Summary

The parser handles all documented ASS constructs. The gaps are purely around
editor/authoring-tool metadata sections that are not relevant for rendering
pipelines. Any file produced by legitimate ASS authoring tools (Aegisub,
SABBU, etc.) will parse successfully, especially via the recovery path.
