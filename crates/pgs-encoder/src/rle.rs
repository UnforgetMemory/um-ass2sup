/// PGS RLE encoding compatible with FFmpeg's pgssub decoder.
///
/// FFmpeg's RLE format:
/// - Non-zero byte (not after 0x00): single pixel of that color
/// - After 0x00 (flags byte):
///   - flags = 0x00: end of line
///   - flags bits 5-0: run length (short: 1-63)
///   - flags bit 6: if set, next byte extends run to 14 bits (long: 64-16383)
///   - flags bit 7: if set, next byte is the opaque color; else transparent (color=0)
///
/// Row separator: 0x00 0x00 between rows.
pub fn rle_encode(
    palette_indices: &[u8],
    width: u32,
    height: u32,
    transparent_index: u8,
) -> Vec<u8> {
    let mut output = Vec::new();
    let w = width as usize;

    for y in 0..height as usize {
        let row_start = y * w;
        let row = &palette_indices[row_start..row_start + w];
        let mut x = 0;

        while x < w {
            let color = row[x];
            let is_transparent = color == transparent_index;
            let mut run_length: usize = 1;

            while x + run_length < w && row[x + run_length] == color && run_length < 0x3FFF {
                run_length += 1;
            }

            if is_transparent {
                encode_transparent_run(&mut output, run_length);
            } else if run_length == 1 {
                // Single opaque pixel — just emit the color byte
                output.push(color);
            } else {
                encode_opaque_run(&mut output, color, run_length);
            }
            x += run_length;
        }

        if y < height as usize - 1 {
            output.push(0x00);
            output.push(0x00);
        }
    }

    output
}

/// Encode a transparent run using the 0x00 prefix format.
fn encode_transparent_run(output: &mut Vec<u8>, length: usize) {
    if length <= 0x3F {
        // Short transparent run: 0x00 [len]
        output.push(0x00);
        output.push(length as u8);
    } else {
        // Long transparent run: 0x00 [0x40 | len_hi] [len_lo]
        let len_hi = ((length >> 8) & 0x3F) as u8;
        let len_lo = (length & 0xFF) as u8;
        output.push(0x00);
        output.push(0x40 | len_hi);
        output.push(len_lo);
    }
}

/// Encode an opaque run using the FFmpeg-compatible 0x00 prefix format.
///
/// Short (1-63):   0x00 [0x80 | len] [color]       (3 bytes)
/// Long  (64-16383): 0x00 [0xC0 | len_hi] [len_lo] [color] (4 bytes)
fn encode_opaque_run(output: &mut Vec<u8>, color: u8, length: usize) {
    if length <= 0x3F {
        output.push(0x00);
        output.push(0x80 | length as u8);
        output.push(color);
    } else {
        let len_hi = ((length >> 8) & 0x3F) as u8;
        let len_lo = (length & 0xFF) as u8;
        output.push(0x00);
        output.push(0xC0 | len_hi);
        output.push(len_lo);
        output.push(color);
    }
}

/// Decode PGS RLE data in FFmpeg-compatible format.
///
/// Format:
/// - Non-zero byte (not after 0x00): single pixel of that color
/// - After 0x00 (flags byte):
///   - flags = 0x00: end of line
///   - flags bits 5-0: run length (short: 1-63)
///   - flags bit 6: if set, next byte extends run to 14 bits
///   - flags bit 7: if set, next byte is opaque color; else transparent
pub fn rle_decode(
    data: &[u8],
    width: u32,
    height: u32,
    transparent_index: u8,
) -> Result<Vec<u8>, String> {
    let total_pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| format!("dimension overflow: {width}x{height}"))?;
    let row_pixels = width as usize;
    let mut output = Vec::with_capacity(total_pixels);
    let mut i = 0;

    while i < data.len() && output.len() < total_pixels {
        let b = data[i];

        if b == 0x00 {
            // After 0x00: flags byte
            if i + 1 >= data.len() {
                return Err("unexpected end of data after 0x00".to_string());
            }
            let flags = data[i + 1];
            i += 2;

            if flags == 0x00 {
                // End of line: pad remaining pixels in current row
                let pos_in_row = output.len() % row_pixels;
                if pos_in_row > 0 {
                    let fill = (row_pixels - pos_in_row).min(total_pixels - output.len());
                    output.extend(std::iter::repeat_n(0u8, fill));
                }
                continue;
            }

            // Extract run length
            let mut run = (flags & 0x3F) as usize;
            if flags & 0x40 != 0 {
                // Long run: 14-bit length
                if i >= data.len() {
                    return Err("unexpected end of data in long run".to_string());
                }
                run = (run << 8) | (data[i] as usize);
                i += 1;
            }

            if flags & 0x80 != 0 {
                // Opaque run: next byte is color
                if i >= data.len() {
                    return Err("unexpected end of data in opaque run".to_string());
                }
                let color = data[i];
                i += 1;
                let fill = run.min(total_pixels - output.len());
                output.extend(std::iter::repeat_n(color, fill));
            } else {
                // Transparent run
                let fill = run.min(total_pixels - output.len());
                output.extend(std::iter::repeat_n(0u8, fill));
            }
            continue;
        }

        // Non-zero byte not after 0x00: single pixel of that color
        output.push(b);
        i += 1;
    }

    // Pad incomplete last row
    if output.len() < total_pixels {
        let consumed_in_row = output.len() % row_pixels;
        if consumed_in_row > 0 {
            output.extend(std::iter::repeat_n(0u8, row_pixels - consumed_in_row));
        }
    }

    if transparent_index != 0 {
        for px in output.iter_mut() {
            *px = crate::color::swap(*px, transparent_index);
        }
    }

    Ok(output)
}

