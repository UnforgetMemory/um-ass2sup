# Test Coverage Report

Generated: 2026-07-13

## Summary

| Metric | Value |
|--------|-------|
| Total crates | 8 (workspace) + 1 parallel workspace (ass2sup-libass) |
| Total test functions (`#[test]`) | **734** |
| Total integration test files | **29** |
| Total doc test blocks (`/// \`\`\``) | **40** |
| Property-based test (`proptest!`) files | 4 |
| Insta snapshot files | 5 |
| Fuzz targets | 6 across 3 crates |

---

## Per-Crate Breakdown

### `subtitle-renderer` (237 tests) ✅ **Highest coverage**
- 237 `#[test]` functions across 7 integration test files
- 2 doc test blocks
- 13 inline `mod tests` in source files
- Covers: font pipeline (types, discovery, index, registry, database, shaper, rasterizer), rendering (context, layout, font_registry_renderer, drawing), effects (composite, blur, shadow, clip), animation (fade, transform, move), karaoke, transform/SIMD
- **Assessment**: Excellent coverage. The font subsystem and rendering pipeline are thoroughly tested.

### `ass-core` (184 tests) ✅ **Strong coverage**
- 184 `#[test]` functions across 4 integration test files
- 18 doc test blocks
- 11 inline `mod tests` in source files
- Property-based tests (`proptest.rs`)
- 4 fuzz targets (parse_ass, parse_lenient, parse_override_tag, parse_srt)
- **Assessment**: Very strong. Critical parsing paths have both unit tests and fuzz targets.

### `color-quantizer` (106 tests) ✅ **Strong coverage**
- 106 `#[test]` functions across 3 integration test files
- 6 doc test blocks
- Property-based tests (`proptest.rs`)
- 1 fuzz target (quantize_rgba)
- 8 inline `mod tests` covering: median_cut, nearest, palette, temporal, naarahara, transfer, tonemap, floyd_steinberg, ordered, adaptive dithering
- **Assessment**: Comprehensive. Both quantizer and dither paths are well-tested.

### `ass2sup-cli` (78 tests) ✅ **Good coverage**
- 78 `#[test]` functions across 9 integration test files
- 5 insta snapshot files for CLI output
- Tests: CLI args, BDN XML output, conversion, error handling, integration, OCR e2e, telemetry
- **Assessment**: Good end-to-end coverage. Snapshot tests provide regression protection for CLI help/error output.

### `pgs-encoder` (50 tests) ✅ **Moderate coverage**
- 50 `#[test]` functions across 2 integration test files
- Property-based RLE roundtrip tests
- 1 fuzz target (decode_pgs)
- 2 inline `mod tests`
- **Assessment**: Moderate. Golden tests + RLE roundtrip provide core coverage, but the DDD domain model could use more edge case tests.

### `subtitle-validator` (43 tests) ✅ **Moderate coverage**
- 43 `#[test]` functions across 2 integration test files
- 8 doc test blocks
- 1 inline `mod tests` in rules.rs (39 lines)
- **Assessment**: Moderate. Core validation rules are covered, but there's room for more edge cases.

### `bdn-xml` (20 tests) ✅ **Light coverage**
- 20 `#[test]` functions across 1 integration test file
- 6 doc test blocks
- Property-based tests embedded in `xml.rs`
- **Assessment**: Light but adequate for a simple XML serialization crate.

### `subtitle-renderer-libass` (15 tests) ⚠️ **Low coverage**
- 15 `#[test]` functions (all inline `mod tests`)
- 0 integration test files
- 0 doc test blocks
- Tests: pgs_adapter (1), vendor (4), composer (4), timeline (7)
- **Assessment**: Low coverage, especially for a crate that bridges to C FFI (libass). Missing: integration tests that exercise the full libass render pipeline, error handling for libass failures, and edge cases in frame compositing.

### `libass-sys` (1 test) ⚠️ **Minimal coverage**
- 1 `#[test]` function in 1 integration test file
- 0 doc test blocks
- Single smoke test checking `ass_library_version()` returns a value > 0
- **Assessment**: Minimal. For FFI bindings to a C library, this is borderline acceptable — the test exists only to verify linkage. The real libass testing happens through `subtitle-renderer-libass`.

---

## Fuzz Targets

| Crate | Fuzz Target | File |
|-------|-------------|------|
| `ass-core` | `parse_ass` | `fuzz/fuzz_targets/parse_ass.rs` |
| `ass-core` | `parse_lenient` | `fuzz/fuzz_targets/parse_lenient.rs` |
| `ass-core` | `parse_override_tag` | `fuzz/fuzz_targets/parse_override_tag.rs` |
| `ass-core` | `parse_srt` | `fuzz/fuzz_targets/parse_srt.rs` |
| `color-quantizer` | `quantize_rgba` | `fuzz/fuzz_targets/quantize_rgba.rs` |
| `pgs-encoder` | `decode_pgs` | `fuzz/fuzz_targets/decode_pgs.rs` |

**Assessment**: All fuzz targets appear current and should compile with the existing API surface.

---

## Stale Artifacts (Cleaned)

| Path | Size | Status | Reason |
|------|------|--------|--------|
| `.output/ass.dll` | 3.5 MB | ✅ **DELETED** | Build artifact from libass cross-compilation, gitignored |
| `.output/ass2sup.exe` | 3.8 MB | ✅ **DELETED** | Build artifact from libass cross-compilation, gitignored |
| `ass2sup-libass/.output/battleship-v5-20260630-174933.sup` | 51.8 MB | ⬜ **NOTED** | In separate workspace (ass2sup-libass), not cleaned |
| `.localref/The.Battleship.Island.2017.DC.BluRay.1080p.DTS-HD.MA5.1_zh_CN.sup` | 29.4 MB | ⬜ **KEPT** | Reference test data, gitignored but deliberately placed |

**Not cleaned:**
- `target/` (23 GB) — build cache, essential for incremental compilation
- `.localref/*.sup` — reference files used for end-to-end verification (see AGENTS.md §16)
- `tests/fixtures/*` — version-controlled test fixtures

---

## Observations

1. **`subtitle-renderer-libass` has no integration tests** despite being the FFI bridge for the libass backend. The inline tests only cover isolated domain logic, not the full render pipeline.

2. **`libass-sys` has only one smoke test.** As bare FFI bindings this is arguably sufficient, but an upgrade-DLL compatibility test would be valuable.

3. **`bdn-xml` coverage is light** but proportional to its scope (XML serialization).

4. **Snapshot tests exist only in `ass2sup-cli`.** Other crates don't use insta, which is fine — most have deterministic output suitable for `assert_eq!` testing.

5. **Doc test coverage is uneven.** `ass-core` leads with 18 doc test blocks, while several crates have 0.

6. **Fuzz targets are concentrated in `ass-core`** (4/6), which makes sense — parsing is the primary attack surface.

7. **No crates have `unsafe_code` deny except `ass-core`** (per AGENTS.md). The other crates should be audited for unsafe usage.

---

## Recommendations

| Priority | Recommendation | Crate(s) |
|----------|---------------|----------|
| High | Add integration tests for libass render pipeline | `subtitle-renderer-libass` |
| Medium | Add doc test blocks for public API surfaces | `ass2sup-cli`, `pgs-encoder`, `subtitle-renderer-libass` |
| Low | Consider property-based tests for segment encoding | `pgs-encoder` |
| Low | Add a linked-library version smoke test | `libass-sys` |
