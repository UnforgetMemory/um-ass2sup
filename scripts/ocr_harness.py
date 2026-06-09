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
            elif isinstance(page, list):
                for res in page:
                    if res is not None and len(res) >= 2:
                        box = res[0] if len(res) > 0 else []
                        if hasattr(box, 'tolist'):
                            box = box.tolist()
                        text = res[1] if len(res) > 1 else ""
                        score = res[2] if len(res) > 2 else 1.0
                        lines.append([box, str(text), float(score)])

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