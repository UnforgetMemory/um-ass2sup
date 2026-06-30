//! Karaoke tags: `\k`, `\kf`, `\K` (uppercase = \kf), `\ko`, `\kt`.
use crate::{KaraokeStyle, OverrideTag};

/// Parse karaoke tags: \k, \kf, \ko, \kt.
pub fn parse(s: &str) -> Option<OverrideTag> {
    let lower = s.to_lowercase();
    if let Some(rest) = lower.strip_prefix("k") {
        let is_upper_k = s.starts_with('K');
        let (tag, num_str) = if let Some(r) = rest.strip_prefix('f') {
            ("kf", r)
        } else if let Some(r) = rest.strip_prefix('o') {
            ("ko", r)
        } else if let Some(r) = rest.strip_prefix('t') {
            ("kt", r)
        } else if is_upper_k {
            ("kf", rest)
        }
        // \K = \kf
        else {
            ("k", rest)
        };
        if let Some(style) = KaraokeStyle::from_tag(tag) {
            if let Ok(dur) = num_str.parse::<u64>() {
                return Some(OverrideTag::Karaoke {
                    style,
                    duration: dur * 10,
                });
            }
        }
    }
    None
}
