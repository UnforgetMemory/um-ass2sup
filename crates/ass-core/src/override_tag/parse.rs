//! Main override tag dispatch — routes each tag to its category parser.

use super::util::split_tags;
use super::{border, clip, color, effect, font, geometry, karaoke, position};
use crate::{KaraokeSegment, OverrideTag, TaggedOverride};

/// Parse a single `\`-delimited tag segment. Returns `None` for unknown tags.
pub fn parse_one_tag(s: &str) -> Option<OverrideTag> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if s == "!" {
        return Some(OverrideTag::AnimationSkip);
    }

    // Dispatch by category — order matters for prefix overlap:
    // p → pbo (check pbo before general p)
    // a → an (check an before a)
    // fr → frx, fry, frz (check longer prefixes first)
    // fsc → fscx, fscy (check longer prefixes first)
    // k → kf, ko, kt (check karaoke before other k uses)
    if let Some(tag) = position::parse(s) {
        return Some(tag);
    }
    if let Some(tag) = karaoke::parse(s) {
        return Some(tag);
    }
    if let Some(tag) = font::parse(s) {
        return Some(tag);
    }
    if let Some(tag) = color::parse(s) {
        return Some(tag);
    }
    if let Some(tag) = geometry::parse(s) {
        return Some(tag);
    }
    if let Some(tag) = border::parse(s) {
        return Some(tag);
    }
    if let Some(tag) = clip::parse(s) {
        return Some(tag);
    }
    if let Some(tag) = effect::parse(s) {
        return Some(tag);
    }

    None
}

