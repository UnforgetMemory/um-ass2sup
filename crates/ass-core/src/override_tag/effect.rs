//! Effect/animation tags: `\fad`, `\fade`, `\t` (transform).
use super::util::{nums_u64, paren_body, split_args};
use crate::OverrideTag;

/// Parse \fad (fade in/out) or \fade (fade in/out with alpha) tag.
pub fn parse(s: &str) -> Option<OverrideTag> {
    // Fade
    if s.starts_with("fad(") || s.starts_with("fade(") {
        if s.starts_with("fade(") && s.matches(',').count() >= 6 {
            let n = nums_u64(s, "fade(");
            if n.len() >= 7 {
                return Some(OverrideTag::FadeComplex {
                    alpha_start: n[0] as u8,
                    alpha_mid: n[1] as u8,
                    alpha_end: n[2] as u8,
                    t1: n[3],
                    t2: n[4],
                    t3: n[5],
                    t4: n[6],
                });
            }
        }
        let n = if s.starts_with("fad(") {
            nums_u64(s, "fad(")
        } else {
            nums_u64(s, "fade(")
        };
        if n.len() >= 2 {
            return Some(OverrideTag::Fade {
                duration_in: n[0],
                duration_out: n[1],
            });
        }
    }
    // Transform \t(...)
    if s.starts_with("t(") {
        let inner = paren_body(s, "t(");
        let parts: Vec<&str> = split_args(inner);
        let nargs = parts.len();
        if nargs > 0 {
            let has_backslash = parts.iter().any(|p| p.contains('\\'));
            if !has_backslash {
                // No backslash args: all parts are timing parameters, no inner tag
                let t1 = parts
                    .first()
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(0);
                let t2 = parts
                    .get(1)
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(t1);
                let accel = parts
                    .get(2)
                    .and_then(|v| v.trim().parse().ok())
                    .unwrap_or(1.0);
                return Some(OverrideTag::Transform {
                    tag: String::new(),
                    t1,
                    t2,
                    accel,
                });
            }
            // First arg starts with \ → backslash-arg: cnt=0, all defaults, tag=whole
            if parts[0].trim().starts_with('\\') {
                let remaining = inner.trim_end_matches(')').trim();
                return Some(OverrideTag::Transform {
                    tag: remaining.to_string(),
                    t1: 0,
                    t2: 0,
                    accel: 1.0,
                });
            }
            // Libass cnt-based parsing: last arg is inner tag, preceding are timing
            let cnt = nargs - 1;
            let (t1, t2, accel) = if cnt >= 3 && nargs >= 4 {
                (
                    parts[0].trim().parse().unwrap_or(0),
                    parts[1].trim().parse().unwrap_or(0),
                    parts[2].trim().parse().unwrap_or(1.0),
                )
            } else if cnt == 2 && nargs >= 3 {
                (
                    parts[0].trim().parse().unwrap_or(0),
                    parts[1].trim().parse().unwrap_or(0),
                    1.0,
                )
            } else if cnt == 1 && nargs >= 2 {
                (0u64, 0u64, parts[0].trim().parse().unwrap_or(1.0))
            } else {
                (0u64, 0u64, 1.0)
            };
            let tag = parts[cnt.min(nargs - 1)].trim().to_string();
            return Some(OverrideTag::Transform { tag, t1, t2, accel });
        }
    }
    None
}
