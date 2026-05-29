#[derive(Debug, Clone)]
pub struct RenderConfig {
    pub width: u32,
    pub height: u32,
    pub script_width: u32,
    pub script_height: u32,
    pub default_font: String,
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

#[derive(Debug, Clone)]
pub struct RenderContext {
    pub x: f32,
    pub y: f32,
    pub font_name: String,
    pub font_size: f32,
    pub primary_color: [u8; 4],
    pub secondary_color: [u8; 4],
    pub outline_color: [u8; 4],
    pub shadow_color: [u8; 4],
    pub bold: bool,
    pub italic: bool,
    pub outline_width: f32,
    pub shadow_depth: f32,
    pub blur: f32,
    pub rotation: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub spacing: f32,
    pub alignment: u8,
    pub margin_l: f32,
    pub margin_r: f32,
    pub margin_v: f32,
    pub origin_x: f32,
    pub origin_y: f32,
    pub shear_x: f32,
    pub shear_y: f32,
    pub clip_x1: f32,
    pub clip_y1: f32,
    pub clip_x2: f32,
    pub clip_y2: f32,
    pub clip_enabled: bool,
    pub wrap_style: u8,
    pub underline: bool,
    pub strikeout: bool,
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
            wrap_style: 0,
            underline: false,
            strikeout: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct RenderedFrame {
    pub pts_ms: u64,
    pub duration_ms: u64,
    pub width: u32,
    pub height: u32,
    pub bitmap: Vec<u8>,
}
