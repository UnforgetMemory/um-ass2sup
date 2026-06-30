# Changelog

## [0.1.0] — 2026-06-29

### 🆕 New Project: ass2sup-libass

Complete libass-based ASS→SUP/PGS converter with DDD multi-module architecture.

#### Crates

- **libass-sys** — Manual `#[repr(C)]` FFI bindings for libass 0.17 (no bindgen).
  Safe wrapper with correct Drop ordering: `free_track → renderer_done → library_done`.

- **ass2sup-core** — DDD domain core with 4 domain modules + 2 infra modules:
  - `renderer` — libass lifecycle (init, track loading, font config, frame rendering)
  - `composer` — `ASS_Image*` linked list → RGBA compositing (Porter-Duff over)
  - `timeline` — frame timestamp generation (per-frame for animated events)
  - `pipeline` — end-to-end orchestration with smart duplicate detection
  - `pgs_adapter` — frame-accurate PTS, gap-detection clears, BT.709 color space
  - `vendor` — vendored RGBA helpers (composite_over, crop_to_tight_bbox)

- **ass2sup-cli** — CLI binary via clap, SUP and BDN XML output modes.

#### Key Features

- libass + fontconfig for accurate font resolution (no custom font pipeline)
- Smart frame rendering: event-boundary timestamps + per-frame for animated events
- Duplicate frame detection via hash of indices+palette+position
- Frame-accurate PTS for NTSC rates (23.976, 29.97)
- Gap detection with clear segments between non-contiguous frame groups
- Final clear at end of stream
- BT.709 color space for HD content (PGS compliance)
- Palette reuse via `quantize_with_prev`
- Overlapping event range handling via BTreeSet dedup

#### CLI Options

- `--font` — default font family
- `--font-dir` — additional font directory
- `--font-fallback-map` — per-style font override (rewrites Fontname in ASS)
- `--check-fonts` — pre-render font availability check via fc-match
- `--to-bdn` — BDN XML + PNG output mode
- `--fps`, `--resolution`, `--max-colors`, `--dither` — conversion parameters

#### Performance

- Battleship Island (2.5h, 1988 events): ~116s, ~52 MB SUP, 3194 unique frames
- 15 unit tests + 1 FFI smoke test (all passing)
- clippy -D warnings clean

#### Reused Crates (path dependencies)

- `color-quantizer` — RGBA→indexed palette with k-d tree, 3 dither methods
- `pgs-encoder` — PGS segment assembly (PCS/WDS/PDS/ODS/END, epoch management)
- `bdn-xml` — BDN XML + PNG output

#### Bug Fixes During Development

- **ASS_Event struct layout**: Missing MarginL/R/V fields caused segfault. Fixed by
  adding all 12 fields matching libass 0.17 ABI (80 bytes).
- **ASS_Event field order**: `name` pointer before `MarginL/R/V` in C struct.
- **ass_set_fonts_dir ordering**: MUST be called before `ass_set_fonts`.
- **ASS_Image.color format**: 0xRRGGBBAA (not 0xAABBGGRR). Fixed compositor.
- **Fade color_alpha**: AA byte used for per-image opacity. Compositor now
  combines with per-pixel bitmap alpha.
