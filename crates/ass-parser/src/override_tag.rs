/// ASS override tag — parsed from `{\tag}` blocks in subtitle text.
///
/// Override tags modify rendering properties (position, color, font, animation, etc.)
/// within a single subtitle event. Tags are enclosed in `{}` blocks and can be combined:
/// `{\b1\i1\fs24}Bold italic text`
///
/// # Tag Reference
///
/// | Tag | Variant | Description |
/// |-----|---------|-------------|
/// | `\pos(x,y)` | [`Pos`] | Fixed screen position |
/// | `\move(x1,y1,x2,y2,t1,t2)` | [`Move`] | Animated movement |
/// | `\fad(in,out)` | [`Fade`] | Simple fade in/out (ms) |
/// | `\fade(a1,a2,a3,t1,t2,t3,t4)` | [`FadeComplex`] | 3-segment alpha animation |
/// | `\t(tag,t1,t2,accel)` | [`Transform`] | Animated attribute interpolation |
/// | `\b1`/`\b0` | [`Bold`] | Toggle bold |
/// | `\bN` | [`BoldWeight`] | Set font weight (100-900) |
/// | `\i1`/`\i0` | [`Italic`] | Toggle italic |
/// | `\fn[name]` | [`FontName`] | Change font family |
/// | `\fs[size]` | [`FontSize`] | Change font size |
/// | `\frz(angle)` | [`Rotation`] | Z-axis rotation (degrees) |
/// | `\fscx(pct)`/`\fscy(pct)` | [`Scale`] | Scale X/Y (percentage) |
/// | `\clip(x1,y1,x2,y2)` | [`Clip`] | Rectangular clip region |
/// | `\iclip(x1,y1,x2,y2)` | [`ClipInverse`] | Inverse rectangular clip |
/// | `\k`/`\kf`/`\ko`/`\kt` | [`Karaoke`] | Karaoke timing |
///
/// See the [ASS specification](http://www.tcax.org/docs/ass-specs.htm) for the full tag list.
#[derive(Debug, Clone, PartialEq)]
pub enum OverrideTag {
    /// `\pos(x,y)` — fixed subtitle position on screen.
    Pos { x: f64, y: f64 },
    /// `\move(x1,y1,x2,y2,t1,t2)` — animated movement from (x1,y1) to (x2,y2) between t1..t2 ms.
    Move { x1: f64, y1: f64, x2: f64, y2: f64, t1: u64, t2: u64 },
    /// `\fad(duration_in,duration_out)` — simple fade in/out in milliseconds.
    Fade { duration_in: u64, duration_out: u64 },
    /// `\fade(a1,a2,a3,t1,t2,t3,t4)` — 3-segment alpha animation (0=transparent, 255=opaque).
    FadeComplex { alpha_start: u8, alpha_mid: u8, alpha_end: u8, t1: u64, t2: u64, t3: u64, t4: u64 },
    /// `\t(tag,t1,t2,accel)` — animated attribute interpolation with acceleration curve.
    Transform { tag: String, t1: u64, t2: u64, accel: f64 },
    /// `\fn[name]` — change font family.
    FontName(String),
    /// `\fs[size]` — change font size in points.
    FontSize(f64),
    /// `\b1`/`\b0` — toggle bold on/off.
    Bold(bool),
    /// `\bN` — set font weight (100–900, e.g., 700 = bold).
    BoldWeight(u32),
    /// `\i1`/`\i0` — toggle italic on/off.
    Italic(bool),
    /// `\u1`/`\u0` — toggle underline on/off.
    Underline(bool),
    /// `\s1`/`\s0` — toggle strikethrough on/off.
    Strikeout(bool),
    /// `\1c&HBBGGRR&` — primary fill color (ASS ABGR format).
    PrimaryColor(super::color::AssColor),
    /// `\2c&HBBGGRR&` — secondary color (used in karaoke).
    SecondaryColor(super::color::AssColor),
    /// `\3c&HBBGGRR&` — outline/border color.
    OutlineColor(super::color::AssColor),
    /// `\4c&HBBGGRR&` — shadow color.
    ShadowColor(super::color::AssColor),
    /// `\alpha&HAA&` — global alpha (0=opaque, 255=transparent, note: inverted from normal).
    Alpha { value: u8 },
    /// `\1a&HAA&` — primary color alpha.
    PrimaryAlpha { value: u8 },
    /// `\2a&HAA&` — secondary color alpha.
    SecondaryAlpha { value: u8 },
    /// `\3a&HAA&` — outline color alpha.
    OutlineAlpha { value: u8 },
    /// `\4a&HAA&` — shadow color alpha.
    ShadowAlpha { value: u8 },
    /// `\frz(angle)`, `\frx(angle)`, `\fry(angle)` — rotation in degrees (Z/X/Y axes).
    Rotation { x: f64, y: f64, z: f64 },
    /// `\fscx(pct)`/`\fscy(pct)` — scale as percentage (100 = normal size).
    Scale { x: f64, y: f64 },
    /// `\fsp(spacing)` — extra spacing between characters in pixels.
    Spacing(f64),
    /// `\be(strength)` — blur edge effect.
    Blur(f64),
    /// `\blur(strength)` — Gaussian blur radius.
    GaussianBlur(f64),
    /// `\bord(width)` — uniform border/outline width.
    Border(f64),
    /// `\xbord(width)` — horizontal-only border width.
    BorderX(f64),
    /// `\ybord(width)` — vertical-only border width.
    BorderY(f64),
    /// `\shad(depth)` — uniform shadow depth.
    Shadow(f64),
    /// `\xshad(depth)` — horizontal-only shadow offset.
    ShadowX(f64),
    /// `\yshad(depth)` — vertical-only shadow offset.
    ShadowY(f64),
    /// `\clip(x1,y1,x2,y2)` — rectangular clip region (content outside is hidden).
    Clip { x1: f64, y1: f64, x2: f64, y2: f64 },
    /// `\iclip(x1,y1,x2,y2)` — inverse rectangular clip (content inside is hidden).
    ClipInverse { x1: f64, y1: f64, x2: f64, y2: f64 },
    /// `\clip(scale, drawing_commands)` — vector path clip from ASS drawing commands.
    ClipDrawing { scale: f32, commands: String },
    /// `\iclip(scale, drawing_commands)` — inverse vector path clip.
    ClipInverseDrawing { scale: f32, commands: String },
    /// `\a[N]` — alignment using legacy SSA numbering (1–11).
    Alignment(u8),
    /// `\an[N]` — alignment using numpad layout (1–9, where 5 = center).
    AlignmentNumpad(u8),
    /// `\q[N]` — wrap style (0=smart, 1=end-of-line, 2=no word wrap, 3=smart with lower line).
    WrapStyle(u8),
    /// `\writing_mode` — text direction (1=horizontal, 2=vertical-right, 3=vertical-left).
    WritingMode(u8),
    /// `\fe[N]` — font charset/encoding index.
    Charset(u8),
    /// `\k`/`\kf`/`\ko`/`\kt[N]` — karaoke timing (duration in centiseconds × 10 = ms).
    Karaoke {
        style: super::karaoke::KaraokeStyle,
        duration: u64,
    },
    /// `\r[name]` — reset to named style (empty string = reset to event's default style).
    Reset(String),
    /// `\r` — reset all override tags to style defaults.
    ResetAll,
    /// `\p[N]` — drawing mode (0=off, 1+=ASS vector drawing commands follow).
    DrawingMode(u8),
    /// `\pbo(offset)` — baseline offset for drawing mode.
    BaselineOffset(f64),
    /// `\org(x,y)` — rotation origin point.
    Origin { x: f64, y: f64 },
    /// `\fax(shear)`/`\fay(shear)` — horizontal/vertical shear factor.
    Shear { x: f64, y: f64 },
    /// Unrecognized override tag (preserved as raw string).
    Unknown(String),
}
