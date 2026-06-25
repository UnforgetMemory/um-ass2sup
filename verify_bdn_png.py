#!/usr/bin/env python3
import re
from PIL import Image
import os

ass_path = ".localref/The.Battleship.Island.2017.DC.BluRay.1080p.DTS-HD.MA5.1_zh_CN.ass"
bdn_dir = ".output/bdn_test/The.Battleship.Island.2017.DC.BluRay.1080p.DTS-HD.MA5.1_zh_CN"
bdn_xml_path = os.path.join(bdn_dir, "BDN.xml")

styles = {}
with open(ass_path, 'r', encoding='utf-8') as f:
    content = f.read()
    
style_pattern = re.compile(r'Style: ([^,]+),[^,]*,[^,]*,&H([0-9A-Fa-f]{8})')
for match in style_pattern.finditer(content):
    style_name = match.group(1)
    color_hex = match.group(2)
    aa = int(color_hex[0:2], 16)
    bb = int(color_hex[2:4], 16)
    gg = int(color_hex[4:6], 16)
    rr = int(color_hex[6:8], 16)
    alpha = 255 - aa
    rgba = (rr, gg, bb, alpha)
    styles[style_name] = {
        'primary_hex': color_hex,
        'expected_rgba': rgba
    }

first_events = {}
with open(ass_path, 'r', encoding='utf-8') as f:
    for line in f:
        if line.startswith('Dialogue:'):
            parts = line.split(',', 9)
            if len(parts) >= 10:
                style = parts[3].strip()
                if style not in first_events:
                    first_events[style] = line.strip()

with open(bdn_xml_path, 'r', encoding='utf-8') as f:
    xml_content = f.read()

event_pattern = re.compile(r'<Event InTC="([^"]+)" OutTC="([^"]+)" Forced="([^"]+)"><Graphic File="([^"]+)" Area="([^"]+)"></Event>')
events = event_pattern.findall(xml_content)

ass_events = []
with open(ass_path, 'r', encoding='utf-8') as f:
    for line in f:
        if line.startswith('Dialogue:'):
            parts = line.split(',', 9)
            if len(parts) >= 10:
                style = parts[3].strip()
                ass_events.append(style)

style_png_map = {}
style_event_idx = {}
for idx, style in enumerate(ass_events, 1):
    if style not in style_event_idx:
        style_event_idx[style] = idx
        png_file = f"{idx:04d}.png"
        style_png_map[style] = png_file

target_styles = ['OP_1', 'OP_2', 'OP_3', 'Note_1', 'Note_2', 'Note_3', 'Note_4', 'ed_1', 'Default', 'location_1']

print("style_name      png        expected_RGBA             actual_RGBA               match?   alpha_correct?  notes")
print("-" * 160)

for style in target_styles:
    if style not in style_png_map:
        print(f"{style:<15} {'N/A':<10} {'N/A':<25} {'N/A':<25} {'N/A':<9} {'N/A':<14} NO PNG MAPPING")
        continue
    
    png_file = style_png_map[style]
    png_path = os.path.join(bdn_dir, png_file)
    
    if not os.path.exists(png_path):
        print(f"{style:<15} {png_file:<10} {'N/A':<25} {'N/A':<25} {'N/A':<9} {'N/A':<14} PNG NOT FOUND")
        continue
    
    img = Image.open(png_path)
    if img.mode != 'RGBA':
        img = img.convert('RGBA')
    
    pixels = list(img.getdata())
    total_pixels = len(pixels)
    transparent_pixels = sum(1 for p in pixels if p[3] == 0)
    opaque_pixels = total_pixels - transparent_pixels
    
    non_transparent = [p for p in pixels if p[3] > 0]
    unique_colors = {}
    for p in non_transparent:
        rgba = tuple(p)
        unique_colors[rgba] = unique_colors.get(rgba, 0) + 1
    
    expected_rgba = styles.get(style, {}).get('expected_rgba', None)
    
    if expected_rgba and unique_colors:
        if expected_rgba[:3] == (0, 0, 0):
            text_colors = dict(unique_colors)
        else:
            text_colors = {c: cnt for c, cnt in unique_colors.items() if c != (0, 0, 0, 255)}
        
        if text_colors:
            actual_rgba = max(text_colors.items(), key=lambda x: x[1])[0]
        else:
            actual_rgba = max(unique_colors.items(), key=lambda x: x[1])[0]
        
        match = (expected_rgba[0] == actual_rgba[0] and 
                 expected_rgba[1] == actual_rgba[1] and 
                 expected_rgba[2] == actual_rgba[2])
        alpha_correct = (expected_rgba[3] == actual_rgba[3])
        
        color_count = unique_colors.get(expected_rgba, 0)
        color_pct = 100 * color_count / len(non_transparent) if non_transparent else 0
        
        notes = []
        if not match:
            notes.append(f"COLOR MISMATCH: expected {expected_rgba}, got {actual_rgba}")
        if color_pct < 5 and expected_rgba != (0, 0, 0, 255):
            notes.append(f"expected color only {color_pct:.1f}% of opaque pixels")
        if transparent_pixels == 0:
            notes.append("NO TRANSPARENCY")
        
        note_str = "; ".join(notes) if notes else "OK"
        print(f"{style:<15} {png_file:<10} {str(expected_rgba):<25} {str(actual_rgba):<25} {str(match):<9} {str(alpha_correct):<14} {note_str}")
    else:
        print(f"{style:<15} {png_file:<10} {str(expected_rgba):<25} {'N/A':<25} {'N/A':<9} {'N/A':<14} NO DATA")

print("\n=== SUMMARY ===")
mismatches = []
for style in target_styles:
    if style not in style_png_map:
        continue
    png_file = style_png_map[style]
    png_path = os.path.join(bdn_dir, png_file)
    if not os.path.exists(png_path):
        continue
    
    img = Image.open(png_path)
    if img.mode != 'RGBA':
        img = img.convert('RGBA')
    pixels = list(img.getdata())
    non_transparent = [p for p in pixels if p[3] > 0]
    unique_colors = {}
    for p in non_transparent:
        unique_colors[tuple(p)] = unique_colors.get(tuple(p), 0) + 1
    
    expected_rgba = styles.get(style, {}).get('expected_rgba', None)
    if expected_rgba and unique_colors:
        if expected_rgba[:3] == (0, 0, 0):
            text_colors = dict(unique_colors)
        else:
            text_colors = {c: cnt for c, cnt in unique_colors.items() if c != (0, 0, 0, 255)}
        if text_colors:
            actual_rgba = max(text_colors.items(), key=lambda x: x[1])[0]
        else:
            actual_rgba = max(unique_colors.items(), key=lambda x: x[1])[0]
        match = (expected_rgba[:3] == actual_rgba[:3])
        if not match:
            mismatches.append((style, expected_rgba, actual_rgba))

if mismatches:
    print(f"COLOR MISMATCHES ({len(mismatches)}):")
    for style, expected, actual in mismatches:
        print(f"  {style}: expected {expected}, actual {actual}")
else:
    print("All primary text colors match expected values.")
