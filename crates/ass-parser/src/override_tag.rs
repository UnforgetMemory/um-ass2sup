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

pub fn parse_override_tag(s: &str) -> Option<OverrideTag> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s.starts_with("pos(") {
        let inner = s.trim_start_matches("pos(").trim_end_matches(')');
        let nums: Vec<f64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 2 {
            return Some(OverrideTag::Pos { x: nums[0], y: nums[1] });
        }
    }
    if s.starts_with("move(") {
        let inner = s.trim_start_matches("move(").trim_end_matches(')');
        let nums: Vec<f64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 4 {
            let (t1, t2) = if nums.len() >= 6 { (nums[4] as u64, nums[5] as u64) } else { (0, 0) };
            return Some(OverrideTag::Move { x1: nums[0], y1: nums[1], x2: nums[2], y2: nums[3], t1, t2 });
        }
    }
    if s.starts_with("fad(") || s.starts_with("fade(") {
        if s.starts_with("fade(") && s.matches(',').count() >= 6 {
            let inner = s.trim_start_matches("fade(").trim_end_matches(')');
            let nums: Vec<u64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
            if nums.len() >= 7 {
                return Some(OverrideTag::FadeComplex {
                    alpha_start: nums[0] as u8, alpha_mid: nums[1] as u8, alpha_end: nums[2] as u8,
                    t1: nums[3], t2: nums[4], t3: nums[5], t4: nums[6],
                });
            }
        }
        let inner = s.trim_start_matches("fad(").trim_start_matches("fade(").trim_end_matches(')');
        let nums: Vec<u64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 2 {
            return Some(OverrideTag::Fade { duration_in: nums[0], duration_out: nums[1] });
        }
    }
    if s.starts_with("fn") {
        return Some(OverrideTag::FontName(s[2..].to_string()));
    }
    if s.starts_with("fs") {
        if let Ok(size) = s[2..].parse() {
            return Some(OverrideTag::FontSize(size));
        }
    }
    if s == "b1" || s == "b0" || s == "b-1" {
        return Some(OverrideTag::Bold(s == "b1"));
    }
    if s.starts_with("b") {
        if let Ok(weight) = s[1..].parse() {
            return Some(OverrideTag::BoldWeight(weight));
        }
    }
    if s == "i1" || s == "i0" || s == "i-1" {
        return Some(OverrideTag::Italic(s == "i1"));
    }
    if s == "u1" || s == "u0" || s == "u-1" {
        return Some(OverrideTag::Underline(s == "u1"));
    }
    if s == "s1" || s == "s0" || s == "s-1" {
        return Some(OverrideTag::Strikeout(s == "s1"));
    }
    if s.starts_with("an") {
        if let Ok(align) = s[2..].parse::<u8>() {
            if (1..=9).contains(&align) {
                return Some(OverrideTag::AlignmentNumpad(align));
            }
        }
    }
    if s.starts_with("a") && !s.starts_with("an") {
        if let Ok(align) = s[1..].parse::<u8>() {
            return Some(OverrideTag::Alignment(align));
        }
    }
    if s.starts_with("k") || s.starts_with("kf") || s.starts_with("ko") || s.starts_with("kt") {
        let (tag_str, dur_str) = if s.starts_with("kf") { ("kf", &s[2..]) }
            else if s.starts_with("ko") { ("ko", &s[2..]) }
            else if s.starts_with("kt") { ("kt", &s[2..]) }
            else { ("k", &s[1..]) };
        if let Some(style) = super::karaoke::KaraokeStyle::from_tag(tag_str) {
            if let Ok(dur) = dur_str.parse::<u64>() {
                return Some(OverrideTag::Karaoke { style, duration: dur * 10 });
            }
        }
    }
    if s.starts_with("t(") {
        let inner = s.trim_start_matches("t(").trim_end_matches(')');
        let parts: Vec<&str> = inner.split(',').collect();
        if !parts.is_empty() {
            let tag = parts[0].trim().to_string();
            let t1 = parts.get(1).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
            let t2 = parts.get(2).and_then(|v| v.trim().parse().ok()).unwrap_or(t1);
            let accel = parts.get(3).and_then(|v| v.trim().parse().ok()).unwrap_or(1.0);
            return Some(OverrideTag::Transform { tag, t1, t2, accel });
        }
    }
    if s.starts_with("clip(") {
        let inner = s.trim_start_matches("clip(").trim_end_matches(')');
        let nums: Vec<f64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 4 {
            return Some(OverrideTag::Clip { x1: nums[0], y1: nums[1], x2: nums[2], y2: nums[3] });
        }
    }
    if s.starts_with("iclip(") {
        let inner = s.trim_start_matches("iclip(").trim_end_matches(')');
        let nums: Vec<f64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 4 {
            return Some(OverrideTag::ClipInverse { x1: nums[0], y1: nums[1], x2: nums[2], y2: nums[3] });
        }
    }
    if s.starts_with("org(") {
        let inner = s.trim_start_matches("org(").trim_end_matches(')');
        let nums: Vec<f64> = inner.split(',').filter_map(|n| n.trim().parse().ok()).collect();
        if nums.len() >= 2 {
            return Some(OverrideTag::Origin { x: nums[0], y: nums[1] });
        }
    }
    if s.starts_with("frz") {
        if let Ok(z) = s[3..].parse::<f64>() {
            return Some(OverrideTag::Rotation { x: 0.0, y: 0.0, z });
        }
    }
    if s.starts_with("frx") {
        if let Ok(x) = s[3..].parse::<f64>() {
            return Some(OverrideTag::Rotation { x, y: 0.0, z: 0.0 });
        }
    }
    if s.starts_with("fry") {
        if let Ok(y) = s[3..].parse::<f64>() {
            return Some(OverrideTag::Rotation { x: 0.0, y, z: 0.0 });
        }
    }
    if s.starts_with("fr") {
        if let Ok(z) = s[2..].parse::<f64>() {
            return Some(OverrideTag::Rotation { x: 0.0, y: 0.0, z });
        }
    }
    if s.starts_with("fscx") {
        if let Ok(x) = s[4..].parse::<f64>() {
            return Some(OverrideTag::Scale { x, y: 100.0 });
        }
    }
    if s.starts_with("fscy") {
        if let Ok(y) = s[4..].parse::<f64>() {
            return Some(OverrideTag::Scale { x: 100.0, y });
        }
    }
    if s.starts_with("fax") {
        if let Ok(x) = s[3..].parse::<f64>() {
            return Some(OverrideTag::Shear { x, y: 0.0 });
        }
    }
    if s.starts_with("fay") {
        if let Ok(y) = s[3..].parse::<f64>() {
            return Some(OverrideTag::Shear { x: 0.0, y });
        }
    }
    if s.starts_with("fsp") {
        if let Ok(v) = s[3..].parse::<f64>() {
            return Some(OverrideTag::Spacing(v));
        }
    }
    if s.starts_with("bord") {
        if let Ok(w) = s[4..].parse::<f64>() {
            return Some(OverrideTag::Border(w));
        }
    }
    if s.starts_with("shad") {
        if let Ok(d) = s[4..].parse::<f64>() {
            return Some(OverrideTag::Shadow(d));
        }
    }
    if s.starts_with("xbord") {
        if let Ok(w) = s[5..].parse::<f64>() {
            return Some(OverrideTag::BorderX(w));
        }
    }
    if s.starts_with("ybord") {
        if let Ok(w) = s[5..].parse::<f64>() {
            return Some(OverrideTag::BorderY(w));
        }
    }
    if s.starts_with("xshad") {
        if let Ok(d) = s[5..].parse::<f64>() {
            return Some(OverrideTag::ShadowX(d));
        }
    }
    if s.starts_with("yshad") {
        if let Ok(d) = s[5..].parse::<f64>() {
            return Some(OverrideTag::ShadowY(d));
        }
    }
    if s.starts_with("be") {
        if let Ok(v) = s[2..].parse::<f64>() {
            return Some(OverrideTag::Blur(v));
        }
    }
    if s.starts_with("blur") {
        if let Ok(v) = s[4..].parse::<f64>() {
            return Some(OverrideTag::GaussianBlur(v));
        }
    }
    if s.starts_with("q") {
        if let Ok(v) = s[1..].parse::<u8>() {
            if v <= 3 {
                return Some(OverrideTag::WrapStyle(v));
            }
        }
    }
    if s.starts_with("p") && !s.starts_with("pos") && !s.starts_with("pbo") {
        if let Ok(v) = s[1..].parse::<u8>() {
            return Some(OverrideTag::DrawingMode(v));
        }
    }
    if s.starts_with("pbo") {
        if let Ok(v) = s[3..].parse::<f64>() {
            return Some(OverrideTag::BaselineOffset(v));
        }
    }
    for (prefix, variant) in [("1c", "primary"), ("2c", "secondary"), ("3c", "outline"), ("4c", "shadow")] {
        if s.starts_with(prefix) {
            let color_str = &s[prefix.len()..];
            if let Ok(color) = parse_ass_color(color_str) {
                return Some(match variant {
                    "primary" => OverrideTag::PrimaryColor(color),
                    "secondary" => OverrideTag::SecondaryColor(color),
                    "outline" => OverrideTag::OutlineColor(color),
                    "shadow" => OverrideTag::ShadowColor(color),
                    _ => unreachable!(),
                });
            }
        }
    }
    if s.starts_with("alpha") {
        let val_str = &s[5..];
        if let Ok(v) = parse_hex_u8(val_str) {
            return Some(OverrideTag::Alpha { value: v });
        }
    }
    for (prefix, variant) in [("1a", "primary"), ("2a", "secondary"), ("3a", "outline"), ("4a", "shadow")] {
        if s.starts_with(prefix) {
            let val_str = &s[prefix.len()..];
            if let Ok(v) = parse_hex_u8(val_str) {
                return Some(match variant {
                    "primary" => OverrideTag::PrimaryAlpha { value: v },
                    "secondary" => OverrideTag::SecondaryAlpha { value: v },
                    "outline" => OverrideTag::OutlineAlpha { value: v },
                    "shadow" => OverrideTag::ShadowAlpha { value: v },
                    _ => unreachable!(),
                });
            }
        }
    }
    if s.starts_with("r") && !s.starts_with("reset") {
        return Some(OverrideTag::Reset(s[1..].to_string()));
    }
    if s.starts_with("fe") {
        if let Ok(v) = s[2..].parse::<u8>() {
            return Some(OverrideTag::Charset(v));
        }
    }
    None
}

fn parse_hex_u8(s: &str) -> Result<u8, std::num::ParseIntError> {
    let s = s.trim().trim_start_matches("H").trim_start_matches("h").trim_end_matches('&');
    u8::from_str_radix(s, 16)
}

fn parse_ass_color(s: &str) -> Result<super::color::AssColor, ()> {
    let s = s.trim().trim_start_matches("H").trim_start_matches("h").trim_end_matches('&');
    if s.len() < 6 { return Err(()); }
    let hex = if s.len() >= 8 { &s[s.len()-8..] } else { s };
    let parse = |range: &str| u8::from_str_radix(range, 16).map_err(|_| ());
    if hex.len() == 8 {
        let alpha = parse(&hex[0..2])?;
        let blue = parse(&hex[2..4])?;
        let green = parse(&hex[4..6])?;
        let red = parse(&hex[6..8])?;
        Ok(super::color::AssColor { alpha, blue, green, red })
    } else if hex.len() == 6 {
        let blue = parse(&hex[0..2])?;
        let green = parse(&hex[2..4])?;
        let red = parse(&hex[4..6])?;
        Ok(super::color::AssColor { alpha: 0, blue, green, red })
    } else {
        Err(())
    }
}
