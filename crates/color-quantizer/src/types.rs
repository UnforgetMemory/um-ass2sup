/// An RGBA color with 8-bit channels.
///
/// Used throughout the quantizer to represent palette entries and pixel data
/// for PGS/ASS subtitle rendering. The alpha channel (`a`) follows ASS
/// convention where `0` is fully transparent and `255` is fully opaque.
///
/// # Examples
///
/// ```no_run
/// use color_quantizer::Rgba;
///
/// let pixel = Rgba::new(255, 128, 0, 255); // opaque orange
/// let transparent = Rgba::new(0, 0, 0, 0); // fully transparent
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgba {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
    /// Alpha channel. `0` = transparent, `255` = opaque.
    pub a: u8,
}

impl Rgba {
    /// Creates a new RGBA color.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use color_quantizer::Rgba;
    ///
    /// let c = Rgba::new(255, 0, 128, 200);
    /// ```
    pub fn new(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Returns the squared Euclidean distance between two colors in RGBA space.
    ///
    /// This is used for nearest-color lookups in palette indexing. The square
    /// root is omitted since only relative ordering is needed.
    pub fn distance_sq(&self, other: &Rgba) -> u32 {
        let dr = i32::from(self.r) - i32::from(other.r);
        let dg = i32::from(self.g) - i32::from(other.g);
        let db = i32::from(self.b) - i32::from(other.b);
        let da = i32::from(self.a) - i32::from(other.a);
        (dr * dr + dg * dg + db * db + da * da) as u32
    }
}

/// The output of color quantization: a reduced palette and per-pixel index map.
///
/// For PGS subtitle encoding, the palette is limited to ≤255 entries so that
/// one index can be reserved for the transparent color.
#[derive(Debug, Clone)]
pub struct QuantizedFrame {
    /// Image width in pixels.
    pub width: u32,
    /// Image height in pixels.
    pub height: u32,
    /// The reduced RGBA palette (up to 255 colors plus optional transparent entry).
    pub palette: Vec<Rgba>,
    /// One byte per pixel indexing into `palette`.
    pub indices: Vec<u8>,
    /// Index within `palette` representing full transparency.
    pub transparent_index: u8,
}

impl QuantizedFrame {
    /// Returns the number of colors in the palette.
    pub fn palette_size(&self) -> usize {
        self.palette.len()
    }
}

/// Dithering algorithm applied during palette reduction.
///
/// Dithering spreads quantization error to neighboring pixels, reducing
/// banding artifacts in smooth gradients. This is especially useful for
/// ASS subtitle renders that contain alpha-blended shadows and gradients.
#[derive(Debug, Clone, Copy, Default)]
pub enum DitherMethod {
    /// No dithering — each pixel maps to the nearest palette color independently.
    None,
    /// Floyd–Steinberg error-diffusion dithering. Good quality, moderate cost.
    #[default]
    FloydSteinberg,
    /// Bayer ordered dithering. Faster but produces visible cross-hatch patterns.
    Ordered,
}
