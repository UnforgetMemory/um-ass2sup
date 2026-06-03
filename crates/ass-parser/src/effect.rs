/// ASS effect type parsed from the Effect field of an event line.
///
/// In ASS files, the 9th comma-separated field of a `Dialogue` line is the
/// Effect field. It can be empty, `"Karaoke"`, or have a structured format
/// for scrolling/banner effects:
///
/// | Format | Meaning |
/// |---|---|
/// | `""` | No effect |
/// | `"Banner;N;direction;fadeaway"` | Scrolling banner |
/// | `"Scroll up;N;top;bottom"` | Vertical scroll up |
/// | `"Scroll down;N;top;bottom"` | Vertical scroll down |
/// | `"Karaoke"` | Karaoke marker |
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Effect {
    /// No effect.
    #[default]
    None,
    /// Horizontal scrolling banner.
    ///
    /// Fields: `Banner;{delay_per_pixels};{left-to-right};{fadeaway_width}`
    /// - `delay_per_pixel`: delay per pixel in milliseconds
    /// - `left_to_right`: `true` for left-to-right, `false` for right-to-left
    /// - `fadeaway_width`: fade edge width in pixels
    Banner {
        /// Delay per pixel in milliseconds.
        delay_per_pixel: u64,
        /// Scroll direction — `true` = left-to-right, `false` = right-to-left.
        left_to_right: bool,
        /// Width of the fade edge in pixels.
        fadeaway_width: f64,
    },
    /// Vertical scrolling up effect.
    ///
    /// Fields: `Scroll up;{delay_per_row};{top_offset};{bottom_offset}`
    /// - `delay_per_row`: delay per text row in milliseconds
    /// - `top_offset`: top margin in pixels
    /// - `bottom_offset`: bottom margin in pixels
    ScrollUp {
        /// Delay per text row in milliseconds.
        delay_per_row: u64,
        /// Top margin offset in pixels.
        top_offset: f64,
        /// Bottom margin offset in pixels.
        bottom_offset: f64,
    },
    /// Vertical scrolling down effect.
    ///
    /// Fields: `Scroll down;{delay_per_row};{top_offset};{bottom_offset}`
    /// - `delay_per_row`: delay per text row in milliseconds
    /// - `top_offset`: top margin in pixels
    /// - `bottom_offset`: bottom margin in pixels
    ScrollDown {
        /// Delay per text row in milliseconds.
        delay_per_row: u64,
        /// Top margin offset in pixels.
        top_offset: f64,
        /// Bottom margin offset in pixels.
        bottom_offset: f64,
    },
    /// Karaoke effect marker.
    ///
    /// When present, the event contains `\k` / `\kf` / `\ko` tags for
    /// syllable-by-syllable timing (already parsed by [`KaraokeSegment`]).
    Karaoke,
}

