# 📦 PGS Encoder Design

> **Domain-Driven Design architecture for Blu-ray PGS/SUP subtitle encoding**

---

## 📋 Table of Contents

- [Overview](#overview)
- [DDD Architecture: domain/ + encoding/](#ddd-architecture-domain--encoding)
- [Segment Types](#segment-types)
- [Display Sets](#display-sets)
- [Epoch Management](#epoch-management)
- [RLE Encoding](#rle-encoding)
- [PotPlayer Compatibility](#potplayer-compatibility)
- [Fade Optimization](#fade-optimization)
- [Timing & Frame Rates](#timing--frame-rates)
- [PgsEncoder API](#pgsencoder-api)

---

## Overview

The PGS encoder (`pgs-encoder` crate) converts quantized RGBA frames into the Blu-ray PGS (Presentation Graphic Stream) format — a binary subtitle stream consumed by Blu-ray players. It is the final stage of the `um-ass2sup` pipeline, consuming output from `color-quantizer` and producing `.sup` files or BDN XML descriptors.

The encoder follows **Domain-Driven Design (DDD)** principles, separating pure domain models from encoding concerns. Wave 1 of this refactoring is complete.

---

## DDD Architecture: domain/ + encoding/

```
crates/pgs-encoder/src/
  domain/                         # Pure domain model — no I/O, no encoding knowledge
    composition.rs                # CompositionState, ObjectComposition, WindowDef
    epoch.rs                      # EpochManager — object versioning, epoch lifecycle
    palette.rs                    # PaletteEntry, YCbCr conversion, color swap
    segment.rs                    # Segment, SegmentPayload (PCS/WDS/PDS/ODS/END), SupFile
    rle.rs                        # RLE encode, chunk_rle_data
    timing.rs                     # Frame rate codes, ms_to_90khz conversion
    mod.rs                        # Re-exports
  encoding/                       # Encoding—how domain objects serialize to binary
    display_set.rs                # DisplaySet builder: EpochStart/NormalCase/EpochContinue/PaletteOnly
    encoder.rs                    # PgsEncoder — frame → display set pipeline
    sup.rs                        # SUP file writer
    mod.rs                        # Re-exports
  color.rs                        # Color type re-exports
  encoder.rs                      # Legacy encoder (partial)
  epoch.rs                        # Legacy epoch (partial)
  lib.rs                          # Crate root
  rle.rs                          # Legacy RLE (partial)
  types.rs                        # Legacy type aliases
```

### Domain Layer (`domain/`)

The domain layer contains **pure models** with zero I/O and zero encoding knowledge. These types represent PGS concepts in a type-safe, testable way:

| Module | Key Types | Purpose |
|---|---|---|
| `composition.rs` | `CompositionState`, `ObjectComposition`, `WindowDef` | PCS state machine and object composition data |
| `epoch.rs` | `EpochManager` | Epoch lifecycle tracking, object versioning, palette/RLE hash comparison |
| `palette.rs` | `PaletteEntry` | Color palette entry with YCbCr conversion |
| `segment.rs` | `Segment`, `SegmentPayload`, `SegmentType`, `SupFile` | PGS segment types and SUP container |
| `rle.rs` | `chunk_rle_data` | Run-length encoding for image data |
| `timing.rs` | `frame_rate_code`, `is_ntsc_fps` | Timing constants and frame rate utilities |

### Encoding Layer (`encoding/`)

The encoding layer handles how domain objects are **serialized to binary**:

| Module | Key Types | Purpose |
|---|---|---|
| `display_set.rs` | `DisplaySetConfig`, `DisplaySetKind` | Display set construction (all variants) |
| `encoder.rs` | `PgsEncoder` | Frame → display set pipeline |
| `sup.rs` | `SupWriter` | SUP file binary writer |

---

## Segment Types

PGS segments are the atomic units of a Blu-ray subtitle stream. Each has a 13-byte header:

```
[PTS (4)] [DTS (4)] [Type (1)] [Length (2)] [Payload (variable)]
```

| Segment Type | Code | Purpose |
|---|---|---|
| **PCS** (Presentation Composition Segment) | `0x16` | Composition state, object references, window positions |
| **WDS** (Window Definition Segment) | `0x17` | Window dimensions (x, y, width, height) |
| **PDS** (Palette Definition Segment) | `0x14` | Color palette entries (YCbCr + alpha) |
| **ODS** (Object Definition Segment) | `0x15` | Image data (RLE-compressed bitmap) |
| **END** (End of Display Set) | `0x80` | Terminator — no payload |

### PCS Payload

```rust
pub struct PcsPayload {
    pub width: u16,
    pub height: u16,
    pub frame_rate: u8,
    pub composition_number: u16,
    pub composition_state: CompositionState,
    pub palette_update: bool,
    pub palette_id: u8,
    pub objects: Vec<ObjectComposition>,
}
```

### Composition States

| State | Value | Meaning |
|---|---|---|
| `NormalCase` | `0x00` | Objects within epoch may have changed |
| `AcquirePoint` | `0x40` | Decoder may start decoding from this point |
| `EpochStart` | `0x80` | New epoch — decoder must flush previous state |
| `EpochContinue` | `0xC0` | Same epoch, composition state unchanged |

---

## Display Sets

A **display set** is a group of PGS segments that together represent one subtitle frame. The encoder builds display sets in four varieties, managed by the `EpochManager`.

### Display Set Kinds

| Kind | Description | PCS State | Palette Update | ODS |
|---|---|---|---|---|
| **EpochStart** | New epoch — decoder starts fresh | `EpochStart` | yes | full RLE data |
| **NormalCase** | Regular frame within an epoch | `NormalCase` | if palette changed | full RLE data |
| **EpochContinue** | Identical to previous frame | `EpochContinue` | no | none (reuses ODS) |
| **PaletteOnly** | Only palette changed (e.g., fade) | `NormalCase` | yes | none (reuses ODS) |

### Display Set Structure

```
EpochStart / NormalCase:
  PCS (composition header + object refs)
  WDS (window definition)
  PDS (palette entries)
  ODS (RLE-encoded bitmap)
  END

EpochContinue:
  PCS (composition header, references existing objects)
  END

PaletteOnly:
  PCS (composition header, palette_update=true)
  PDS (new palette only)
  END
```

### Multi-Window Display Sets

For frames with large RLE data that exceeds half the PGS decode buffer (2 MB), the encoder automatically splits into multiple windows via `build_multi_window_display_set()`. This avoids decoder buffer overflow.

---

## Epoch Management

The `EpochManager` tracks the lifecycle of a continuous subtitle presentation:

```
EpochManager
  ├── frame_count: u32
  ├── object_version: u16
  ├── max_frames: u32
  ├── prev_palette_hash: Option<u64>
  └── prev_rle_hash: Option<u64>
```

### Epoch Lifecycle

```
EpochStart (first frame of a subtitle event)
  │
  ▼
NormalCase (subsequent frames with same object content)
  │
  ├── PaletteOnly (when only colors change, e.g. fade-in)
  │
  ├── EpochContinue (when frame is identical to previous)
  │
  └── EpochStart (when content changes significantly)
```

The manager decides which display set kind to emit via `decide_kind(palette_hash, rle_hash)`:

| Previous Hash | New Hash | Decision |
|---|---|---|
| — | — | `EpochStart` |
| same palette | same RLE | `EpochContinue` |
| different palette | same RLE | `PaletteOnly` |
| different palette | different RLE | `NormalCase` |

---

## RLE Encoding

The PGS specification requires run-length encoding for bitmap data. The encoder implements multiple strategies:

### `chunk_rle_data`

Splits RLE data into chunks of maximum `MAX_DECODE_BUFFER` (2 MB) to ensure decoder compatibility.

### RLE Encoding Modes

- **Single-segment**: Best for small bitmaps (most cases)
- **Multi-segment**: Large bitmaps split across multiple ODS segments

```rust
// RLE encoding in domain/rle.rs
pub fn encode_rle(data: &[u8], width: u16, palette_size: u16) -> Vec<u8>;
pub fn chunk_rle_data(rle_data: &[u8]) -> Vec<(u32, Vec<u8>)>;
```

### Performance

| Size | Median Time |
|---|---|
| 64×32 (small) | 2.84 µs |
| 1920×1080 (full frame) | 2.45 ms |

---

## PotPlayer Compatibility

The encoder includes specific workarounds for **PotPlayer**, a popular media player for Windows that is stricter than the PGS specification.

### `MAX_OBJECT_REFS = 2`

PotPlayer crashes on PCS segments containing more than 2 object references. The encoder enforces this limit:

```rust
pub const MAX_OBJECT_REFS: usize = 2;
```

When a display set would exceed this limit, `build_multi_window_display_set()` splits the objects across multiple display sets (chunks of 2).

### `palette_update = true` Required

PotPlayer requires `palette_update = true` on **all** PCS segments. The encoder defaults to this behavior. Omitting it causes palette rendering issues in PotPlayer.

### `num_objects = 0` Crash

Setting `num_objects = 0` in a palette-clear PCS causes PotPlayer to crash. The encoder always includes at least one object reference.

### Fade Alpha Handling

Show PCS for fade events must use alpha=0 palette (via `encode_multi_object_display_set_with_alpha(Some(0))`) to prevent a 1-frame full-alpha flash.

---

## Fade Optimization

Fade-in and fade-out events receive special treatment:

### PDS-Only Fade

Instead of re-encoding the full bitmap (ODS) for every fade frame, the encoder emits only a **new palette** (PDS) with adjusted alpha values:

```
┌─ Normal frame ─┐     ┌─ Fade frame 1 ─┐     ┌─ Fade frame 2 ─┐
│ PCS (updated)  │     │ PCS (updated)  │     │ PCS (updated)  │
│ PDS (new)      │     │ PDS (alpha 50%)│     │ PDS (alpha 25%)│
│ ODS (RLE data) │     │ END            │     │ END            │
│ END            │     │                │     │                │
└────────────────┘     └────────────────┘     └────────────────┘
```

This dramatically reduces SUP file size for fade transitions since the ODS (typically >90% of the segment size) is omitted.

### Implementation

The `EpochManager` detects palette-only changes and routes to `build_palette_only_display_set()`:

```rust
DisplaySetKind::PaletteOnly => {
    // Same RLE content, only palette changed (e.g., fade)
    build_palette_only_display_set(cfg, frame, pts, dts, palette_update, &palette_entries, fc)
}
```

---

## Timing & Frame Rates

### 90 kHz Clock

PGS uses a 90 kHz timestamp base. The encoder provides two conversion paths:

#### ms-based (legacy)

```rust
pub fn ms_to_90khz(&self, ms: u64) -> u64 {
    if is_ntsc_fps(self.fps) {
        (u128::from(ms) * 90000 * 1001 / 1000000) as u64
    } else {
        ms * 90
    }
}
```

#### Frame-index-based (preferred)

```rust
pub fn pts_at_frame(&self, first_pts: u64, frame_idx: u64) -> u64 {
    if is_ntsc_fps(self.fps) {
        // 23.976 (= 24000/1001) fps → 3753.75 ticks/frame = 15015/4
        first_pts + frame_idx * 15015 / 4
    } else {
        let ticks = (90000.0 / self.fps) as u64;
        first_pts + frame_idx * ticks
    }
}
```

The frame-index-based path avoids sub-frame drift from the ms → 90 kHz double conversion.

### NTSC 1001/1000 Factor

For NTSC frame rates (23.976, 29.97, 59.94), the encoder applies the 1001/1000 factor to align PTS values with the actual display duration.

### Frame Rate Codes

| FPS | Code |
|---|---|
| 23.976 | `0x10` |
| 24 | `0x20` |
| 25 | `0x30` |
| 29.97 | `0x40` |
| 50 | `0x60` |
| 59.94 | `0x70` |

---

## PgsEncoder API

### Construction

```rust
let encoder = PgsEncoder::new(1920, 1080, 23.976);
```

### Encoding Frames

```rust
// ms-based PTS (simpler, may have sub-frame drift)
let segments = encoder.encode_frame(&quantized_frame, pts_ms, duration_ms);

// Frame-index-based PTS (precise, recommended)
let segments = encoder.encode_frame_at_pts(&quantized_frame, pts, duration_ms);

// Direct to bytes
let bytes = encoder.encode_frame_to_bytes(&quantized_frame, pts_ms, duration_ms);
```

### Clear Screen

```rust
// Emit palette-clear display set at event boundary
let clear_segments = encoder.emit_clear(pts);
```

### Composition Number

`composition_number` increments after every `encode_frame` (wrapping_add), including NormalCase. This ensures each display set has a unique composition number as required by the PGS specification.

---

## Continue Reading

- [🏛️ Architecture](architecture.md) — Where the PGS encoder fits in the pipeline
- [🎯 Color Quantizer](color-quantizer.md) — How frames are quantized before encoding
- [🎨 Rendering Backends](rendering-backends.md) — How rendered frames are produced
