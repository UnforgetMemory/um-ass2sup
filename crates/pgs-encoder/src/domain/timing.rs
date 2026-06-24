/// Determine the PGS frame rate code byte from an FPS value.
pub fn frame_rate_code(fps: f64) -> u8 {
    if fps <= 24.0 {
        0x10
    } else if fps <= 25.0 {
        0x20
    } else if fps <= 30.0 {
        0x40
    } else if fps <= 50.0 {
        0x50
    } else if fps <= 60.0 {
        0x70
    } else {
        0x10
    }
}

/// Check if a frame rate is NTSC-based (23.976, 29.97, 59.94).
pub fn is_ntsc_fps(fps: f64) -> bool {
    (fps - 23.976).abs() < 0.01 || (fps - 29.97).abs() < 0.01 || (fps - 59.94).abs() < 0.01
}

/// Convert milliseconds to 90 kHz PTS ticks.
///
/// Uses NTSC-correct formula for 23.976/29.97/59.94 fps,
/// simple `ms * 90` otherwise.
pub fn ms_to_90khz(ms: u64, fps: f64) -> u64 {
    if is_ntsc_fps(fps) {
        (ms as u128 * 90000 * 1001 / 1000000) as u64
    } else {
        ms * 90
    }
}

/// Parse a timecode string (HH:MM:SS.mmm or HH:MM:SS:FF) to milliseconds.
pub fn timecode_to_ms(timecode: &str) -> Option<u64> {
    let parts: Vec<&str> = if timecode.contains(':') {
        timecode.split(':').collect()
    } else {
        return None;
    };

    match parts.len() {
        4 => {
            let h = parts[0].parse::<u64>().ok()?;
            let m = parts[1].parse::<u64>().ok()?;
            let s = parts[2].parse::<u64>().ok()?;
            let ms = parts[3].parse::<u64>().ok()?;
            Some(h * 3_600_000 + m * 60_000 + s * 1_000 + ms)
        }
        3 => {
            let h = parts[0].parse::<u64>().ok()?;
            let m = parts[1].parse::<u64>().ok()?;
            let s_ms: Vec<&str> = parts[2].split('.').collect();
            let s = s_ms[0].parse::<u64>().ok()?;
            let ms = if s_ms.len() > 1 {
                s_ms[1].parse::<u64>().ok()?
            } else {
                0
            };
            Some(h * 3_600_000 + m * 60_000 + s * 1_000 + ms)
        }
        _ => None,
    }
}
