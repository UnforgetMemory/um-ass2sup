//! Utility functions shared across the conversion pipeline.

use ass_core::{Event, OverrideTag};

/// Compute the optimal render timestamp for an event, adjusted for fade effects.
///
/// PGS subtitles are static — they cannot animate alpha.  For `\fad(in, out)`
/// we shift the render point to `start + in` so the fade-in has completed.
pub fn compute_render_pts(event: &Event) -> u64 {
    const VISIBLE_ALPHA: u8 = 128;

    let start_ms = event.start_ms;
    let end_ms = event.end_ms;

    let mut fade_render_pt: Option<u64> = None;

    for to in &event.override_tags {
        match &to.tag {
            OverrideTag::Fade { duration_in, .. } => {
                if *duration_in > 0 {
                    fade_render_pt = Some(start_ms.saturating_add(*duration_in).min(end_ms));
                }
            }
            OverrideTag::FadeComplex {
                alpha_start,
                alpha_mid,
                alpha_end,
                t1,
                t2,
                t3,
                ..
            } => {
                let a1 = *alpha_start;
                let a2 = *alpha_mid;
                if a1 <= VISIBLE_ALPHA {
                    fade_render_pt = Some(start_ms);
                } else if *t1 > 0 && a2 <= VISIBLE_ALPHA {
                    let t =
                        ((VISIBLE_ALPHA as f32 - a1 as f32) / (a2 as f32 - a1 as f32)) * *t1 as f32;
                    if t >= 0.0 {
                        fade_render_pt = Some(start_ms.saturating_add(t as u64).min(end_ms));
                    }
                } else if a2 <= VISIBLE_ALPHA {
                    fade_render_pt = Some(start_ms.saturating_add(*t1).min(end_ms));
                } else if *t3 > 0 {
                    let a3 = *alpha_end;
                    if a3 < a2 {
                        let t = ((VISIBLE_ALPHA as f32 - a2 as f32) / (a3 as f32 - a2 as f32))
                            * *t3 as f32;
                        if t >= 0.0 {
                            fade_render_pt =
                                Some(start_ms.saturating_add(*t1 + *t2 + t as u64).min(end_ms));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fade_render_pt.unwrap_or(start_ms)
}

/// Crop a rendered RGBA bitmap to the tight bounding box of non-transparent pixels.
///
/// Returns `(cropped_rgba, x, y, w, h)` or `None` if the frame is entirely transparent.
pub fn crop_to_tight_bbox(
    bitmap: &[u8],
    width: u32,
    height: u32,
) -> Option<(Vec<u8>, u32, u32, u32, u32)> {
    if bitmap.len() != (width as usize) * (height as usize) * 4 {
        return None;
    }
    let mut min_x = u32::MAX;
    let mut min_y = u32::MAX;
    let mut max_x = 0u32;
    let mut max_y = 0u32;
    let mut any = false;

    for y in 0..height {
        for x in 0..width {
            let off = ((y * width + x) * 4) as usize;
            if bitmap[off + 3] > 0 {
                any = true;
                if x < min_x {
                    min_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if x > max_x {
                    max_x = x;
                }
                if y > max_y {
                    max_y = y;
                }
            }
        }
    }

    if !any {
        return None;
    }

    let w = max_x - min_x + 1;
    let h = max_y - min_y + 1;
    let mut out = Vec::with_capacity((w as usize) * (h as usize) * 4);

    for y in min_y..=max_y {
        let row_start = ((y * width + min_x) * 4) as usize;
        let row_end = row_start + (w as usize) * 4;
        out.extend_from_slice(&bitmap[row_start..row_end]);
    }

    Some((out, min_x, min_y, w, h))
}
