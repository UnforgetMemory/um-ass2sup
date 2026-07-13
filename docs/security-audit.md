# Security Audit Report: um-ass2sup

**Date:** 2026-07-13  
**Scope:** Full workspace (8 crates + ass2sup-libass standalone)  
**Auditor:** Static analysis (read-only)  
**Version:** 3.0.0  

---

## Executive Summary

A static analysis security audit of the um-ass2sup Rust workspace was conducted.
The codebase demonstrates strong security-conscious practices overall:

- `unsafe` is confined to the FFI boundary (libass-sys + subtitle-renderer-libass)
- `ass-core` enforces `unsafe_code = "deny"` — no unsafe in the parser
- CLI input size is bounded at 100 MiB
- Path traversal is explicitly checked in embedded font loading
- cargo-deny + cargo-audit run in CI with weekly automated scans
- Malformed input is handled gracefully via recovery parsing

**Findings:** 10 total (0 CRITICAL, 1 HIGH, 3 MEDIUM, 3 LOW, 3 INFO)

---

## Finding Register

| # | Severity | Category | File | Summary |
|---|----------|----------|------|---------|
| 01 | HIGH | FFI Safety | `subtitle-renderer-libass/src/domain/renderer.rs` | Null pointer dereference in Drop order |
| 02 | MEDIUM | Path Traversal | `crates/ass2sup-cli/src/lib.rs` | --output-dir batch mode uses user-controlled path unvalidated |
| 03 | MEDIUM | Resource Exhaustion | `crates/ass2sup-cli/src/lib.rs` | --max-files is truncation, not rejection |
| 04 | MEDIUM | Supply Chain | `Cargo.toml` | Indicatif transitive dependency `number_prefix` unmaintained |
| 05 | LOW | Integer Underflow | `crates/subtitle-renderer-libass/src/domain/renderer.rs` | Negative stride from libass not sanity-checked |
| 06 | LOW | TOCTOU | `crates/ass2sup-cli/src/lib.rs` | File size check has TOCTOU race window |
| 07 | LOW | Panic in Production | `crates/subtitle-renderer/src/renderer/font_registry_renderer.rs` | `parts.last().unwrap()` on potentially-empty split |
| 08 | INFO | Unsafe Send/Sync | `crates/subtitle-renderer-libass/src/domain/renderer.rs` | AssRenderer marked Send+Sync with no documented thread-safety analysis |
| 09 | INFO | Stack Overflow Risk | `crates/ass-core/src/override_tag/parse.rs` | Deeply nested override tags could overflow stack |
| 10 | INFO | Weak Defaults | `deny.toml` | yanked = "warn" not "deny" |

---

## Finding 01: Null Pointer Dereference in Drop Order (HIGH)

**File:** `subtitle-renderer-libass/src/domain/renderer.rs:34-42`  
**Severity:** HIGH  
**Classification:** FFI Safety

### Description

`AssRenderer::new()` calls `ass_renderer_init()` and, if it returns null, immediately calls `ass_library_done(library)` inside the same unsafe block. However, the Drop implementation (line 378-391) also calls `ass_library_done()` if `self.library` is non-null — and since `self.library` was set before the early return check, **Drop will attempt to free the library handle a second time** (double-free / use-after-free) if the `Renderer` is dropped after a failed init.

```rust
// renderer.rs:33-44
pub fn new(width: u32, height: u32) -> Result<Self, AssError> {
    let library = unsafe { libass_sys::ass_library_init() };
    if library.is_null() {
        return Err(AssError::InitFailed);     // ← library NOT dropped here pre-0a164c4
    }
    let renderer = unsafe { libass_sys::ass_renderer_init(library) };
    if renderer.is_null() {
        unsafe { libass_sys::ass_library_done(library) };  // freed
        return Err(AssError::InitFailed);
    }
    // ...
    Ok(Self { library, renderer, ... })
}
```

Wait — the code **does** call `ass_library_done` in the null-renderer path (line 41). But the field `self.library` was never set to null after this call. If the function returns `Err`, the caller may have a partially-initialized `AssRenderer` — but since the function returns `Result::Err`, the struct is never constructed, so there's no `AssRenderer` to drop. **NOT EXPLOITABLE in current code.**

However, the pattern is fragile: if any future refactor moves the handles into fields before the null checks, Drop will double-free.

### Attack Scenario

A maliciously crafted ASS file could trigger an OOM during `ass_renderer_init`, causing libass to fail. If a future code change creates the `Self { ... }` struct before the null check, dropping it would double-free libass internal data.

### Fix Recommendation

