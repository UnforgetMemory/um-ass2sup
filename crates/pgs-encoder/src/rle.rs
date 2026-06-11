/// PGS-specific RLE encoding for palette-indexed pixel data.
///
/// Format per run:
/// - Single pixel (color != 0): just the color byte
/// - Single transparent pixel (color == 0): 0x00
/// - Short run (len <= 0x3F):
///   - Transparent: `[len_hi | 0x00] [len_lo]` (2 bytes)
///   - Opaque:      `[color] [len_hi | 0x40] [len_lo]` (3 bytes)
/// - Long run (len > 0x3F, max 0x3FFF):
///   - Transparent: `[len_hi | 0x40] [len_lo]` (2 bytes)
///   - Opaque:      `[color] [len_hi | 0x80] [len_lo]` (3 bytes)
///
/// Row separator: 0x00 0x00 between rows (except after last row).
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
            let mut run_length: usize = 1;

            while x + run_length < w && row[x + run_length] == color && run_length < 0x3FFF {
                run_length += 1;
            }

            let (enc_color, enc_transparent) = if transparent_index != 0 {
                (swap(color, transparent_index), 0)
            } else {
                (color, transparent_index)
            };
            encode_run(&mut output, enc_color, run_length, enc_transparent);
            x += run_length;
        }

        if y < height as usize - 1 {
            output.push(0x00);
            output.push(0x00);
        }
    }

    output
}

fn swap(val: u8, pivot: u8) -> u8 {
    if val == 0 {
        pivot
    } else if val == pivot {
        0
    } else {
        val
    }
}

fn encode_run(output: &mut Vec<u8>, color: u8, length: usize, transparent_index: u8) {
    debug_assert!(length > 0 && length <= 0x3FFF);
    let is_transparent = color == transparent_index;
    let color_in_collision_range = color & 0xC0 == 0x40;

    if length == 1 && is_transparent {
        output.push(0x00);
        output.push(0x01);
    } else if length == 1 && !color_in_collision_range {
        output.push(color);
    } else if length == 1 {
        output.push(color);
        output.push(0x80);
        output.push(0x01);
    } else if is_transparent && length <= 0x3F {
        output.push(0x00);
        output.push(length as u8);
    } else if length <= 0x3F && !color_in_collision_range {
        output.push(color);
        output.push(0x40 | length as u8);
    } else {
        let len_lo = (length & 0xFF) as u8;
        let len_hi = ((length >> 8) & 0x3F) as u8;
        if is_transparent && (len_lo & 0xC0 == 0x80) {
            let first_len = (length & 0xFF00) | 0x7F;
            encode_run(output, transparent_index, first_len, transparent_index);
            encode_run(
                output,
                transparent_index,
                length - first_len,
                transparent_index,
            );
        } else if is_transparent {
            output.push(0x40 | len_hi);
            output.push(len_lo);
        } else {
            output.push(color);
            output.push(0x80 | len_hi);
            output.push(len_lo);
        }
    }
}

