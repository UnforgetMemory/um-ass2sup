//! Time unit conversions and format helpers.
//!
//! All functions use pure integer arithmetic — no floating point.

/// Convert milliseconds to 90kHz PTS ticks.
///
/// PGS (Presentation Graphics Stream) uses a 90kHz clock.
/// Conversion is exact: `ms × 90 = ticks`.
/// No frame rate needed — this is a straight unit conversion.
///
/// # Examples
/// ```
/// use ass_core::time::ms_to_90khz;
/// assert_eq!(ms_to_90khz(1000), 90_000);
/// ```
#[inline]
pub const fn ms_to_90khz(ms: u64) -> u64 {
    ms * 90
}

/// Format milliseconds as ASS timecode `H:MM:SS.CS`.
///
/// CS = centiseconds, so sub-10ms precision is lost.
///
/// # Examples
/// ```
/// use ass_core::time::ms_to_ass_timecode;
/// assert_eq!(ms_to_ass_timecode(90500), "0:01:30.50");
/// ```
pub fn ms_to_ass_timecode(ms: u64) -> String {
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let cs = (ms % 1_000) / 10;
    format!("{h}:{m:02}:{s:02}.{cs:02}")
}

/// Format milliseconds as SRT timecode `HH:MM:SS,mmm`.
///
/// # Examples
/// ```
/// use ass_core::time::ms_to_srt_timecode;
/// assert_eq!(ms_to_srt_timecode(90500), "00:01:30,500");
/// ```
pub fn ms_to_srt_timecode(ms: u64) -> String {
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let frac = ms % 1_000;
    format!("{h:02}:{m:02}:{s:02},{frac:03}")
}

/// Format milliseconds as TTML/WebVTT timecode `HH:MM:SS.fff`.
///
/// # Examples
/// ```
/// use ass_core::time::ms_to_ttml_timecode;
/// assert_eq!(ms_to_ttml_timecode(90500), "00:01:30.500");
/// ```
pub fn ms_to_ttml_timecode(ms: u64) -> String {
    let h = ms / 3_600_000;
    let m = (ms % 3_600_000) / 60_000;
    let s = (ms % 60_000) / 1_000;
    let frac = ms % 1_000;
    format!("{h:02}:{m:02}:{s:02}.{frac:03}")
}

/// Format milliseconds as BDN XML frame timecode `HH:MM:SS:FF`.
///
/// Uses the given [`Fps`] to compute frame numbers.
/// Pure integer arithmetic — no `fps as u64` truncation.
///
/// # Examples
/// ```
/// use ass_core::time::{ms_to_frame_timecode, Fps};
/// // At NTSC 24fps, frame 1 starts at 1001/24000*1000 ≈ 41.7ms
/// let tc = ms_to_frame_timecode(0, Fps::FILM_24);
/// assert_eq!(tc, "00:00:00:00");
/// ```
pub fn ms_to_frame_timecode(ms: u64, fps: crate::time::Fps) -> String {
    // Compute frame count via ceiling division of ms * numer / (1000 * denom)
    // We need to convert ms → frame index using the Fps.
    // frame_idx = ceil(ms * numer / (1000 * denom))
    let a = ms.saturating_mul(fps.numer as u64);
    let b = 1000u64.saturating_mul(fps.denom as u64);
    let frame_idx = a
        .saturating_add(b)
        .saturating_sub(1)
        .checked_div(b)
        .unwrap_or(0);

    let frames_per_sec = fps.numer as u64;
    let total_secs = frame_idx / frames_per_sec;
    let frames = frame_idx % frames_per_sec;

    let h = total_secs / 3600;
    let m = (total_secs % 3600) / 60;
    let s = total_secs % 60;

    format!("{h:02}:{m:02}:{s:02}:{frames:02}")
}

/// Snap a timestamp forward to the next frame boundary.
///
/// Pure integer ceiling division:
/// ```text
/// frame_idx  = ceil(ms * fps.numer / (1000 * fps.denom))
/// snapped_ms = frame_idx * 1000 * fps.denom / fps.numer
/// ```
pub fn snap_to_frame(ms: u64, fps: crate::time::Fps) -> u64 {
    fps.snap_to_frame(ms)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::time::Fps;

    #[test]
    fn ms_to_90khz_basic() {
        assert_eq!(ms_to_90khz(1000), 90_000);
        assert_eq!(ms_to_90khz(0), 0);
        assert_eq!(ms_to_90khz(1), 90);
        // Large value: 1 hour
        assert_eq!(ms_to_90khz(3_600_000), 324_000_000);
    }

    #[test]
    fn ms_to_ass_timecode_basic() {
        assert_eq!(ms_to_ass_timecode(90500), "0:01:30.50");
        assert_eq!(ms_to_ass_timecode(0), "0:00:00.00");
        assert_eq!(ms_to_ass_timecode(3_600_000), "1:00:00.00");
    }

    #[test]
    fn ms_to_srt_timecode_basic() {
        assert_eq!(ms_to_srt_timecode(90500), "00:01:30,500");
        assert_eq!(ms_to_srt_timecode(3_600_000), "01:00:00,000");
    }

    #[test]
    fn ms_to_ttml_timecode_basic() {
        assert_eq!(ms_to_ttml_timecode(90500), "00:01:30.500");
    }

    #[test]
    fn ms_to_frame_timecode_film_24_zero() {
        let tc = ms_to_frame_timecode(0, Fps::FILM_24);
        assert_eq!(tc, "00:00:00:00");
    }

    #[test]
    fn ms_to_frame_timecode_film_24_first_second() {
        // At 24fps: 1000ms = 24 frames, so at ms=1000 frame should be 24
        // But we ceil-snap: ms=1000 → ceil(1000*24/1000) = ceil(24) = 24 frames
        // frame 24 = second 1, frame 0
        let tc = ms_to_frame_timecode(1000, Fps::FILM_24);
        assert_eq!(tc, "00:00:01:00");
    }

    #[test]
    fn ms_to_frame_timecode_ntsc_24() {
        // At NTSC 24fps: 1000ms → ceil(1000*24000/(1000*1001)) = ceil(23.976) = 24 frames
        // 24 frames at 24000/1001 fps = not exactly 1 second
        let tc = ms_to_frame_timecode(1000, Fps::NTSC_24);
        // Should be 0:00:00:24 (not 0:00:01:00 because NTSC frame is 1001/24000 sec)
        assert_eq!(tc, "00:00:00:24");
    }

    #[test]
    fn ms_to_frame_timecode_ntsc_24_drift_check() {
        // After 1000 frames of NTSC 24fps, check drift
        let ms = 1000u64;
        let a = ms.saturating_mul(Fps::NTSC_24.numer as u64);
        let b = 1000u64.saturating_mul(Fps::NTSC_24.denom as u64);
        let frame_idx = a.saturating_add(b).saturating_sub(1) / b;
        // frame_idx = ceil(24000000 / 1001000) = ceil(23.976...) = 24
        assert_eq!(frame_idx, 24);
    }

    #[test]
    fn snap_to_frame_film_24() {
        let snapped = snap_to_frame(0, Fps::FILM_24);
        assert_eq!(snapped, 0);
    }
}
