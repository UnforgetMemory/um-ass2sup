//! Quantize an RGBA image to a fixed palette.
//!
//! Run with: `cargo run -p color-quantizer --example quantize_image`

use color_quantizer::{DitherMethod, Quantizer, Rgba};

fn main() {
    // Synthetic 4x4 RGBA gradient (64 bytes: 16 pixels * 4 channels).
    let width = 4u32;
    let height = 4u32;
    let mut rgba = Vec::with_capacity(64);
    for y in 0..height {
        for x in 0..width {
            rgba.push((x * 64) as u8); // R
            rgba.push((y * 64) as u8); // G
            rgba.push(128); // B
            rgba.push(255); // A
        }
    }

    // Build a quantizer: 4-color palette, Floyd-Steinberg dithering.
    let quantizer = Quantizer::new(4).with_dither(DitherMethod::FloydSteinberg);

    let q = quantizer.quantize(&rgba, width, height);

    println!("Palette ({} colors):", q.palette.len());
    for (i, c) in q.palette.iter().enumerate() {
        let Rgba { r, g, b, a } = c;
        println!("  [{i}] #{r:02X}{g:02X}{b:02X} alpha={a}");
    }

    // Indexed pixels: one byte per pixel = index into `q.palette`.
    println!("\nIndexed ({} bytes):", q.indices.len());
    for row in q.indices.chunks(width as usize) {
        print!("  ");
        for &idx in row {
            print!("{idx} ");
        }
        println!();
    }
}
