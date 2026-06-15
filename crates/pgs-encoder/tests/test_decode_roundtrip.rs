//! Diagnostic roundtrip test: QuantizedFrame → encode → decode → RGBA check
//!
//! Creates a 1920×1080 QuantizedFrame with sparse opaque white glyph pixels
//! near the bottom of the frame (simulating a subtitle), then runs the full
//! encode→decode→composite pipeline and counts opaque pixels in the result.

use color_quantizer::{QuantizedFrame, Rgba};
use pgs_encoder::{decode_frame_to_rgba, decode_sup, PgsEncoder, RenderContext};

/// Create a 1920×1080 frame with mostly transparent pixels plus some
/// opaque white "glyph" pixels near the bottom.
fn make_sparse_glyph_frame() -> QuantizedFrame {
    let width = 1920u32;
    let height = 1080u32;
    let total = (width * height) as usize;
    // Palette: index 0 = transparent, index 1 = opaque white
    let palette = vec![
        Rgba::new(0, 0, 0, 0),         // index 0: transparent
        Rgba::new(255, 255, 255, 255), // index 1: opaque white
    ];

    let mut indices = vec![0u8; total]; // all transparent by default

    // Place "glyph" pixels: rows 1040-1049, columns 860-1059
    let mut glyph_count = 0usize;
    for row in 1040..1050 {
        for col in 860..1060 {
            let pos = row as usize * width as usize + col as usize;
            indices[pos] = 1;
            glyph_count += 1;
        }
    }
    // edge pixels
    for row in 1039..1051 {
        indices[row as usize * width as usize + 859] = 1;
        indices[row as usize * width as usize + 1060] = 1;
        glyph_count += 2;
    }

    println!("=== Test Frame ===");
    println!("Dimensions: {}x{}", width, height);
    println!("Total pixels: {}", total);
    println!(
        "Palette entries: {} (idx 0=transparent, idx 1=white)",
        palette.len()
    );
    println!("Opaque glyph pixels (index=1): {}/{}", glyph_count, total);

    QuantizedFrame {
        width,
        height,
        palette,
        indices,
        transparent_index: 0,
        x: 0,
        y: 0,
    }
}

fn make_bottom_row_frame() -> QuantizedFrame {
    let width = 1920u32;
    let height = 1080u32;
    let total = (width * height) as usize;
    let palette = vec![Rgba::new(0, 0, 0, 0), Rgba::new(255, 255, 255, 255)];

    let mut indices = vec![0u8; total];
    for col in 800..1120 {
        let pos = (height - 1) as usize * width as usize + col as usize;
        indices[pos] = 1;
    }
    let count = indices.iter().filter(|&&i| i == 1).count();
    println!("=== Bottom-Row Frame === opaque pixels in last row: {count}");
    QuantizedFrame {
        width,
        height,
        palette,
        indices,
        transparent_index: 0,
        x: 0,
        y: 0,
    }
}

fn make_bottom_row_plus1_frame() -> QuantizedFrame {
    let width = 1920u32;
    let height = 1080u32;
    let total = (width * height) as usize;
    let palette = vec![Rgba::new(0, 0, 0, 0), Rgba::new(255, 255, 255, 255)];

    let mut indices = vec![0u8; total];
    for col in 800..1120 {
        let pos = (height - 2) as usize * width as usize + col as usize;
        indices[pos] = 1;
    }
    let count = indices.iter().filter(|&&i| i == 1).count();
    println!("=== Second-to-Last Row Frame === opaque pixels in row 1078: {count}");
    QuantizedFrame {
        width,
        height,
        palette,
        indices,
        transparent_index: 0,
        x: 0,
        y: 0,
    }
}

