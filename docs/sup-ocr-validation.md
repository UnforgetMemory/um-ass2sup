# SUP→OCR Validation Pipeline

The OCR validation pipeline verifies the encode→decode round-trip by rendering SUP frames to PNG and comparing OCR output against the original ASS text.

## Architecture

```
ASS file
  │
  ├─→ ass2sup (encode) → SUP binary
  │                            │
  │                            ├─→ decode_sup → DisplaySet[]
  │                            │                      │
  │                            │               decode_frame_to_rgba
  │                            │                      │
  │                            │               frame_to_png
  │                            │                      │
  │                            └──────────────────────┴─→ PNG (decoded)
  │                                                       │
  ├─→ render → quantize → PNG (encoded) ←─────────────────┘
  │                            │
  └─→ PaddleOCR ───────────────┴─→ OCR text → compare with ASS text
```

## Components

### `pgs-encoder/src/color.rs` — Color conversion

- `ycbcr_to_rgba(y, cb, cr, alpha)` — BT.601 full-range inverse
- `palette_to_rgba(entries)` — converts palette entries to RGBA array
- `swap(val, pivot)` — palette index 0↔pivot swap for transparent handling

### `pgs-encoder/src/decode_to_image.rs` — Frame decoder

- `RenderContext` — carries window/palette/object state across display sets
- `decode_frame_to_rgba(display_set, ctx, transparent_index)` — decodes one frame
- `frame_to_png(frame)` — encodes RGBA frame as PNG bytes
- `DecodeImageError` / `PngEncodeError` — error types

### `ass2sup-cli/src/ocr.rs` — OCR utilities

- `run_ocr(png_path)` — calls `scripts/ocr_harness.py`, returns `OcrResult`
- `parse_ocr_json(json_str)` — parses PaddleOCR JSON output
- `extract_text(ocr)` — concatenates all OCR text regions
- `strip_ass_tags(text)` — removes ASS override tags from text
- `normalized_similarity(a, b)` — Levenshtein-based similarity (0.0–1.0)
- `is_match(ocr, ass, threshold)` — similarity comparison with threshold

### `scripts/ocr_harness.py` — PaddleOCR wrapper

- Accepts PNG path as argument
- Returns JSON array of `[box, text, score]` per detected region
- Exit code 0 on success, 1 on error, 2 on PaddlePaddle not available, 3 on PIR/onednn infrastructure error
- Configurable via `OCR_HARNESS` environment variable

## Running E2E Tests

```bash
# 项目 venv 使用 uv 管理（Python 3.13）
source .venv313/bin/activate

# 确认 PaddlePaddle 可导入
python -c "import paddle; print(paddle.__version__)"

# 运行 E2E 测试（需要 --ignored 标志）
cargo test -p ass2sup-cli test_ocr_roundtrip -- --ignored --nocapture
```

## Similarity Threshold

- Default threshold: 0.80
- OCR accuracy for CJK text is typically 95–99%
- Threshold 0.70 accommodates minor OCR errors and punctuation differences
- Levenshtein is applied after lowercase + space removal

## Limitations

- PaddleOCR may miss punctuation or misinterpret English case
- Multi-line text detection depends on subtitle layout
- `#[ignore]` test requires `test_data/sample.ass` to exist
- Requires PaddleOCR installation (not available in standard CI)