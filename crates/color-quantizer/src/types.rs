#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    pub fn distance_sq(&self, other: &Rgba) -> u32 {
        let dr = self.r as i32 - other.r as i32;
        let dg = self.g as i32 - other.g as i32;
        let db = self.b as i32 - other.b as i32;
        let da = self.a as i32 - other.a as i32;
        (dr * dr + dg * dg + db * db + da * da) as u32
    }
}

#[derive(Debug, Clone)]
pub struct QuantizedFrame {
    pub width: u32,
    pub height: u32,
    pub palette: Vec<Rgba>,
    pub indices: Vec<u8>,
    pub transparent_index: u8,
}

impl QuantizedFrame {
    pub fn palette_size(&self) -> usize {
        self.palette.len()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DitherMethod {
    None,
    FloydSteinberg,
    Ordered,
}

impl Default for DitherMethod {
    fn default() -> Self {
        DitherMethod::FloydSteinberg
    }
}
