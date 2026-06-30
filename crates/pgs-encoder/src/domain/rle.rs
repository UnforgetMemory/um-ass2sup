/// Maximum pixel run length for a single RLE command (14-bit).
const MAX_RUN_LENGTH: usize = 0x3FFF;

/// Encode a run of transparent pixels using the 0x00 prefix format.
fn encode_transparent_run(output: &mut Vec<u8>, length: usize) {
    if length <= 0x3F {
        output.push(0x00);
        output.push(length as u8);
    } else {
        let len_hi = ((length >> 8) & 0x3F) as u8;
        let len_lo = (length & 0xFF) as u8;
        output.push(0x00);
        output.push(0x40 | len_hi);
        output.push(len_lo);
    }
}

/// Encode an opaque run using the FFmpeg-compatible 0x00 prefix format.
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

/// Encode a frame's palette indices into PGS RLE byte stream.
///
/// Format compatible with FFmpeg's `pgssub` decoder:
/// - Non-zero byte (not after 0x00): single pixel of that color
/// - After 0x00 (flags byte):
///   - flags = 0x00: end of line
///   - flags bits 5-0: run length
///   - flags bit 6: if set, next byte extends run to 14 bits
///   - flags bit 7: if set, next byte is opaque color; else transparent
///
/// Row separator: `0x00 0x00` between rows.
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

            while x + run_length < w && row[x + run_length] == color && run_length < MAX_RUN_LENGTH
            {
                run_length += 1;
            }

            if is_transparent {
                encode_transparent_run(&mut output, run_length);
            } else if run_length == 1 && color != 0 {
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

/// Decode PGS RLE-compressed bitmap data into raw palette indices.
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
            if i + 1 >= data.len() {
                return Err("unexpected end of data after 0x00".to_string());
            }
            let flags = data[i + 1];
            i += 2;
            if flags == 0x00 {
                let pos_in_row = output.len() % row_pixels;
                if pos_in_row > 0 {
                    let fill = (row_pixels - pos_in_row).min(total_pixels - output.len());
                    output.extend(std::iter::repeat_n(0u8, fill));
                }
                continue;
            }
            let mut run = (flags & 0x3F) as usize;
            if flags & 0x40 != 0 {
                if i >= data.len() {
                    return Err("unexpected end of data in long run".to_string());
                }
                run = (run << 8) | (data[i] as usize);
                i += 1;
            }
            if flags & 0x80 != 0 {
                if i >= data.len() {
                    return Err("unexpected end of data in opaque run".to_string());
                }
                let color = data[i];
                i += 1;
                output.extend(std::iter::repeat_n(
                    color,
                    run.min(total_pixels - output.len()),
                ));
            } else {
                output.extend(std::iter::repeat_n(
                    0u8,
                    run.min(total_pixels - output.len()),
                ));
            }
            continue;
        }
        output.push(b);
        i += 1;
    }
    if output.len() < total_pixels {
        let consumed_in_row = output.len() % row_pixels;
        if consumed_in_row > 0 {
            output.extend(std::iter::repeat_n(0u8, row_pixels - consumed_in_row));
        }
    }
    if transparent_index != 0 {
        for px in output.iter_mut() {
            *px = crate::domain::palette::swap(*px, transparent_index);
        }
    }
    Ok(output)
}