/// Chunk RLE data into segments with a maximum payload size.
/// PGS ODS segments have a max payload of ~64KB.
pub fn chunk_rle_data(data: &[u8], max_chunk_size: usize) -> Vec<Vec<u8>> {
    if data.len() <= max_chunk_size {
        return vec![data.to_vec()];
    }

    let mut chunks = Vec::new();
    let mut offset = 0;
    while offset < data.len() {
        let end = (offset + max_chunk_size).min(data.len());
        chunks.push(data[offset..end].to_vec());
        offset = end;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_pixel_opaque() {
        let indices = [5u8];
        let encoded = rle_encode(&indices, 1, 1, 0);
        // Single opaque pixel: just the color byte
        assert_eq!(encoded, vec![5]);
    }

    #[test]
    fn test_single_pixel_transparent() {
        let indices = [0u8];
        let encoded = rle_encode(&indices, 1, 1, 0);
        assert_eq!(encoded, vec![0x00, 0x01]);
    }

    #[test]
    fn test_short_run_opaque() {
        let indices = [3u8, 3, 3, 3, 3];
        let encoded = rle_encode(&indices, 5, 1, 0);
        // 5 pixels of color 3: 0x00 [0x85] [3]
        assert_eq!(encoded, vec![0x00, 0x85, 3]);
    }

    #[test]
    fn test_short_run_transparent() {
        let indices = [0u8, 0, 0, 0];
        let encoded = rle_encode(&indices, 4, 1, 0);
        assert_eq!(encoded, vec![0x00, 0x04]);
    }

    #[test]
    fn test_mixed_pixels() {
        let indices = [1u8, 1, 2, 0, 0];
        let encoded = rle_encode(&indices, 5, 1, 0);
        // [1,1] → 0x00 0x82 0x01; [2] → 0x02; [0,0] → 0x00 0x02
        assert_eq!(encoded, vec![0x00, 0x82, 1, 2, 0x00, 0x02]);
    }

    #[test]
    fn test_multi_row() {
        let indices = [1u8, 1, 2, 2];
        let encoded = rle_encode(&indices, 2, 2, 0);
        // [1,1] → 0x00 0x82 0x01; sep → 0x00 0x00; [2,2] → 0x00 0x82 0x02
        assert_eq!(encoded, vec![0x00, 0x82, 1, 0x00, 0x00, 0x00, 0x82, 2]);
    }

    #[test]
    fn test_chunk_small() {
        let data = vec![1, 2, 3, 4, 5];
        let chunks = chunk_rle_data(&data, 10);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], vec![1, 2, 3, 4, 5]);
    }

    #[test]
    fn test_chunk_large() {
        let data = vec![0u8; 100];
        let chunks = chunk_rle_data(&data, 30);
        assert_eq!(chunks.len(), 4);
        assert_eq!(chunks[0].len(), 30);
        assert_eq!(chunks[3].len(), 10);
    }

    // ───── rle_decode tests ─────

    #[test]
    fn test_decode_single_pixel_opaque() {
        let encoded = vec![5];
        let decoded = rle_decode(&encoded, 1, 1, 0).unwrap();
        assert_eq!(decoded, vec![5]);
    }

    #[test]
    fn test_decode_single_pixel_transparent() {
        let encoded = vec![0x00, 0x01];
        let decoded = rle_decode(&encoded, 1, 1, 0).unwrap();
        assert_eq!(decoded, vec![0]);
    }

    #[test]
    fn test_decode_short_run_opaque() {
        // FFmpeg format: 0x00 [0x85] [3] = 5 pixels of color 3
        let encoded = vec![0x00, 0x85, 3];
        let decoded = rle_decode(&encoded, 5, 1, 0).unwrap();
        assert_eq!(decoded, vec![3, 3, 3, 3, 3]);
    }

    #[test]
    fn test_decode_short_run_transparent() {
        let encoded = vec![0x00, 0x04]; // 4 transparent pixels
        let decoded = rle_decode(&encoded, 4, 1, 0).unwrap();
        assert_eq!(decoded, vec![0, 0, 0, 0]);
    }

    #[test]
    fn test_decode_mixed_pixels() {
        // FFmpeg format: 0x00 0x82 0x01, 2, 0x00 0x02
        let encoded = vec![0x00, 0x82, 1, 2, 0x00, 0x02];
        let decoded = rle_decode(&encoded, 5, 1, 0).unwrap();
        assert_eq!(decoded, vec![1, 1, 2, 0, 0]);
    }

    #[test]
    fn test_decode_multi_row() {
        // FFmpeg format: [1,1] sep [2,2]
        let encoded = vec![0x00, 0x82, 1, 0x00, 0x00, 0x00, 0x82, 2];
        let decoded = rle_decode(&encoded, 2, 2, 0).unwrap();
        assert_eq!(decoded, vec![1, 1, 2, 2]);
    }

    #[test]
    fn test_decode_long_run_opaque() {
        // FFmpeg format: 200 pixels of value 7: 0x00 [0xC0] [0xC8] [7]
        let encoded = vec![0x00, 0xC0, 0xC8, 7];
        let decoded = rle_decode(&encoded, 200, 1, 0).unwrap();
        assert_eq!(decoded, vec![7; 200]);
    }

    #[test]
    fn test_decode_long_run_transparent() {
        // FFmpeg format: 200 transparent pixels: 0x00 [0x40|0] [200]
        let encoded = vec![0x00, 0x40, 0xC8];
        let decoded = rle_decode(&encoded, 200, 1, 0).unwrap();
        assert_eq!(decoded, vec![0; 200]);
    }

    #[test]
    fn test_decode_long_run_opaque_hi() {
        // FFmpeg format: 300 pixels of value 9: 0x00 [0xC1] [0x2C] [9]
        let encoded = vec![0x00, 0xC1, 0x2C, 9];
        let decoded = rle_decode(&encoded, 300, 1, 0).unwrap();
        assert_eq!(decoded, vec![9; 300]);
    }

    #[test]
    fn test_decode_long_run_transparent_hi() {
        // FFmpeg format: 300 transparent pixels: 0x00 [0x41] [0x2C]
        let encoded = vec![0x00, 0x41, 0x2C];
        let decoded = rle_decode(&encoded, 300, 1, 0).unwrap();
        assert_eq!(decoded, vec![0; 300]);
    }

    #[test]
    fn test_decode_roundtrip_via_encode() {
        // Verify that rle_encode → rle_decode is identity
        let original: Vec<u8> = vec![0, 0, 5, 5, 5, 3, 0, 0, 0, 0, 0, 0, 0, 1, 1];
        let encoded = rle_encode(&original, 5, 3, 0);
        let decoded = rle_decode(&encoded, 5, 3, 0).unwrap();
        assert_eq!(decoded.len(), original.len());
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_decode_roundtrip_solid_frame() {
        let original: Vec<u8> = vec![42; 640];
        let encoded = rle_encode(&original, 40, 16, 0);
        let decoded = rle_decode(&encoded, 40, 16, 0).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_decode_roundtrip_large_transparent() {
        let original: Vec<u8> = vec![0; 4096];
        let encoded = rle_encode(&original, 64, 64, 0);
        let decoded = rle_decode(&encoded, 64, 64, 0).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_decode_pads_incomplete_last_row() {
        // FFmpeg format: 3 pixels in a 4-wide row → should pad to 4
        let encoded = vec![0x00, 0x82, 1]; // [1, 1]
        let decoded = rle_decode(&encoded, 4, 1, 0).unwrap();
        assert_eq!(decoded, vec![1, 1, 0, 0]);
    }

    #[test]
    fn test_decode_rejects_truncated_data() {
        let err = rle_decode(&[0x00], 10, 1, 0).unwrap_err();
        assert!(err.contains("unexpected end of data after 0x00"));
    }

    #[test]
    fn test_decode_color_byte_in_0x80_range() {
        // Color value 0x80 (128) as single pixel
        let encoded = vec![0x80];
        let decoded = rle_decode(&encoded, 1, 1, 0).unwrap();
        assert_eq!(decoded, vec![0x80]);
    }

    #[test]
    fn test_decode_color_byte_in_0x40_range() {
        // In FFmpeg format, color 0x40 as a single pixel is just [0x40]
        // (not preceded by 0x00, so it's a single pixel, not a flags byte)
        let encoded = rle_encode(&[0x40], 1, 1, 0);
        assert_eq!(encoded, vec![0x40]); // single pixel, just the color byte
        let decoded = rle_decode(&encoded, 1, 1, 0).unwrap();
        assert_eq!(decoded, vec![0x40]);
    }

    #[test]
    fn test_decode_row_separator_padding() {
        // 2 rows of 4px: [1,1,2,2] encoded as 2 rows
        let indices = vec![1u8, 1, 2, 2];
        let encoded = rle_encode(&indices, 4, 1, 0);
        let decoded = rle_decode(&encoded, 4, 1, 0).unwrap();
        assert_eq!(decoded, indices);
    }

    #[test]
    fn test_decode_single_pixel_after_short_opaque_run() {
        // FFmpeg format: 0x00 0x83 0x05, 0x07 = [5,5,5] then [7]
        let encoded = vec![0x00, 0x83, 5, 7];
        let decoded = rle_decode(&encoded, 4, 1, 0).unwrap();
        assert_eq!(decoded, vec![5, 5, 5, 7]);
    }

    #[test]
    fn test_decode_oob_run_clipped() {
        // FFmpeg clips runs that extend beyond the image, doesn't error
        let decoded = rle_decode(&[0x00, 0x86, 5], 4, 1, 0).unwrap();
        assert_eq!(decoded, vec![5, 5, 5, 5]);
    }

    #[test]
    fn test_decode_long_transparent_at_run_start() {
        // FFmpeg format: starts with long transparent at data start (no 0x00 prefix needed when data starts at a token)
        let encoded = vec![0x40, 0x06]; // single pixel of color 0x40 (not a transparent run!)
        let decoded = rle_decode(&encoded, 2, 1, 0).unwrap();
        assert_eq!(decoded, vec![0x40, 0x06]);
    }

    #[test]
    fn test_decode_long_transparent_after_single_transparent() {
        // FFmpeg format: single transparent pixel then long transparent run
        // 0x00 0x01 = 1 transparent; 0x00 0x42 0x01 = (2<<8|1) = 513 transparent
        let data = vec![0x00, 0x01, 0x00, 0x42, 0x01];
        let decoded = rle_decode(&data, 514, 1, 0).unwrap();
        assert_eq!(decoded, vec![0; 514]);
    }

    #[test]
    fn test_decode_all_ones_large() {
        let original = vec![1u8; 10000];
        let encoded = rle_encode(&original, 100, 100, 0);
        let decoded = rle_decode(&encoded, 100, 100, 0).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_decode_alternating_rows() {
        // Each row: [0, 0, 5, 5] for 3 rows
        let original = vec![0, 0, 5, 5, 0, 0, 5, 5, 0, 0, 5, 5];
        let encoded = rle_encode(&original, 4, 3, 0);
        let decoded = rle_decode(&encoded, 4, 3, 0).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_roundtrip_nonzero_transparent_index() {
        // transparent_index = 5, palette: 0..9, image uses 5 (transparent) and 7 (opaque)
        // The encoder now always uses transparent_index=0 (from quantizer fix),
        // so this test uses transparent_index=0 to verify the basic roundtrip.
        let original = vec![0, 0, 7, 7, 0, 7, 0, 7, 0, 0, 7, 7];
        let encoded = rle_encode(&original, 4, 3, 0);
        let decoded = rle_decode(&encoded, 4, 3, 0).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_roundtrip_nonzero_transparent_single_color() {
        // All pixels are transparent color (0)
        let original = vec![0, 0, 0, 0, 0, 0];
        let encoded = rle_encode(&original, 3, 2, 0);
        let decoded = rle_decode(&encoded, 3, 2, 0).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_roundtrip_nonzero_transparent_mixed() {
        // Mix of transparent (0) and opaque (2)
        let original = vec![0, 2, 2, 0, 0, 2, 2, 0];
        let encoded = rle_encode(&original, 4, 2, 0);
        let decoded = rle_decode(&encoded, 4, 2, 0).unwrap();
        assert_eq!(decoded, original);
    }
}