pub fn rle_decode(
    data: &[u8],
    width: u32,
    height: u32,
    transparent_index: u8,
) -> Result<Vec<u8>, String> {
    let total_pixels = (width as usize) * (height as usize);
    let row_pixels = width as usize;
    let mut output = Vec::with_capacity(total_pixels);
    let mut i = 0;

    while i < data.len() && output.len() < total_pixels {
        let b = data[i];

        if b == 0x00 {
            if i + 1 >= data.len() {
                return Err("unexpected end of data in transparent sequence".to_string());
            } else {
                let n = data[i + 1];
                if n == 0x00 {
                    let pos_in_row = output.len() % row_pixels;
                    if pos_in_row > 0 {
                        let fill = (row_pixels - pos_in_row).min(total_pixels - output.len());
                        output.extend(std::iter::repeat_n(0u8, fill));
                    }
                    i += 2;
                } else if n < 0x40 {
                    let fill = (n as usize).min(total_pixels - output.len());
                    output.extend(std::iter::repeat_n(0u8, fill));
                    i += 2;
                } else {
                    output.push(0u8);
                    i += 1;
                }
            }
            continue;
        }

        if b & 0xC0 == 0x40 {
            let next = if i + 1 < data.len() { data[i + 1] } else { 0 };
            if next & 0xC0 == 0x80 {
                // Ambiguous: could be transparent [0x40|len_hi, len_lo] with len_lo >= 0x80,
                // or opaque [color, 0x80|len_hi, len_lo] with color in 0x40..0x7F.
                // Try transparent interpretation first.
                let transparent_len = ((b & 0x3F) as usize) << 8 | (next as usize);
                let remaining = total_pixels - output.len();
                if transparent_len > 0 && transparent_len <= remaining {
                    i += 2;
                    output.extend(std::iter::repeat_n(0u8, transparent_len));
                    continue;
                }
                // Transparent failed; try opaque.
                let color = b;
                i += 2;
                if i >= data.len() {
                    return Err("unexpected end of data in long opaque run".to_string());
                }
                let len_lo = data[i] as usize;
                i += 1;
                let len_hi = (next & 0x3F) as usize;
                let len = (len_hi << 8) | len_lo;
                if len > 0 && len <= remaining {
                    output.extend(std::iter::repeat_n(color, len));
                    continue;
                }
                return Err(format!("invalid run length {len}"));
            }
            let len_hi = (b & 0x3F) as usize;
            i += 1;
            if i >= data.len() {
                return Err("unexpected end of data in long transparent run".to_string());
            }
            let len_lo = data[i] as usize;
            i += 1;
            let len = (len_hi << 8) | len_lo;
            if len == 0 || len > total_pixels - output.len() {
                return Err(format!("invalid run length {len}"));
            }
            output.extend(std::iter::repeat_n(0u8, len));
            continue;
        }

        let color = b;
        i += 1;

        if i >= data.len() {
            output.push(color);
            continue;
        }

        let n = data[i];
        if n & 0xC0 == 0x40 {
            let len = (n & 0x3F) as usize;
            if len > 0 && len <= total_pixels - output.len() {
                output.extend(std::iter::repeat_n(color, len));
                i += 1;
                continue;
            }
            if len == 0 {
                // len==0 means this is not a valid short opaque run.
                // The 0x40 byte is the start of a transparent long run.
                output.push(color);
                continue;
            }
            return Err(format!("invalid run length {len}"));
        } else if n & 0xC0 == 0x80 {
            let len_hi = (n & 0x3F) as usize;
            i += 1;
            if i >= data.len() {
                return Err("unexpected end of data in long opaque run".to_string());
            }
            let len_lo = data[i] as usize;
            i += 1;
            let len = (len_hi << 8) | len_lo;
            if len > 0 && len <= total_pixels - output.len() {
                output.extend(std::iter::repeat_n(color, len));
                continue;
            }
            return Err(format!("invalid run length {len}"));
        }

        output.push(color);
    }

    if output.len() < total_pixels {
        let consumed_in_row = output.len() % row_pixels;
        if consumed_in_row > 0 {
            output.extend(std::iter::repeat_n(0u8, row_pixels - consumed_in_row));
        }
    }

    if output.len() < total_pixels {
        return Err(format!(
            "RLE decode produced {} pixels, expected {total_pixels}",
            output.len()
        ));
    }

    if transparent_index != 0 {
        for px in output.iter_mut() {
            *px = swap(*px, transparent_index);
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
        assert_eq!(encoded, vec![3, 0x45]);
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
        assert_eq!(encoded, vec![1, 0x42, 2, 0x00, 0x02]);
    }

    #[test]
    fn test_multi_row() {
        let indices = [1u8, 1, 2, 2];
        let encoded = rle_encode(&indices, 2, 2, 0);
        assert_eq!(encoded, vec![1, 0x42, 0x00, 0x00, 2, 0x42]);
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
        let encoded = vec![3, 0x45]; // 5 pixels of value 3
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
        let encoded = vec![1, 0x42, 2, 0x00, 0x02]; // 1,1,2,0,0
        let decoded = rle_decode(&encoded, 5, 1, 0).unwrap();
        assert_eq!(decoded, vec![1, 1, 2, 0, 0]);
    }

    #[test]
    fn test_decode_multi_row() {
        let encoded = vec![1, 0x42, 0x00, 0x00, 2, 0x42]; // [1,1][sep][2,2]
        let decoded = rle_decode(&encoded, 2, 2, 0).unwrap();
        assert_eq!(decoded, vec![1, 1, 2, 2]);
    }

    #[test]
    fn test_decode_long_run_opaque() {
        // 200 pixels of value 7: len=0xC8 → len_hi=0, len_lo=0xC8
        let encoded = vec![7, 0x80, 0xC8];
        let decoded = rle_decode(&encoded, 200, 1, 0).unwrap();
        assert_eq!(decoded, vec![7; 200]);
    }

    #[test]
    fn test_decode_long_run_transparent() {
        // 200 transparent pixels: len=0xC8 → len_hi=0, len_lo=0xC8
        let encoded = vec![0x40, 0xC8];
        let decoded = rle_decode(&encoded, 200, 1, 0).unwrap();
        assert_eq!(decoded, vec![0; 200]);
    }

    #[test]
    fn test_decode_long_run_opaque_hi() {
        // 300 pixels of value 9: len=0x12C → len_hi=1, len_lo=0x2C
        let encoded = vec![9, 0x81, 0x2C];
        let decoded = rle_decode(&encoded, 300, 1, 0).unwrap();
        assert_eq!(decoded, vec![9; 300]);
    }

    #[test]
    fn test_decode_long_run_transparent_hi() {
        // 300 transparent pixels: len=0x12C → len_hi=1, len_lo=0x2C
        let encoded = vec![0x41, 0x2C];
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
        // 3 pixels in a 4-wide row → should pad to 4
        let encoded = vec![1, 0x42]; // [1, 1]
        let decoded = rle_decode(&encoded, 4, 1, 0).unwrap();
        assert_eq!(decoded, vec![1, 1, 0, 0]);
    }

    #[test]
    fn test_decode_rejects_truncated_data() {
        let err = rle_decode(&[0x00], 10, 1, 0).unwrap_err();
        assert!(err.contains("unexpected end of data in transparent sequence"));
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
        // Long transparent run at 0x41 = start of run
        // But a single pixel color 0x40 would be encoded as... actually
        // color 0x40 as single pixel is just [0x40]. Let's test:
        // Color 0x40 → NOT long transparent because we need context.
        // At run start, 0x40 = long transparent marker.
        // But color pixel 0x40 is only produced by the encoder as [0x40].
        // The decoder at run start sees 0x40 → processes as long transparent.
        // This means color index 64 CANNOT be decoded from single-pixel encoding!
        // This is a genuine PGS RLE quirk.
        let encoded = rle_encode(&[0x40], 1, 1, 0);
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
        // [5,5,5] as short run [5, 0x43], then [7] as single
        let encoded = vec![5, 0x43, 7];
        let decoded = rle_decode(&encoded, 4, 1, 0).unwrap();
        assert_eq!(decoded, vec![5, 5, 5, 7]);
    }

    #[test]
    fn test_decode_oob_run_rejected() {
        let err = rle_decode(&[5, 0x46], 4, 1, 0).unwrap_err();
        assert!(err.contains("invalid run length"));
    }

    #[test]
    fn test_decode_long_transparent_at_run_start() {
        // Starts directly with long transparent marker (no leading 0x00)
        let encoded = vec![0x40, 0x06]; // 6 transparent pixels
        let decoded = rle_decode(&encoded, 6, 1, 0).unwrap();
        assert_eq!(decoded, vec![0; 6]);
    }

    #[test]
    fn test_decode_long_transparent_after_single_transparent() {
        // Single transparent pixel followed by long transparent run
        // RLE: [0x00][0x42 0x01] = 1 + (2<<8|1) = 1 + 513 = 514 transparent
        // But encoder merges adjacent same-color runs, so this specific pattern
        // would only appear from malformed data. Test that we handle it.
        let data = vec![0x00, 0x42, 0x01];
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
        let original = vec![5, 5, 7, 7, 5, 7, 5, 7, 5, 5, 7, 7];
        let encoded = rle_encode(&original, 4, 3, 5);
        let decoded = rle_decode(&encoded, 4, 3, 5).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_roundtrip_nonzero_transparent_single_color() {
        // All pixels are the transparent color (index 3)
        let original = vec![3, 3, 3, 3, 3, 3];
        let encoded = rle_encode(&original, 3, 2, 3);
        let decoded = rle_decode(&encoded, 3, 2, 3).unwrap();
        assert_eq!(decoded, original);
    }

    #[test]
    fn test_roundtrip_nonzero_transparent_mixed() {
        // transparent_index = 1, mix of transparent (1) and opaque (0, 2)
        let original = vec![1, 0, 2, 1, 1, 0, 2, 1];
        let encoded = rle_encode(&original, 4, 2, 1);
        let decoded = rle_decode(&encoded, 4, 2, 1).unwrap();
        assert_eq!(decoded, original);
    }
}
