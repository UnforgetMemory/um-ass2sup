//! Encode a single-frame PGS subtitle, then decode and verify roundtrip.
//!
//! Run with: `cargo run -p pgs-encoder --example encode_sup`

use color_quantizer::{DitherMethod, Quantizer, Rgba};
use pgs_encoder::{decode_sup, verify_roundtrip, PgsEncoder};

fn main() {
    // Synthetic 2x2 magenta image.
    let width = 2u32;
    let height = 2u32;
    let magenta = [255, 0, 255, 255];
    let rgba: Vec<u8> = magenta.iter().cycle().take(16).copied().collect();

    // 1) Quantize to a 1-color palette.
    let quantizer = Quantizer::new(1).with_dither(DitherMethod::None);
    let frame = quantizer.quantize(&rgba, width, height);

    println!(
        "Quantized to {} colors, {} indexed pixels",
        frame.palette.len(),
        frame.indices.len()
    );

    // 2) Encode to PGS byte stream (1920x1080 display, 23.976 fps).
    let mut encoder = PgsEncoder::new(1920, 1080, 23.976);
    let sup_bytes = encoder.encode_frame_to_bytes(&frame, 0, 2000);
    println!("Encoded {} bytes of PGS data", sup_bytes.len());

    // 3) Decode it back.
    let display_sets = decode_sup(&sup_bytes).expect("decode should succeed");
    println!("Decoded {} display set(s)", display_sets.len());

    // 4) Verify lossless roundtrip.
    verify_roundtrip(&sup_bytes).expect("roundtrip should pass");
    println!("Roundtrip verified OK");

    // Sample first palette entry.
    if let Some(Rgba { r, g, b, a }) = frame.palette.first() {
        println!("First palette color: #{r:02X}{g:02X}{b:02X} alpha={a}");
    }
}
