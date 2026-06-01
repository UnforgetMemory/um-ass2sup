/// Configuration for the subtitle rendering pipeline.
///
/// `width` and `height` define the output bitmap dimensions (e.g. 1920×1080 for
/// Full HD). `script_width` and `script_height` define the coordinate system
/// used by ASS override tags like `\pos` and `\clip` — they are usually the same
/// as the output dimensions but can differ when the ASS file was authored for a
/// different resolution (e.g. 1280×720 script rendered to 1920×1080 output).
///
/// # Defaults
///
/// | Field | Default |
/// |-------|---------|
/// | `width` | 1920 |
/// | `height` | 1080 |
/// | `script_width` | 1920 |
/// | `script_height` | 1080 |
/// | `default_font` | `"Arial"` |
/// | `default_font_size` | 48.0 |
#[derive(Debug, Clone)]
pub struct RenderConfig {
    /// Output bitmap width in pixels.
    pub width: u32,
    /// Output bitmap height in pixels.
    pub height: u32,
    /// ASS script coordinate system width (used by `\pos`, `\clip`, etc.).
    pub script_width: u32,
    /// ASS script coordinate system height.
    pub script_height: u32,
    /// Font family name used when the ASS style does not specify one.
    pub default_font: String,
    /// Font size in points used when the ASS style does not specify one.
    pub default_font_size: f32,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
            script_width: 1920,
            script_height: 1080,
            default_font: "Arial".into(),
            default_font_size: 48.0,
        }
    }
}

/// Per-event rendering state built from ASS style defaults + override tags.
///
/// `build_context()` populates this from a [`Style`](ass_parser::style::Style)
/// and the event's override tags. Fields use RGBA byte order (`[R, G, B, A]`)
/// where alpha 255 = fully opaque (the opposite of ASS's inverted alpha).
///
/// # Defaults
///
/// | Field | Default |
/// |-------|---------|
/// | `primary_color` | white `[255,255,255,255]` |
/// | `secondary_color` | blue `[0,0,255,255]` |
/// | `outline_color` | black `[0,0,0,255]` |
/// | `shadow_color` | semi-transparent black `[0,0,0,128]` |
/// | `scale_x` / `scale_y` | 100.0 (percent) |
/// | `alignment` | 2 (bottom-center, ASS numpad) |
/// | `alpha_multiplier` | 1.0 (fully opaque) |
#[derive(Debug, Clone)]
pub struct RenderContext {
    /// X position in script coordinates (set by `\pos`, `\move`).
    pub x: f32,
    /// Y position in script coordinates.
    pub y: f32,
    /// Font family name (set by `\fn`).
    pub font_name: String,
    /// Font size in points (set by `\fs`).
    pub font_size: f32,
    /// Primary fill color `[R, G, B, A]` (set by `\1c`, `\c`).
    pub primary_color: [u8; 4],
    /// Secondary karaoke color `[R, G, B, A]` (set by `\2c`).
    pub secondary_color: [u8; 4],
    /// Outline stroke color `[R, G, B, A]` (set by `\3c`).
    pub outline_color: [u8; 4],
    /// Shadow color `[R, G, B, A]` (set by `\4c`).
    pub shadow_color: [u8; 4],
    /// Bold weight flag (set by `\b1` / `\b0`).
    pub bold: bool,
    /// Italic flag (set by `\i1` / `\i0`).
    pub italic: bool,
    /// Outline width in pixels (set by `\bord`).
    pub outline_width: f32,
    /// Shadow depth in pixels (set by `\shad`).
    pub shadow_depth: f32,
    /// Gaussian blur radius (set by `\blur`, `\be`).
    pub blur: f32,
    /// Z-axis rotation in degrees (set by `\frz`, `\fr`).
    pub rotation: f32,
    /// Horizontal scale percentage (set by `\fscx`). 100.0 = normal.
    pub scale_x: f32,
    /// Vertical scale percentage (set by `\fscy`). 100.0 = normal.
    pub scale_y: f32,
    /// Extra letter spacing in pixels (set by `\fsp`).
    pub spacing: f32,
    /// ASS numpad alignment 1–9 (set by `\an`, `\a`). See [`alignment_to_pos`](crate::Renderer::alignment_to_pos).
    pub alignment: u8,
    /// Left margin override in pixels.
    pub margin_l: f32,
    /// Right margin override in pixels.
    pub margin_r: f32,
    /// Vertical margin override in pixels (top or bottom depending on alignment).
    pub margin_v: f32,
    /// Rotation origin X (set by `\org`).
    pub origin_x: f32,
    /// Rotation origin Y (set by `\org`).
    pub origin_y: f32,
    /// Horizontal shear factor (set by `\fax`).
    pub shear_x: f32,
    /// Vertical shear factor (set by `\fay`).
    pub shear_y: f32,
    /// Clip rectangle left edge (set by `\clip`).
    pub clip_x1: f32,
    /// Clip rectangle top edge.
    pub clip_y1: f32,
    /// Clip rectangle right edge.
    pub clip_x2: f32,
    /// Clip rectangle bottom edge.
    pub clip_y2: f32,
    /// Whether a clip rectangle is active.
    pub clip_enabled: bool,
    /// If true, pixels *inside* the clip rect are cleared (set by `\iclip`).
    pub clip_inverse: bool,
    /// Text wrapping mode (set by `\q`). 0=smart, 1=no wrap, 2=force newline, 3=smart (same as 0).
    pub wrap_style: u8,
    /// Underline decoration (set by `\u1` / `\u0`).
    pub underline: bool,
    /// Strikeout decoration (set by `\s1` / `\s0`).
    pub strikeout: bool,
    /// Alpha multiplier for fade effects (1.0 = fully opaque, 0.0 = fully transparent)
    pub alpha_multiplier: f32,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            font_name: "Arial".into(),
            font_size: 48.0,
            primary_color: [255, 255, 255, 255],
            secondary_color: [0, 0, 255, 255],
            outline_color: [0, 0, 0, 255],
            shadow_color: [0, 0, 0, 128],
            bold: false,
            italic: false,
            outline_width: 2.0,
            shadow_depth: 2.0,
            blur: 0.0,
            rotation: 0.0,
            scale_x: 100.0,
            scale_y: 100.0,
            spacing: 0.0,
            alignment: 2,
            margin_l: 10.0,
            margin_r: 10.0,
            margin_v: 10.0,
            origin_x: 0.0,
            origin_y: 0.0,
            shear_x: 0.0,
            shear_y: 0.0,
            clip_x1: -1.0,
            clip_y1: -1.0,
            clip_x2: -1.0,
            clip_y2: -1.0,
            clip_enabled: false,
            clip_inverse: false,
            wrap_style: 0,
            underline: false,
            strikeout: false,
            alpha_multiplier: 1.0,
        }
    }
}

/// A single rendered subtitle frame as an RGBA bitmap.
///
/// The bitmap is stored in row-major order with 4 bytes per pixel
/// (red, green, blue, alpha). Alpha 255 = fully opaque.
#[derive(Debug, Clone)]
pub struct RenderedFrame {
    /// Presentation timestamp in milliseconds (90 kHz PTS base for PGS).
    pub pts_ms: u64,
    /// Display duration in milliseconds.
    pub duration_ms: u64,
    /// Bitmap width in pixels.
    pub width: u32,
    /// Bitmap height in pixels.
    pub height: u32,
    /// RGBA pixel data (`width * height * 4` bytes, row-major).
    pub bitmap: Vec<u8>,
}