/// Parse override tags and karaoke segments from `{...}` blocks in text.
pub fn parse_tags(text: &str) -> (Vec<TaggedOverride>, Vec<KaraokeSegment>) {
    let mut tags = Vec::new();
    let mut karaoke = Vec::new();
    let mut in_block = false;
    let mut buf = String::new();

    let mut pending_karaoke: Option<(crate::KaraokeStyle, u64)> = None;
    let mut syllable = String::new();
    let mut seg_idx = 0usize;

    for c in text.chars() {
        if c == '{' {
            in_block = true;
            buf.clear();
            continue;
        }
        if c == '}' {
            in_block = false;
            for seg in split_tags(&buf) {
                if let Some(tag) = parse_one_tag(&seg) {
                    if let crate::OverrideTag::Karaoke { style, duration } = &tag {
                        if let Some((ps, pd)) = pending_karaoke.take() {
                            karaoke.push(crate::KaraokeSegment::new(
                                ps,
                                pd,
                                std::mem::take(&mut syllable),
                                seg_idx,
                            ));
                            seg_idx += 1;
                        }
                        pending_karaoke = Some((*style, *duration));
                    }
                    tags.push(TaggedOverride { tag, span: None });
                }
            }
            buf.clear();
            continue;
        }
        if in_block {
            buf.push(c);
        } else if pending_karaoke.is_some() {
            syllable.push(c);
        }
    }
    if let Some((style, duration)) = pending_karaoke.take() {
        karaoke.push(crate::KaraokeSegment::new(
            style,
            duration,
            std::mem::take(&mut syllable),
            seg_idx,
        ));
    }
    (tags, karaoke)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KaraokeStyle, OverrideTag};

    // ── Position ──
    #[test]
    fn pos() {
        assert!(matches!(
            parse_one_tag("pos(100,200)"),
            Some(OverrideTag::Pos { x: 100.0, y: 200.0 })
        ));
    }
    #[test]
    fn move_4args() {
        assert!(matches!(
            parse_one_tag("move(0,0,100,200)"),
            Some(OverrideTag::Move { .. })
        ));
    }
    #[test]
    fn move_6args_swap() {
        let t = parse_one_tag("move(0,0,100,200,500,100)");
        if let Some(OverrideTag::Move { t1, t2, .. }) = t {
            assert_eq!((t1, t2), (100, 500));
        } else {
            panic!();
        }
    }

    // ── Font ──
    #[test]
    fn bold_on() {
        assert_eq!(parse_one_tag("b1"), Some(OverrideTag::Bold(true)));
    }
    #[test]
    fn bold_off() {
        assert_eq!(parse_one_tag("b0"), Some(OverrideTag::Bold(false)));
    }
    #[test]
    fn bold_neg() {
        assert_eq!(parse_one_tag("b-1"), Some(OverrideTag::Bold(true)));
    }
    #[test]
    fn bold_weight() {
        assert_eq!(parse_one_tag("b700"), Some(OverrideTag::BoldWeight(700)));
    }
    #[test]
    fn italic_on() {
        assert_eq!(parse_one_tag("i1"), Some(OverrideTag::Italic(true)));
    }
    #[test]
    fn italic_off() {
        assert_eq!(parse_one_tag("i0"), Some(OverrideTag::Italic(false)));
    }
    #[test]
    fn font_name() {
        assert_eq!(
            parse_one_tag("fnArial"),
            Some(OverrideTag::FontName("Arial".into()))
        );
    }
    #[test]
    fn font_size() {
        assert_eq!(parse_one_tag("fs24"), Some(OverrideTag::FontSize(24.0)));
    }

    // ── Karaoke ──
    #[test]
    fn karaoke_k() {
        assert!(matches!(
            parse_one_tag("k50"),
            Some(OverrideTag::Karaoke {
                style: KaraokeStyle::Instant,
                duration: 500
            })
        ));
    }
    #[test]
    fn karaoke_k_upper() {
        assert!(matches!(
            parse_one_tag("K50"),
            Some(OverrideTag::Karaoke {
                style: KaraokeStyle::Fill,
                duration: 500
            })
        ));
    }

    // ── Colour ──
    #[test]
    fn colour_1c() {
        assert!(matches!(
            parse_one_tag("1c&HFF0000&"),
            Some(OverrideTag::PrimaryColor(_))
        ));
    }
    #[test]
    fn colour_c_alias() {
        assert!(matches!(
            parse_one_tag("c&HFF0000&"),
            Some(OverrideTag::PrimaryColor(_))
        ));
    }
    #[test]
    fn alpha_tag() {
        assert!(matches!(
            parse_one_tag("alpha&H80&"),
            Some(OverrideTag::Alpha { value: 128 })
        ));
    }

    // ── Clip ──
    #[test]
    fn clip_rect() {
        assert!(matches!(
            parse_one_tag("clip(10,20,30,40)"),
            Some(OverrideTag::Clip { .. })
        ));
    }
    #[test]
    fn clip_drawing() {
        assert!(matches!(
            parse_one_tag("clip(1,m 0 0 l 100 0)"),
            Some(OverrideTag::ClipDrawing { scale: 1.0, .. })
        ));
    }
    #[test]
    fn clip_at() {
        assert_eq!(
            parse_one_tag("clip(@)"),
            Some(OverrideTag::ClipDrawingCurrent)
        );
    }
    #[test]
    fn iclip_rect() {
        assert!(matches!(
            parse_one_tag("iclip(10,20,30,40)"),
            Some(OverrideTag::ClipInverse { .. })
        ));
    }

    // ── Transform ──
    #[test]
    fn transform_parse() {
        assert!(matches!(
            parse_one_tag("t(\\b1,0,1000,1)"),
            Some(OverrideTag::Transform { .. })
        ));
    }
    #[test]
    fn transform_nested() {
        assert!(matches!(
            parse_one_tag("t(\\pos(100,200),0,500,1)"),
            Some(OverrideTag::Transform { .. })
        ));
    }

    // ── Alignment ──
    #[test]
    fn align_an() {
        assert_eq!(parse_one_tag("an5"), Some(OverrideTag::AlignmentNumpad(5)));
    }
    #[test]
    fn align_vsfilter() {
        assert_eq!(parse_one_tag("a4"), Some(OverrideTag::AlignmentVsfilter(5)));
    }

    // ── Geometry ──
    #[test]
    fn rotate_frz() {
        assert!(matches!(
            parse_one_tag("frz(45)"),
            Some(OverrideTag::Rotation { z: 45.0, .. })
        ));
    }
    #[test]
    fn scale_x() {
        assert!(matches!(
            parse_one_tag("fscx(150)"),
            Some(OverrideTag::Scale { x: 150.0, .. })
        ));
    }
    #[test]
    fn spacing() {
        assert_eq!(parse_one_tag("fsp(5)"), Some(OverrideTag::Spacing(5.0)));
    }

    // ── Border / Shadow ──
    #[test]
    fn border_bord() {
        assert_eq!(
            parse_one_tag("bord(3)"),
            Some(OverrideTag::Border { x: 3.0, y: 3.0 })
        );
    }
    #[test]
    fn border_xbord() {
        assert_eq!(parse_one_tag("xbord(2)"), Some(OverrideTag::BorderX(2.0)));
    }
    #[test]
    fn shadow_shad() {
        assert_eq!(
            parse_one_tag("shad(4)"),
            Some(OverrideTag::Shadow { x: 4.0, y: 4.0 })
        );
    }

    // ── Edge ──
    #[test]
    fn empty() {
        assert_eq!(parse_one_tag(""), None);
    }
    #[test]
    fn animation_skip() {
        assert_eq!(parse_one_tag("!"), Some(OverrideTag::AnimationSkip));
    }
    #[test]
    fn reset_all() {
        assert_eq!(parse_one_tag("r"), Some(OverrideTag::ResetAll));
    }
    #[test]
    fn reset_named() {
        assert_eq!(
            parse_one_tag("rDefault"),
            Some(OverrideTag::Reset("Default".into()))
        );
    }
    #[test]
    fn drawing_positive() {
        assert_eq!(parse_one_tag("p1"), Some(OverrideTag::DrawingMode(1)));
    }
    #[test]
    fn drawing_negative() {
        assert_eq!(parse_one_tag("p-1"), Some(OverrideTag::DrawingMode(0)));
    }
    #[test]
    fn writing_mode() {
        assert_eq!(
            parse_one_tag("writing_mode(2)"),
            Some(OverrideTag::WritingMode(2))
        );
    }
    #[test]
    fn wrap_style() {
        assert_eq!(parse_one_tag("q2"), Some(OverrideTag::WrapStyle(2)));
    }
    #[test]
    fn charset() {
        assert_eq!(parse_one_tag("fe128"), Some(OverrideTag::Charset(128)));
    }
    #[test]
    fn unknown() {
        assert_eq!(parse_one_tag("unknown"), None);
    }
    #[test]
    fn fsc_reset() {
        assert_eq!(parse_one_tag("fsc"), Some(OverrideTag::ScaleReset));
    }
    #[test]
    fn fn0_reset() {
        assert_eq!(
            parse_one_tag("fn0"),
            Some(OverrideTag::FontName(String::new()))
        );
    }
    #[test]
    fn shear_x() {
        assert!(matches!(
            parse_one_tag("fax(0.5)"),
            Some(OverrideTag::Shear { x: 0.5, .. })
        ));
    }
    #[test]
    fn blur_be() {
        assert_eq!(parse_one_tag("be(3)"), Some(OverrideTag::Blur(3.0)));
    }
    #[test]
    fn fad() {
        assert!(matches!(
            parse_one_tag("fad(100,200)"),
            Some(OverrideTag::Fade { .. })
        ));
    }

    // ── Tag extraction ──
    #[test]
    fn basic_extraction() {
        let (tags, _) = parse_tags("{\\b1\\fs20}BoldSmall");
        assert!(tags
            .iter()
            .any(|t| matches!(t.tag, OverrideTag::Bold(true))));
    }
    #[test]
    fn karaoke_extraction() {
        let (_, kara) = parse_tags("{\\k50}Hel{\\k100}lo");
        assert_eq!(kara.len(), 2);
        assert_eq!(kara[0].duration_ms, 500);
    }
}