fn run_roundtrip(frame: &QuantizedFrame, label: &str) {
    println!("\n========== ROUNDTRIP TEST: {label} ==========");

    // --- Encode ---
    let mut encoder = PgsEncoder::new(1920, 1080, 24.0);
    let sup_bytes = encoder.encode_frame_to_bytes(frame, 0, 5000);
    println!("SUP size: {} bytes", sup_bytes.len());

    // --- Decode SUP to DisplaySet ---
    let display_sets = decode_sup(&sup_bytes).expect("decode_sup should succeed");
    println!("Display sets: {}", display_sets.len());
    assert!(
        !display_sets.is_empty(),
        "should have at least one display set"
    );

    // Log segment details
    for (i, ds) in display_sets.iter().enumerate() {
        println!("  DisplaySet {i}: {} segments", ds.segments.len());
        for (j, seg) in ds.segments.iter().enumerate() {
            match &seg.payload {
                pgs_encoder::ParsedPayload::PresentationComposition {
                    width,
                    height,
                    palette_update,
                    palette_id,
                    objects,
                    ..
                } => {
                    println!(
                        "    Seg {j}: PCS {}x{} palette_update={} palette_id={} objects={}",
                        width,
                        height,
                        palette_update,
                        palette_id,
                        objects.len()
                    );
                }
                pgs_encoder::ParsedPayload::WindowDefinition { windows } => {
                    println!("    Seg {j}: WDS {} windows", windows.len());
                    for w in windows {
                        println!(
                            "      window_id={} x={} y={} {}x{}",
                            w.window_id, w.x, w.y, w.width, w.height
                        );
                    }
                }
                pgs_encoder::ParsedPayload::PaletteDefinition {
                    palette_id,
                    entries,
                    ..
                } => {
                    println!(
                        "    Seg {j}: PDS palette_id={} entries={}",
                        palette_id,
                        entries.len()
                    );
                    for entry in entries.iter().take(4) {
                        println!(
                            "      entry[{}]: Y={} Cb={} Cr={} alpha={}",
                            entry.index, entry.y, entry.cb, entry.cr, entry.alpha
                        );
                    }
                    if entries.len() > 4 {
                        println!("      ... {} more entries", entries.len() - 4);
                    }
                }
                pgs_encoder::ParsedPayload::ObjectDefinition {
                    object_id,
                    width,
                    height,
                    data,
                    ..
                } => {
                    println!(
                        "    Seg {j}: ODS object_id={} {}x{} rle_data_len={}",
                        object_id,
                        width,
                        height,
                        data.len()
                    );
                    println!(
                        "      First 16 RLE bytes: {:02x?}",
                        &data[..data.len().min(16)]
                    );
                }
                pgs_encoder::ParsedPayload::End => {
                    println!("    Seg {j}: END");
                }
            }
        }
    }

    // --- Decode DisplaySet to RGBA ---
    let mut ctx = RenderContext::default();
    let rgba = decode_frame_to_rgba(&display_sets[0], &mut ctx, 0u8)
        .expect("decode_frame_to_rgba should succeed");

    println!("\n=== Decoded RGBA ===");
    println!("Decoded dimensions: {}x{}", rgba.width, rgba.height);
    println!(
        "Decoded data len: {} bytes ({} pixels)",
        rgba.data.len(),
        rgba.data.len() / 4
    );

    let total_pixels = rgba.data.len() / 4;
    let transparent_pixels = rgba.data.chunks(4).filter(|p| p[3] == 0).count();
    let opaque_pixels = rgba.data.chunks(4).filter(|p| p[3] == 255).count();
    let semi_pixels = rgba
        .data
        .chunks(4)
        .filter(|p| p[3] > 0 && p[3] < 255)
        .count();
    let non_transparent = total_pixels - transparent_pixels;

    println!("Total pixels: {total_pixels}");
    println!("Transparent (alpha=0): {transparent_pixels}");
    println!("Opaque (alpha=255): {opaque_pixels}");
    println!("Semi-transparent (0<alpha<255): {semi_pixels}");
    println!("Non-transparent (alpha>0): {non_transparent}");

    // Show first few non-zero pixels
    let mut shown = 0;
    for (i, pixel) in rgba.data.chunks(4).enumerate() {
        if pixel[3] > 0 {
            println!(
                "  non-zero pixel at index {}: RGBA({},{},{},{}) [row={}, col={}]",
                i,
                pixel[0],
                pixel[1],
                pixel[2],
                pixel[3],
                i / 1920,
                i % 1920
            );
            shown += 1;
            if shown >= 5 {
                break;
            }
        }
    }

    if non_transparent == 0 {
        println!("\n*** FAIL: {label}: Decoded frame has ZERO non-transparent pixels! ***");
    } else {
        println!(
            "\n*** PASS: {label}: Decoded frame has {non_transparent} non-transparent pixels ***"
        );
    }
}

#[test]
fn test_roundtrip_sparse_glyph() {
    let frame = make_sparse_glyph_frame();
    run_roundtrip(&frame, "sparse_glyph");
}

#[test]
fn test_roundtrip_bottom_row() {
    let frame = make_bottom_row_frame();
    run_roundtrip(&frame, "bottom_row");
}

#[test]
fn test_roundtrip_bottom_row_plus1() {
    let frame = make_bottom_row_plus1_frame();
    run_roundtrip(&frame, "second_to_last_row");
}
