#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // The fuzzer sends arbitrary bytes. We interpret them as RGBA pixels.
    //
    // Strategy: use the first byte for max_colors, then interpret the
    // remaining bytes as RGBA pixel data capped at 1024 pixels. We set
    // width = total_pixels, height = 1 so that rgba.len() == width *
    // height * 4 holds (avoiding the quantizer's assertion panic on
    // size mismatch).
    if data.is_empty() {
        return;
    }
    let max_colors = (data[0] as usize).clamp(1, 255);
    let pixel_data = &data[1..];
    let total_pixels = (pixel_data.len() / 4).min(1024);
    let rgba = &pixel_data[..total_pixels * 4];
    let width = total_pixels as u32;
    let height = 1u32;
    let q = color_quantizer::Quantizer::new(max_colors);
    q.quantize(rgba, width, height);
});
