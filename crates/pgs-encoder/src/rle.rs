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
pub fn rle_encode(palette_indices: &[u8], width: u32, height: u32) -> Vec<u8> {
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

            encode_run(&mut output, color, run_length);
            x += run_length;
        }

        // Row separator (not after last row)
        if y < height as usize - 1 {
            output.push(0x00);
            output.push(0x00);
        }
    }

    output
}

fn encode_run(output: &mut Vec<u8>, color: u8, length: usize) {
    debug_assert!(length > 0 && length <= 0x3FFF);

    if length == 1 {
        if color == 0 {
            output.push(0x00);
        } else {
            output.push(color);
        }
    } else if length <= 0x3F {
        let len = length as u8;
        if color == 0 {
            output.push(0x00);
            output.push(len);
        } else {
            output.push(color);
            output.push(0x40 | len);
        }
    } else {
        let len_lo = (length & 0xFF) as u8;
        let len_hi = ((length >> 8) & 0x3F) as u8;
        if color == 0 {
            output.push(0x40 | len_hi);
            output.push(len_lo);
        } else {
            output.push(color);
            output.push(0x80 | len_hi);
            output.push(len_lo);
        }
    }
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
        let encoded = rle_encode(&indices, 1, 1);
        assert_eq!(encoded, vec![5]);
    }

    #[test]
    fn test_single_pixel_transparent() {
        let indices = [0u8];
        let encoded = rle_encode(&indices, 1, 1);
        assert_eq!(encoded, vec![0x00]);
    }

    #[test]
    fn test_short_run_opaque() {
        let indices = [3u8, 3, 3, 3, 3];
        let encoded = rle_encode(&indices, 5, 1);
        assert_eq!(encoded, vec![3, 0x45]);
    }

    #[test]
    fn test_short_run_transparent() {
        let indices = [0u8, 0, 0, 0];
        let encoded = rle_encode(&indices, 4, 1);
        assert_eq!(encoded, vec![0x00, 0x04]);
    }

    #[test]
    fn test_mixed_pixels() {
        let indices = [1u8, 1, 2, 0, 0];
        let encoded = rle_encode(&indices, 5, 1);
        assert_eq!(encoded, vec![1, 0x42, 2, 0x00, 0x02]);
    }

    #[test]
    fn test_multi_row() {
        let indices = [1u8, 1, 2, 2];
        let encoded = rle_encode(&indices, 2, 2);
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
}
