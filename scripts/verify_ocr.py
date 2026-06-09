#!/usr/bin/env python3
import sys
import subprocess
import json
import os
from PIL import Image, ImageDraw, ImageFont

def render_text_png(text, output_path, font_size=64):
    width, height = 1920, 1080
    img = Image.new('RGBA', (width, height), (0, 0, 0, 255))
    draw = ImageDraw.Draw(img)
    font_paths = [
        '/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf',
        '/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf',
        '/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf',
        '/usr/share/fonts/truetype/notosans/NotoSans-Regular.ttf',
    ]
    font = None
    for fp in font_paths:
        if os.path.exists(fp):
            try:
                font = ImageFont.truetype(fp, font_size)
                print(f"Using font: {fp}")
                break
            except Exception:
                pass
    if font is None:
        font = ImageFont.load_default()
    bbox = draw.textbbox((0, 0), text, font=font)
    text_w = bbox[2] - bbox[0]
    text_h = bbox[3] - bbox[1]
    x = (width - text_w) // 2
    y = (height - text_h) // 2
    draw.text((x, y), text, fill=(255, 255, 255, 255), font=font)
    img.save(output_path, 'PNG')
    print(f"Saved: {output_path} ({width}x{height})")
    return output_path

def run_ocr_harness(png_path):
    python_path = os.path.expanduser('~/paddle_env/bin/python3')
    script_path = os.path.abspath(os.path.join(os.path.dirname(__file__), 'ocr_harness.py'))
    cmd = [python_path, script_path, png_path]
    result = subprocess.run(cmd, capture_output=True, text=True, timeout=60)
    return result.stdout, result.stderr, result.returncode

def main():
    test_texts = [
        "Hello World",
        "Second line of subtitles",
        "Final subtitle line",
    ]

    print("=== OCR Pipeline Verification ===\n")

    for i, text in enumerate(test_texts):
        png_path = f'/tmp/ocr_verify_{i}.png'
        print(f"--- Test {i+1}: '{text}' ---")
        render_text_png(text, png_path)
        stdout, stderr, code = run_ocr_harness(png_path)
        print(f"Exit code: {code}")
        if stdout.strip():
            try:
                results = json.loads(stdout)
                for r in results:
                    box, rec_text, score = r
                    print(f"  OCR: '{rec_text}' (score={score:.3f})")
            except Exception as e:
                print(f"  Parse error: {e}")
                print(f"  Raw output: {stdout[:200]}")
        if stderr:
            print(f"  Stderr: {stderr[:200]}")
        print()

if __name__ == '__main__':
    main()
