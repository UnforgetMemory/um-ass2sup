#![allow(missing_docs)]

/// Borrowed zero-copy view into raw RGBA pixel data.
pub struct RgbaRef<'a>(pub &'a [u8]);

impl<'a> RgbaRef<'a> {
    pub fn new(data: &'a [u8]) -> Self {
        Self(data)
    }

    pub fn pixel_at(&self, x: usize, y: usize, width: u32) -> [u8; 4] {
        let offset = (y * width as usize + x) * 4;
        [
            self.0[offset],
            self.0[offset + 1],
            self.0[offset + 2],
            self.0[offset + 3],
        ]
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.0
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}