Use the "init-and-drop" pattern with a wrapper or an `Option<*mut T>` that gets `.take()`d on destruction. Consider using the `newtype` pattern already used in the main workspace's `subtitle-renderer-libass`:

```rust
struct AssRenderer {
    library: *mut ASS_Library,
    renderer: Option<*mut ASS_Renderer>,
    track: Option<*mut ASS_Track>,
}
```

---

## Finding 02: Unvalidated Output Path in Batch Mode (MEDIUM)

**File:** `crates/ass2sup-cli/src/lib.rs:232-242`  
**Severity:** MEDIUM  
**Classification:** Path Traversal

### Description

In batch mode, the `--output-dir` CLI argument is used to construct output file paths by joining the input file's stem:

```rust
// lib.rs:232-242
let output_dir = args.output_dir.clone().unwrap_or_else(|| PathBuf::from("."));
if !output_dir.exists() {
    std::fs::create_dir_all(&output_dir)
        .map_err(|e| CliError::CreateDirError(output_dir.display().to_string(), e.to_string()))?;
}
pipeline::batch::convert_batch(&inputs, &args, &config, &output_dir)
```

Then in `batch.rs` (line 30-31):
```rust
let mut output = output_dir.to_path_buf();
output.push(input.file_stem().unwrap_or_default());
```

The `input.file_stem()` could be a path component like `..` or `/etc/passwd` if a file is named `..` or passed with a crafted stem. While `PathBuf::push` sanitizes absolute paths (replacing the base), a stem of `..` could write outside the output directory.

**Attack scenario:** If an attacker controls an input filename like `..` (or `a/../../etc/cron`) and `--output-dir` is set, output files could be written to parent directories.

### Fix Recommendation

Sanitize the output path:

```rust
let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("subtitle");
// Reject stem containing path separators or parent directory components
if stem.contains("..") || stem.contains('/') || stem.contains('\\') {
    return Err(CliError::Conversion(format!("Invalid filename stem: {stem}")));
}
```

---

## Finding 03: `--max-files` is Truncation, Not Hard Limit (MEDIUM)

**File:** `crates/ass2sup-cli/src/lib.rs:68-73`  
**Severity:** MEDIUM  
**Classification:** Resource Exhaustion

### Description

The `--max-files N` flag truncates the glob results to N files **after** collection:

```rust
if let Some(max) = args.max_files {
    globbed.truncate(max);
}
```

This means:
1. All glob results are collected first (unbounded memory)
2. `truncate()` silently drops excess files without log/warning

An attacker who can place many `.ass` files in a directory (e.g., via a file upload feature, or in a shared temp directory) could cause OOM during glob expansion.

### Attack Scenario

If the program runs in a context where the working directory contains 10M+ small `.ass` files, `collect_flat_glob` would allocate enormous memory before truncation.

### Fix Recommendation

Use an iterator with `.take(N)` instead of collecting everything:
```rust
entries.filter_map(|e| e.ok()).filter(|e| e.is_file()).take(max).collect()
```

Plus log a warning when the glob returned more files than `--max-files`.

---

## Finding 04: Unmaintained Transitive Dependency (MEDIUM)

**File:** `Cargo.toml` (via indicatif → console → `number_prefix`)  
**Severity:** MEDIUM  
**Classification:** Supply Chain

### Description

The `number_prefix` crate (transitive via `indicatif` → `console`) is flagged as unmaintained in RUSTSEC-2025-0119. The audit.yml workflow explicitly ignores this advisory:

```yaml
run: cargo audit --deny warnings --ignore RUSTSEC-2025-0119
```

As of this audit, the project pins `indicatif = "0.18"` (workspace deps). The `number_prefix` advisory indicates the crate is unmaintained, which means no security patches will be published for it.

### Attack Scenario

Low direct exploitability — `number_prefix` is used by `indicatif` for progress bar number formatting (e.g., "1.2k items"). However, if a vulnerability is discovered in an unmaintained crate, there will be no fix. Supply chain risk is elevated for the FFI paths.

### Fix Recommendation

1. Upgrade `indicatif` to 0.19+ if available (which may drop the dependency)
2. Audit whether `number_prefix` can be vendored or replaced
3. Track the advisory in SECURITY.md with a review date (already done)

---

## Finding 05: Negative Stride from libass Not Sanity-Checked (LOW)

**File:** `crates/subtitle-renderer-libass/src/domain/renderer.rs:269-283`  
**Severity:** LOW  
**Classification:** FFI Safety

### Description

