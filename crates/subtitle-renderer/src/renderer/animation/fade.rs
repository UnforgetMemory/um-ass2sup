/// Compute alpha multiplier for `\fad(t1, t2)` at a given elapsed time.
pub fn compute_fad_alpha(
    elapsed_ms: u64,
    event_duration_ms: u64,
    fade_in_ms: u64,
    fade_out_ms: u64,
) -> f32 {
    if event_duration_ms == 0 {
        return 1.0;
    }
    let fi = fade_in_ms.min(event_duration_ms);
    let fo = fade_out_ms.min(event_duration_ms);
    if elapsed_ms < fi {
        elapsed_ms as f32 / fi as f32
    } else if elapsed_ms > event_duration_ms.saturating_sub(fo) {
        let fade_end = event_duration_ms.saturating_sub(fo);
        let t = (elapsed_ms.saturating_sub(fade_end)) as f32 / fo as f32;
        1.0 - t.min(1.0)
    } else {
        1.0
    }
}

/// Compute alpha multiplier for `\fade(a1, a2, a3, t1, t2, t3, t4)`.
#[allow(clippy::too_many_arguments)]
pub fn compute_fade_complex(
    elapsed_ms: u64,
    a1: u8,
    a2: u8,
    a3: u8,
    t1: u64,
    t2: u64,
    t3: u64,
    t4: u64,
) -> f32 {
    let alpha_at = |ms: u64| -> f32 {
        if ms < t1 {
            a1 as f32 / 255.0
        } else if ms < t2 {
            lerp(a1 as f32, a2 as f32, (ms - t1) as f32 / (t2 - t1) as f32)
        } else if ms < t3 {
            a2 as f32 / 255.0
        } else if ms < t4 {
            lerp(a2 as f32, a3 as f32, (ms - t3) as f32 / (t4 - t3) as f32)
        } else {
            a3 as f32 / 255.0
        }
    };
    alpha_at(elapsed_ms)
}

/// Linear interpolation helper.
fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_fad_alpha_full_visibility() {
        let a = compute_fad_alpha(5000, 10000, 1000, 1000);
        assert!((a - 1.0).abs() < 0.01, "mid-event alpha should be 1.0");
    }

    #[test]
    fn test_compute_fad_alpha_fade_in() {
        let a = compute_fad_alpha(500, 10000, 1000, 1000);
        assert!(
            (a - 0.5).abs() < 0.01,
            "halfway through fade-in should be 0.5"
        );
    }

    #[test]
    fn test_compute_fad_alpha_fade_out() {
        let a = compute_fad_alpha(9500, 10000, 1000, 1000);
        assert!(
            (a - 0.5).abs() < 0.01,
            "halfway through fade-out should be 0.5"
        );
    }

    #[test]
    fn test_compute_fad_alpha_beginning() {
        let a = compute_fad_alpha(0, 10000, 1000, 1000);
        assert!((a - 0.0).abs() < 0.01, "at start alpha should be 0.0");
    }

    #[test]
    fn test_compute_fade_complex_constant() {
        let a = compute_fade_complex(5000, 128, 128, 128, 1000, 2000, 8000, 9000);
        assert!((a - 128.0 / 255.0).abs() < 0.01, "mid segment at a2");
    }
}
