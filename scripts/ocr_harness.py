#!/usr/bin/env python3
"""PaddleOCR harness for OCR validation.

Receives a PNG path on the command line, runs PaddleOCR on it,
and prints the raw JSON result to stdout.

Usage:
    python3 scripts/ocr_harness.py /path/to/frame.png

Output:
    JSON array of [box, text, score] tuples, one per detected text region.
    Empty array if no text found.

Exit codes:
    0  - success (even if no text found)
    1  - PaddleOCR not installed or other error (test should fail)
    2  - PaddlePaddle not available (test should skip)
"""
from __future__ import annotations

import sys
import os


def main() -> int:
    if len(sys.argv) < 2:
        print("Usage: ocr_harness.py <image.png>", file=sys.stderr)
        return 1

    png_path = sys.argv[1]

    if not os.path.isfile(png_path):
        print(f"Error: file not found: {png_path}", file=sys.stderr)
        return 1

    # Preprocess: composite RGBA onto contrasting background, then auto-crop
    try:
        from PIL import Image
        import numpy as np
        img = Image.open(png_path)
        if img.mode == 'RGBA':
            # Determine background color based on text color
            arr = np.array(img)
            alpha = arr[:, :, 3]
            text_mask = alpha > 0
            if text_mask.any():
                text_pixels = arr[text_mask]
                avg_brightness = text_pixels[:, :3].mean()
                bg_color = (0, 0, 0) if avg_brightness > 128 else (255, 255, 255)
            else:
                bg_color = (0, 0, 0)
            bg = Image.new('RGB', img.size, bg_color)
            bg.paste(img, mask=img.split()[3])
            # Auto-crop to text bounding box
            arr_bg = np.array(bg)
            if bg_color == (0, 0, 0):
                non_bg = np.any(arr_bg > 0, axis=2)
            else:
                non_bg = np.any(arr_bg < 255, axis=2)
            rows = np.any(non_bg, axis=1)
            cols = np.any(non_bg, axis=0)
            if rows.any() and cols.any():
                y_min, y_max = np.where(rows)[0][[0, -1]]
                x_min, x_max = np.where(cols)[0][[0, -1]]
                pad = 20
                y_min = max(0, y_min - pad)
                y_max = min(bg.height - 1, y_max + pad)
                x_min = max(0, x_min - pad)
                x_max = min(bg.width - 1, x_max + pad)
                bg = bg.crop((x_min, y_min, x_max + 1, y_max + 1))
            import tempfile
            tmp = tempfile.NamedTemporaryFile(suffix='.png', delete=False)
            bg.save(tmp.name)
            png_path = tmp.name
    except ImportError:
        pass  # PIL/numpy not available, use original image

    try:
        from paddleocr import PaddleOCR
    except ImportError as e:
        print(f"Error: PaddleOCR not installed: {e}", file=sys.stderr)
        return 1

    try:
        import paddle
    except ImportError:
        print("Error: paddlepaddle core not installed. Run: pip install paddlepaddle", file=sys.stderr)
        return 2

    try:
        ocr = PaddleOCR(lang="ch", use_doc_orientation_classify=False)
        result = ocr.predict(str(png_path))
        lines = []
        if result and len(result) > 0:
            page = result[0]
            if isinstance(page, dict):
                rec_texts = page.get("rec_texts")
                if rec_texts is None:
                    rec_texts = []
                rec_scores = page.get("rec_scores")
                if rec_scores is None:
                    rec_scores = []
                rec_boxes = page.get("rec_boxes")
                if rec_boxes is None:
                    rec_boxes = page.get("dt_polys")
                if rec_boxes is None:
                    rec_boxes = []
                for i, text in enumerate(rec_texts):
                    box = rec_boxes[i] if i < len(rec_boxes) else []
                    if hasattr(box, 'tolist'):
                        box = box.tolist()
                    score = rec_scores[i] if i < len(rec_scores) else 1.0
                    lines.append([box, str(text), float(score)])
            else:
                # PaddleOCR v5+ returns OCRResult object; try subscript access
                try:
                    rec_texts = page["rec_texts"]
                    rec_scores = page.get("rec_scores", [])
                    rec_boxes = page.get("rec_boxes", page.get("dt_polys", []))
                    for i, text in enumerate(rec_texts):
                        box = rec_boxes[i] if i < len(rec_boxes) else []
                        if hasattr(box, 'tolist'):
                            box = box.tolist()
                        score = rec_scores[i] if i < len(rec_scores) else 1.0
                        lines.append([box, str(text), float(score)])
                except (KeyError, TypeError, IndexError):
                    pass

        import json
        print(json.dumps(lines, ensure_ascii=False))
        return 0

    except NotImplementedError as e:
        # PaddlePaddle PIR/onednn infrastructure error — not our code
        err_msg = str(e)
        if "ConvertPirAttribute2RuntimeAttribute" in err_msg or "pir::ArrayAttribute" in err_msg:
            print(f"Error: PaddlePaddle PIR infrastructure error (NotImplementedError). "
                  f"This is a known issue on some Linux environments with PaddlePaddle 3.x. "
                  f"Details: {err_msg[:120]}", file=sys.stderr)
            return 3  # Special code: infrastructure skip
        # Other NotImplementedErrors treated as harness failure (exit 1)
        print(f"NotImplementedError: {e}", file=sys.stderr)
        return 1
    except ImportError as e:
        print(f"ImportError: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())