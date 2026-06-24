#![allow(missing_docs)]

/// Streaming pixel iterator over raw RGBA data.
///
/// Yields `[r, g, b, a]` arrays without allocating intermediate `Rgba`
/// structs, enabling zero-copy integration with SIMD quantize paths.
pub struct ChunkIter<'a> {
    data: &'a [u8],
    pos: usize,
    width: u32,
    x: u32,
    y: u32,
}

impl<'a> ChunkIter<'a> {
    pub fn new(data: &'a [u8], width: u32) -> Self {
        Self {
            data,
            pos: 0,
            width,
            x: 0,
            y: 0,
        }
    }
}

impl<'a> Iterator for ChunkIter<'a> {
    type Item = [u8; 4];

    fn next(&mut self) -> Option<Self::Item> {
        if self.pos + 4 > self.data.len() {
            return None;
        }
        let pixel = [
            self.data[self.pos],
            self.data[self.pos + 1],
            self.data[self.pos + 2],
            self.data[self.pos + 3],
        ];
        self.pos += 4;
        self.x += 1;
        if self.x == self.width {
            self.x = 0;
            self.y += 1;
        }
        Some(pixel)
    }
}
