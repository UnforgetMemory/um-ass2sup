use libass_sys;

/// A single image layer produced by libass, with alpha buffer copied to safe storage.
#[derive(Debug, Clone)]
pub struct AssImageData {
    /// Width of the bitmap in pixels.
    pub w: u32,
    /// Height of the bitmap in pixels.
    pub h: u32,
    /// Bytes per row (may be larger than `w`).
    pub stride: u32,
    /// Alpha channel buffer (one byte per pixel, row-major, stride bytes per row).
    pub bitmap: Vec<u8>,
    /// RGBA color packed as 0xAABBGGRR.
    pub color: u32,
    /// X position in the video frame.
    pub dst_x: u32,
    /// Y position in the video frame.
    pub dst_y: u32,
    /// Layer type (character, outline, or shadow).
    pub image_type: ImageType,
}

/// Image layer type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageType {
    /// Foreground character glyph.
    Character,
    /// Outline/border.
    Outline,
    /// Drop shadow.
    Shadow,
}

impl From<libass_sys::ImageType> for ImageType {
    fn from(t: libass_sys::ImageType) -> Self {
        match t {
            libass_sys::ImageType::Character => ImageType::Character,
            libass_sys::ImageType::Outline => ImageType::Outline,
            libass_sys::ImageType::Shadow => ImageType::Shadow,
        }
    }
}

/// Full RGBA frame buffer.
#[derive(Debug, Clone)]
pub struct RgbaFrame {
    /// RGBA pixel data (4 bytes per pixel, row-major).
    pub data: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
}

/// Cropped RGBA region (tight bounding box of non-transparent content).
#[derive(Debug, Clone)]
pub struct CroppedFrame {
    /// RGBA pixel data for the cropped region.
    pub data: Vec<u8>,
    /// X offset in the full frame.
    pub x: u32,
    /// Y offset in the full frame.
    pub y: u32,
    /// Width of the cropped region.
    pub width: u32,
    /// Height of the cropped region.
    pub height: u32,
}

/// Parsed event metadata from ASS track.
#[derive(Debug, Clone)]
pub struct AssEventInfo {
    /// Start time in milliseconds.
    pub start_ms: i64,
    /// Duration in milliseconds.
    pub duration_ms: i64,
    /// Index into the track's style array.
    pub style: i32,
    /// Event text with override tags.
    pub text: String,
}