/// Parses an ASS effect string into an [`Effect`] value.
///
/// Returns [`Effect::None`] for empty strings, unrecognized formats, or
/// malformed parameters (individual fields fall back to sensible defaults).
///
/// # Format
///
/// See the [`Effect`] documentation for the expected string formats.
///
/// # Examples
///
/// ```
/// use ass_parser::effect::{Effect, parse_effect};
///
/// assert_eq!(parse_effect(""), Effect::None);
/// assert_eq!(parse_effect("Karaoke"), Effect::Karaoke);
///
/// let banner = parse_effect("Banner;8;1;40");
/// assert_eq!(banner, Effect::Banner { delay_per_pixel: 8, left_to_right: true, fadeaway_width: 40.0 });
/// ```
pub fn parse_effect(s: &str) -> Effect {
    let s = s.trim();
    if s.is_empty() {
        return Effect::None;
    }

    // "Karaoke" — exact match (case-insensitive per ASS spec)
    if s.eq_ignore_ascii_case("Karaoke") {
        return Effect::Karaoke;
    }

    // Split by semicolon — the ASS effect delimiter
    let parts: Vec<&str> = s.split(';').collect();
    let keyword = parts[0].trim();

    match keyword {
        kw if kw.eq_ignore_ascii_case("Banner") => {
            let delay_per_pixel = parts.get(1).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
            let left_to_right = matches!(parts.get(2).map(|v| v.trim()), Some("1"));
            let fadeaway_width = parts.get(3).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
            Effect::Banner { delay_per_pixel, left_to_right, fadeaway_width }
        }
        kw if kw.eq_ignore_ascii_case("Scroll up") => {
            let delay_per_row = parts.get(1).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
            let top_offset = parts.get(2).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
            let bottom_offset = parts.get(3).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
            Effect::ScrollUp { delay_per_row, top_offset, bottom_offset }
        }
        kw if kw.eq_ignore_ascii_case("Scroll down") => {
            let delay_per_row = parts.get(1).and_then(|v| v.trim().parse().ok()).unwrap_or(0);
            let top_offset = parts.get(2).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
            let bottom_offset = parts.get(3).and_then(|v| v.trim().parse().ok()).unwrap_or(0.0);
            Effect::ScrollDown { delay_per_row, top_offset, bottom_offset }
        }
        _ => Effect::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- None ---

    #[test]
    fn parse_empty_string() {
        assert_eq!(parse_effect(""), Effect::None);
    }

    #[test]
    fn parse_whitespace_string() {
        assert_eq!(parse_effect("   "), Effect::None);
    }

    #[test]
    fn parse_garbage_string() {
        assert_eq!(parse_effect("SomeGarbage"), Effect::None);
    }

    #[test]
    fn parse_garbage_with_semicolons() {
        assert_eq!(parse_effect("Foo;bar;baz"), Effect::None);
    }

    // --- Karaoke ---

    #[test]
    fn parse_karaoke() {
        assert_eq!(parse_effect("Karaoke"), Effect::Karaoke);
    }

    #[test]
    fn parse_karaoke_lowercase() {
        assert_eq!(parse_effect("karaoke"), Effect::Karaoke);
    }

    #[test]
    fn parse_karaoke_mixed_case() {
        assert_eq!(parse_effect("KARAOKE"), Effect::Karaoke);
    }

    #[test]
    fn parse_karaoke_trimmed() {
        assert_eq!(parse_effect("  Karaoke  "), Effect::Karaoke);
    }

    // --- Banner ---

    #[test]
    fn parse_banner_full() {
        assert_eq!(
            parse_effect("Banner;8;1;40"),
            Effect::Banner { delay_per_pixel: 8, left_to_right: true, fadeaway_width: 40.0 }
        );
    }

    #[test]
    fn parse_banner_right_to_left() {
        assert_eq!(
            parse_effect("Banner;5;0;20"),
            Effect::Banner { delay_per_pixel: 5, left_to_right: false, fadeaway_width: 20.0 }
        );
    }

    #[test]
    fn parse_banner_no_fadeaway() {
        assert_eq!(
            parse_effect("Banner;10;1;0"),
            Effect::Banner { delay_per_pixel: 10, left_to_right: true, fadeaway_width: 0.0 }
        );
    }

    #[test]
    fn parse_banner_missing_parts() {
        // Only "Banner" keyword, rest default
        assert_eq!(
            parse_effect("Banner"),
            Effect::Banner { delay_per_pixel: 0, left_to_right: false, fadeaway_width: 0.0 }
        );
    }

    #[test]
    fn parse_banner_partial_parts() {
        assert_eq!(
            parse_effect("Banner;100;1"),
            Effect::Banner { delay_per_pixel: 100, left_to_right: true, fadeaway_width: 0.0 }
        );
    }

    #[test]
    fn parse_banner_non_numeric_speed() {
        assert_eq!(
            parse_effect("Banner;fast;1;40"),
            Effect::Banner { delay_per_pixel: 0, left_to_right: true, fadeaway_width: 40.0 }
        );
    }

    #[test]
    fn parse_banner_float_delay() {
        // delay_per_pixel is u64; "8.5" fails to parse so defaults to 0
        // remaining fields parse independently
        assert_eq!(
            parse_effect("Banner;8.5;1;40"),
            Effect::Banner { delay_per_pixel: 0, left_to_right: true, fadeaway_width: 40.0 }
        );
    }

    // --- Scroll up ---

    #[test]
    fn parse_scroll_up_full() {
        assert_eq!(
            parse_effect("Scroll up;100;50;50"),
            Effect::ScrollUp { delay_per_row: 100, top_offset: 50.0, bottom_offset: 50.0 }
        );
    }

    #[test]
    fn parse_scroll_up_lowercase() {
        assert_eq!(
            parse_effect("scroll up;100;50;50"),
            Effect::ScrollUp { delay_per_row: 100, top_offset: 50.0, bottom_offset: 50.0 }
        );
    }

    #[test]
    fn parse_scroll_up_different_offsets() {
        assert_eq!(
            parse_effect("Scroll up;200;30;80"),
            Effect::ScrollUp { delay_per_row: 200, top_offset: 30.0, bottom_offset: 80.0 }
        );
    }

    #[test]
    fn parse_scroll_up_zero_offsets() {
        assert_eq!(
            parse_effect("Scroll up;50;0;0"),
            Effect::ScrollUp { delay_per_row: 50, top_offset: 0.0, bottom_offset: 0.0 }
        );
    }

    #[test]
    fn parse_scroll_up_float_offsets() {
        assert_eq!(
            parse_effect("Scroll up;100;50.5;60.25"),
            Effect::ScrollUp { delay_per_row: 100, top_offset: 50.5, bottom_offset: 60.25 }
        );
    }

    #[test]
    fn parse_scroll_up_missing_delay() {
        assert_eq!(
            parse_effect("Scroll up;"),
            Effect::ScrollUp { delay_per_row: 0, top_offset: 0.0, bottom_offset: 0.0 }
        );
    }

    // --- Scroll down ---

    #[test]
    fn parse_scroll_down_full() {
        assert_eq!(
            parse_effect("Scroll down;150;40;60"),
            Effect::ScrollDown { delay_per_row: 150, top_offset: 40.0, bottom_offset: 60.0 }
        );
    }

    #[test]
    fn parse_scroll_down_mixed_case() {
        assert_eq!(
            parse_effect("SCROLL DOWN;120;30;30"),
            Effect::ScrollDown { delay_per_row: 120, top_offset: 30.0, bottom_offset: 30.0 }
        );
    }

    #[test]
    fn parse_scroll_down_extra_semicolons() {
        // Trailing ; should be ignored gracefully
        let result = parse_effect("Scroll down;100;50;50;extra");
        // extra part after 3rd value is ignored since we just use get(3)
        assert_eq!(
            result,
            Effect::ScrollDown { delay_per_row: 100, top_offset: 50.0, bottom_offset: 50.0 }
        );
    }

    // --- Default ---

    #[test]
    fn default_is_none() {
        assert_eq!(Effect::default(), Effect::None);
    }

    // --- Debug / Display / Clone ---

    #[test]
    fn debug_format() {
        let effect = Effect::Banner { delay_per_pixel: 8, left_to_right: true, fadeaway_width: 40.0 };
        let debug = format!("{:?}", effect);
        assert!(debug.contains("Banner"));
        assert!(debug.contains("8"));
    }

    #[test]
    fn clone_equality() {
        let effect = Effect::ScrollUp { delay_per_row: 100, top_offset: 50.0, bottom_offset: 50.0 };
        assert_eq!(effect.clone(), effect);
    }

    #[test]
    fn partial_eq_none() {
        assert_ne!(Effect::Karaoke, Effect::None);
        assert_ne!(Effect::Banner { delay_per_pixel: 0, left_to_right: false, fadeaway_width: 0.0 }, Effect::None);
    }
}
