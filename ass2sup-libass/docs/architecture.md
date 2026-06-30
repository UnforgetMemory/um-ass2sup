# Architecture — ass2sup-libass

## Overview

```
┌─────────────────────────────────────────────────────────────┐
│                        ass2sup-cli                          │
│  (clap CLI parser, tracing init, calls Ass2Sup::convert_*)  │
└─────────────────┬───────────────────────────────────────────┘
                  │
                  ▼
┌─────────────────────────────────────────────────────────────┐
│                       ass2sup-core                          │
│                                                             │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                    Domain Layer                        │  │
│  │                                                       │  │
│  │  ┌──────────┐  ┌──────────┐  ┌──────────┐           │  │
│  │  │renderer  │  │composer  │  │timeline  │           │  │
│  │  │ libass   │  │ASS_Image │  │timestamp │           │  │
│  │  │ lifecycle │  │→ RGBA   │  │generation│           │  │
│  │  └────┬─────┘  └────┬─────┘  └──────────┘           │  │
│  │       │              │                               │  │
│  │       ▼              ▼                               │  │
│  │  ┌────────────────────────────────────────────────┐  │  │
│  │  │                pipeline Ass2Sup                  │  │  │
│  │  │   process_events: render→compose→crop→quantize  │  │  │
│  │  └──────────────────────┬─────────────────────────┘  │  │
│  └─────────────────────────┼───────────────────────────┘  │
│                            │                              │
│  ┌─────────────────────────┼───────────────────────────┐  │
│  │                Infrastructure Layer                  │  │
│  │                         │                            │  │
│  │  ┌──────────────┐   ┌──▼──────────┐                  │  │
│  │  │vendor        │   │pgs_adapter  │                  │  │
│  │  │RGBA helpers  │   │quantize→    │                  │  │
│  │  │composite_over│   │encode_sup   │                  │  │
│  │  │crop_to_tight │   │encode_bdn   │                  │  │
│  │  └──────────────┘   └──────┬──────┘                  │  │
│  └────────────────────────────┼─────────────────────────┘  │
└───────────────────────────────┼───────────────────────────┘
                                │
                    ┌───────────┴───────────┐
                    │                       │
                    ▼                       ▼
           ┌──────────────┐      ┌──────────────────┐
           │color-quantizer│      │  pgs-encoder     │
           │RGBA→indexed   │      │  PCS/WDS/PDS/ODS │
           │k-d tree       │      │  Epoch management│
           │3 dither modes │      │  NTSC timing     │
           └───────────────┘      └────────┬─────────┘
                                           │
                                           ▼
                                        .sup
```

## Data flow per frame

```
ass_render_frame(track, timestamp_ms)
        │
        ▼
  ASS_Image* (shadow → outline → character)
        │
        ▼
  For each image:
    ┌──────────────────────────────┐
    │ pixel alpha = bitmap[y*stride+x]   │
    │ pixel_rgba = (color & 0xFFFFFF00)   │
    │             | alpha                  │
    │ Porter-Duff "over" at (dst_x,dst_y) │
    └──────────────────────────────┘
        │
        ▼
  RGBA frame (1920×1080)
        │
        ▼
  crop_to_tight_bbox → CroppedFrame(x,y,w,h)
        │
        ▼
  ColorPipeline::quantize() → QuantizedFrame
        │
        ▼
  PgsEncoder::encode_frame() → Vec<Segment>
        │
        ▼
  sup_to_bytes() → .sup file
```

## Duplicate detection (smart rendering)

The pipeline hashes each QuantizedFrame's palette + indices and skips encoding duplicates. Instead of encoding the same frame, it extends the previous frame's duration to cover the gap. This reduces output size significantly for static subtitles.

## Linking

The project links against `libass.so` using a **local copy** in `links/` directory:
- `links/libass.so.9.4.1` — runtime library
- `links/libass.so` — symlink for cargo linker to resolve

This approach avoids requiring the libass-dev package at build time.

## Key API ordering constraint

When configuring fonts, **`ass_set_fonts_dir()` MUST be called before `ass_set_fonts()`**. Failure to do so causes a segfault during font initialization on this system (Debian with fontconfig).

## Correct C struct layout

The `ASS_Event` struct in the FFI bindings must exactly match the C layout:

```c
// C struct (libass 0.17) — 80 bytes on x86_64
struct ASS_Event {
    long long Start;         //  0
    long long Duration;      //  8
    int ReadOrder;           // 16
    int Layer;               // 20
    int Style;               // 24
    char *Name;              // 32 (4-byte pad after Style)
    int MarginL;             // 40
    int MarginR;             // 44
    int MarginV;             // 48
    char *Effect;            // 56 (4-byte pad after MarginV)
    char *Text;              // 64
    void *render_priv;       // 72
}; // total = 80 bytes
```

The `name` pointer comes **before** `MarginL/R/V`. Getting this wrong causes segfaults when reading event text.
