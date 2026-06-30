/// ASS effect type from the 9th field of an event line.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum Effect {
    #[default]
    None,
    /// Horizontal scrolling banner: `Banner;delay;direction;fadeaway`
    Banner {
        delay: u64,
        left_to_right: bool,
        fadeaway: u64,
    },
    /// Vertical scroll up: `Scroll up;delay;top;bottom`
    ScrollUp { delay: u64, top: u64, bottom: u64 },
    /// Vertical scroll down: `Scroll down;delay;top;bottom`
    ScrollDown { delay: u64, top: u64, bottom: u64 },
    /// Karaoke marker.
    Karaoke,
}

/// Parse an ASS effect string (e.g. "fade(255,0,0,255,255,255)") into an Effect enum.
pub fn parse_effect(s: &str) -> Effect {
    let s = s.trim();
    if s.is_empty() {
        return Effect::None;
    }
    if s.eq_ignore_ascii_case("Karaoke") {
        return Effect::Karaoke;
    }
    let parts: Vec<&str> = s.split(';').collect();
    let kw = parts[0].trim();
    let u = |i: usize| {
        parts
            .get(i)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(0)
    };

    match kw {
        _ if kw.eq_ignore_ascii_case("Banner") => Effect::Banner {
            delay: u(1),
            left_to_right: parts.get(2).map(|v| v.trim() == "1").unwrap_or(false),
            fadeaway: u(3),
        },
        _ if kw.eq_ignore_ascii_case("Scroll up") => Effect::ScrollUp {
            delay: u(1),
            top: u(2),
            bottom: u(3),
        },
        _ if kw.eq_ignore_ascii_case("Scroll down") => Effect::ScrollDown {
            delay: u(1),
            top: u(2),
            bottom: u(3),
        },
        _ => Effect::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn none_empty() {
        assert_eq!(parse_effect(""), Effect::None);
    }
    #[test]
    fn karaoke() {
        assert_eq!(parse_effect("Karaoke"), Effect::Karaoke);
    }
    #[test]
    fn karaoke_lc() {
        assert_eq!(parse_effect("karaoke"), Effect::Karaoke);
    }
    #[test]
    fn banner() {
        assert_eq!(
            parse_effect("Banner;8;1;40"),
            Effect::Banner {
                delay: 8,
                left_to_right: true,
                fadeaway: 40
            }
        );
    }
    #[test]
    fn scroll_up() {
        assert_eq!(
            parse_effect("Scroll up;100;50;50"),
            Effect::ScrollUp {
                delay: 100,
                top: 50,
                bottom: 50
            }
        );
    }
    #[test]
    fn scroll_down() {
        assert_eq!(
            parse_effect("Scroll down;150;40;60"),
            Effect::ScrollDown {
                delay: 150,
                top: 40,
                bottom: 60
            }
        );
    }
}