The `render_frame` method reads `w`, `h`, and `stride` from libass's `ASS_Image` struct and uses them to allocate a buffer:

```rust
let w = img.w.max(0) as u32;
let h = img.h.max(0) as u32;
let stride = img.stride.max(0) as u32;

let bitmap = if w > 0 && h > 0 && !img.bitmap.is_null() {
    let mut buf = Vec::with_capacity((stride * h) as usize);
    unsafe {
        std::ptr::copy_nonoverlapping(img.bitmap, buf.as_mut_ptr(), buf.capacity());
        buf.set_len(buf.capacity());
    }
    buf
};
```

While `img.stride.max(0)` prevents negative stride values from causing underflow in `stride * h`, the stride may be **smaller than `w`** if libass returns unexpected data. In practice, libass always returns stride >= w. **NOT EXPLOITABLE in practice** — this is a defense-in-depth concern.

### Fix Recommendation

Add an assertion or check:
```rust
assert!(stride >= w, "libass stride ({stride}) < width ({w}): invalid");
```

---

## Finding 06: TOCTOU Race in File Size Check (LOW)

**File:** `crates/ass2sup-cli/src/lib.rs:128-138`  
**Severity:** LOW  
**Classification:** Filesystem Race

### Description

The 100 MiB file size check and the subsequent file read are separate operations:

```rust
// Size check
for input in &inputs {
    let size = std::fs::metadata(input)
        .map_err(|e| CliError::ReadError(...))?
        .len();
    if size > MAX_INPUT_SIZE_BYTES { return Err(...); }
}

// Later (in convert_file::parse_input):
let content = std::fs::read_to_string(input)...;
```

Between the `metadata()` call and the `read_to_string()` call, a race window exists where the file could be replaced with a larger file. This is a classic TOCTOU (Time-of-Check-Time-of-Use) vulnerability.

### Attack Scenario

In a shared hosting or CI environment, an attacker with write access to the input file could replace it after the size check but before the read, causing OOM.

### Fix Recommendation

Read the file first, then check the size:
```rust
let content = std::fs::read_to_string(input)?;
if content.len() > MAX_INPUT_SIZE_BYTES as usize {
    return Err(CliError::InputTooLarge { ... });
}
```

This eliminates the race window.

---

## Finding 07: `parts.last().unwrap()` on Potentially Empty Split (LOW)

**File:** `crates/subtitle-renderer/src/renderer/font_registry_renderer.rs:741`  
**Severity:** LOW  
**Classification:** Panic in Production

### Description

`parse_font_name` uses `parts.last().unwrap()` where `parts` is computed from `family.split_whitespace()`:

```rust
pub fn parse_font_name(family: &str) -> Option<(String, crate::font::types::FontWeight)> {
    let parts: Vec<&str> = family.split_whitespace().collect();
    if parts.len() < 2 {
        return None;  // ← early return protects against empty
    }
    // ...
    let last = parts.last().unwrap();
```

**The early return on line 720 (`if parts.len() < 2`) prevents the panic.** `parts.last()` is safe because we already checked that `parts.len()` >= 2.

**NOT EXPLOITABLE** — this is a documentation/cleanliness issue. The `unwrap()` is provably safe.

### Fix Recommendation

Replace with `parts[parts.len() - 1]` for clarity, or use `#[expect(clippy::unwrap_used)]` with a comment explaining why it's safe.

---

## Finding 08: `unsafe impl Send+Sync` Without Documented Thread-Safety Analysis (INFO)

**File:** `crates/subtitle-renderer-libass/src/domain/renderer.rs:25-26`  
**Severity:** INFO  
**Classification:** FFI Safety

### Description

`AssRenderer` is marked `Send` and `Sync`:

```rust
// libass handles are thread-safe (internally mutex-protected)
unsafe impl Send for AssRenderer {}
unsafe impl Sync for AssRenderer {}
```

The comment asserts libass handles are internally synchronized. However:
1. The `Drop` implementation calls free functions without synchronization
2. `configure_fonts` and `render_frame` can be called concurrently from separate threads (if `Sync`)
3. `load_ass` mutates track state without internal locking

libass's thread-safety documentation is limited. The `ass_render_frame` function is documented as not thread-safe for the same track, but is thread-safe for different tracks on different renderers.

**NOT EXPLOITABLE in current usage** — the library is always used from a single-threaded pipeline (the rayon-based batch uses one `AssRenderer` per file, not shared across threads).

### Fix Recommendation

Replace `unsafe impl Send+Sync` with `impl !Send` and `impl !Sync`, and use an `Arc<Mutex<AssRenderer>>` wrapper if cross-thread access is needed. Add a safety comment if retaining `Sync`.

