fn main() {
    let sup_path = std::env::args()
        .nth(1)
        .expect("Usage: decode_sup_to_png <input.sup> <output_dir>");
    let out_dir = std::env::args()
        .nth(2)
        .expect("Usage: decode_sup_to_png <input.sup> <output_dir>");
    std::fs::create_dir_all(&out_dir).unwrap();

    let data = std::fs::read(&sup_path).expect("read sup");
    let display_sets = pgs_encoder::decode_sup(&data).expect("decode sup");
    println!("Decoded {} display sets", display_sets.len());

    let mut ctx = pgs_encoder::RenderContext::default();

    for (i, ds) in display_sets.iter().enumerate() {
        match pgs_encoder::decode_frame_to_rgba(ds, &mut ctx, 255) {
            Ok(frame) => {
                let png_bytes = pgs_encoder::frame_to_png(&frame).expect("encode png");
                let path = format!("{}/frame_{:03}.png", out_dir, i);
                std::fs::write(&path, &png_bytes).expect("write png");
                println!(
                    "frame {:03}: {}x{} ({} bytes) → {}",
                    i,
                    frame.width,
                    frame.height,
                    png_bytes.len(),
                    path
                );
            }
            Err(e) => {
                eprintln!("frame {:03}: decode error: {}", i, e);
            }
        }
    }
}
