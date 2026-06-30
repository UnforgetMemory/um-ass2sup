//! Position tags: `\pos(x,y)`, `\move(x1,y1,x2,y2[,t1,t2])`, `\org(x,y)`.
use super::util::nums_f64;
use crate::OverrideTag;

/// Parse \a or \an alignment tag.
pub fn parse(s: &str) -> Option<OverrideTag> {
    if s.starts_with("pos(") {
        let n = nums_f64(s, "pos(");
        if n.len() >= 2 {
            return Some(OverrideTag::Pos { x: n[0], y: n[1] });
        }
    }
    if s.starts_with("move(") {
        let n = nums_f64(s, "move(");
        if n.len() >= 4 {
            let (t1, t2) = if n.len() >= 6 {
                (n[4] as u64, n[5] as u64)
            } else {
                (0, 0)
            };
            let (t1, t2) = if t1 > t2 { (t2, t1) } else { (t1, t2) }; // libass: swap
            return Some(OverrideTag::Move {
                x1: n[0],
                y1: n[1],
                x2: n[2],
                y2: n[3],
                t1,
                t2,
            });
        }
    }
    if s.starts_with("org(") {
        let n = nums_f64(s, "org(");
        if n.len() >= 2 {
            return Some(OverrideTag::Origin { x: n[0], y: n[1] });
        }
    }
    None
}