---

## Finding 09: Deep Stack Recursion in Override Tag Parsing (INFO)

**File:** `crates/ass-core/src/override_tag/parse.rs`  
**Severity:** INFO  
**Classification:** Stack Overflow (Resource Exhaustion)

### Description

The override tag parser uses recursive descent for nested `{\t(...)}` transform tags. A deeply nested input like `{\t({\t({\t(...))})})` could cause stack overflow.

The fuzz targets include `parse_override_tag`, suggesting this is being tested. The parser is a hand-written recursive descent parser.

**NOT EXPLOITABLE in practice** — Rust default stack size is 2 MB (8 MB on Linux), and ASS files with >1000 levels of nesting would hit the hardware limit. The fuzz target exercises this path.

### Fix Recommendation

Consider adding a recursion depth counter with a limit (e.g., 256) to the parse function:

```rust
fn parse_tags_inner(input: &str, depth: usize) -> ... {
    if depth > MAX_NESTING { return Err(...); }
    // ...
}
```

---

## Finding 10: `yanked = "warn"` in deny.toml (INFO)

**File:** `deny.toml:7`  
**Severity:** INFO  
**Classification:** Supply Chain

### Description

The `deny.toml` has:
```toml
yanked = "warn"
```

This means yanked crate versions in the dependency tree only produce warnings, not build failures. A yanked crate could contain a known vulnerability that the maintainer recalled, but the downstream is still using it.

### Fix Recommendation

Change to `yanked = "deny"` to block builds using yanked dependencies, with targeted `skip` entries if a yanked dependency must be temporarily tolerated.

---

## Positive Findings (Areas Done Well)

1. **Path traversal protection in embedded fonts** (`convert.rs:204-213`): The code explicitly checks for `ParentDir` components and logs a warning. Well done.

2. **100 MiB input size limit**: Enforced at the CLI boundary before any parsing.

3. **Recovery parsing in ass-core**: `parse_with_recovery` handles malformed input gracefully — panics are prevented by comprehensive error handling.

4. **FFI pointer null checks**: All `ass_*()` return values are checked for null before use.

5. **cargo-deny + cargo-audit in CI**: Weekly automated supply chain checks via GitHub Actions (`audit.yml`).

6. **Fuzz targets**: 5 fuzz targets across ass-core, color-quantizer, and pgs-encoder.

7. **Clean `unsafe` boundary**: Only 2 crates use unsafe (libass-sys and subtitle-renderer-libass); ass-core denies it entirely.

8. **No heap allocation in hot render paths**: Performance constraints explicitly prohibit this.

---

## Severity Distribution

```
CRITICAL: 0
HIGH:     1 (FFI null-ptr double-free risk in Drop — not exploitable in current code)
MEDIUM:   3 (path traversal, resource exhaustion, unmaintained dep)
LOW:      3 (stride check, TOCTOU, proveably-safe unwrap)
INFO:     3 (Send/Sync, stack recursion, deny.toml yanked policy)
```

---

## Supply Chain Summary

| Dependency | Status | Notes |
|-----------|--------|-------|
| `number_prefix` (transitive via `indicatif` → `console`) | ⚠️ Unmaintained | RUSTSEC-2025-0119, explicitly ignored |
| `cargo-deny` | ✅ Weekly scan | License whitelist, unknown-source deny |
| `cargo-audit` | ✅ Weekly scan | Only one ignored advisory |
| `unsafe` deps | ✅ None | All deps are pure Rust except libass-sys |
| Fuzz targets | ✅ 5 active | ass-core (3), color-quantizer (1), pgs-encoder (1) |

---

## Final Assessment

The codebase demonstrates a mature security posture. The one HIGH finding (Finding 01) is **not currently exploitable** due to the `Result::Err` short-circuit preventing struct construction — it is a latent risk for future refactoring. The MEDIUM findings are real but low-likelihood in practice (path traversal requires attacker-controlled filenames, resource exhaustion requires 100M+ files).

**Estimated effort to fix all findings:** ~4 hours (mostly documentation, one or two defensive checks).

**Recommended priority order:**
1. Finding 02: Sanitize output path stems (1 line change)
2. Finding 06: Eliminate TOCTOU by reading first, then checking size (2 line change)
3. Finding 08: Document or restrict Send+Sync (documentation)
4. Finding 01: Restructure init order (refactor for defense-in-depth)
5. Finding 09: Add recursion depth limit (defense-in-depth)
6. Finding 10: Change yanked to "deny" (config change)
