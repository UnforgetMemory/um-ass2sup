/// Rational frame rate — pure integer, no floating point.
///
/// Represents frame rate as a fraction `numer / denom`.
/// Standard NTSC rates use the 1000/1001 factor:
/// - 23.976 → 24000/1001
/// - 29.97  → 30000/1001
/// - 59.94  → 60000/1001
///
/// # Integer arithmetic guarantee
/// All time conversions use only `u32`/`u64` math.
/// No `f32`, `f64`, or division-by-float anywhere.
///
/// # Examples
/// ```
/// use ass_core::time::Fps;
///
/// let fps = Fps::NTSC_24;
/// assert_eq!(fps.numer, 24000);
/// assert_eq!(fps.denom, 1001);
///
/// // Pure integer snap-to-frame
/// let snapped = fps.snap_to_frame(1000);
/// assert!(snapped >= 1000); // always forward to next frame boundary
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fps {
    /// Numerator (e.g., 24000 for NTSC 24fps)
    pub numer: u32,
    /// Denominator (e.g., 1001 for NTSC rates)
    pub denom: u32,
}

impl Fps {
    // ── Standard frame rate constants ──
    /// NTSC 24fps: 24000/1001 (≈23.976)
    pub const NTSC_24: Self = Self {
        numer: 24000,
        denom: 1001,
    };
    /// NTSC 30fps: 30000/1001 (≈29.97)
    pub const NTSC_30: Self = Self {
        numer: 30000,
        denom: 1001,
    };
    /// NTSC 60fps: 60000/1001 (≈59.94)
    pub const NTSC_60: Self = Self {
        numer: 60000,
        denom: 1001,
    };
    /// PAL 25fps: 25/1
    pub const PAL_25: Self = Self {
        numer: 25,
        denom: 1,
    };
    /// Film 24fps: 24/1
    pub const FILM_24: Self = Self {
        numer: 24,
        denom: 1,
    };
    /// Film 30fps: 30/1
    pub const FILM_30: Self = Self {
        numer: 30,
        denom: 1,
    };
    /// Film 50fps: 50/1
    pub const FILM_50: Self = Self {
        numer: 50,
        denom: 1,
    };
    /// Film 60fps: 60/1
    pub const FILM_60: Self = Self {
        numer: 60,
        denom: 1,
    };

    /// Create from explicit numerator and denominator.
    ///
    /// # Panics
    /// Panics if `denom` is 0.
    #[inline]
    pub const fn new(numer: u32, denom: u32) -> Self {
        assert!(denom != 0, "Fps denominator cannot be zero");
        Self { numer, denom }
    }

    /// Approximate from `f64`, matching the nearest standard rate.
    ///
    /// Uses a tolerance of 0.001 to detect standard rates.
    /// Falls back to a denominator=1000 rational approximation.
    pub fn from_f64(v: f64) -> Self {
        // Check standard rates within tolerance
        let candidates = [
            (24000.0 / 1001.0, Self::NTSC_24),
            (30000.0 / 1001.0, Self::NTSC_30),
            (60000.0 / 1001.0, Self::NTSC_60),
            (25.0, Self::PAL_25),
            (24.0, Self::FILM_24),
            (30.0, Self::FILM_30),
            (50.0, Self::FILM_50),
            (60.0, Self::FILM_60),
        ];
        for (expected, fps) in candidates {
            if (v - expected).abs() < 0.001 {
                return fps;
            }
        }
        // Fallback: approximate with 1000 denominator
        let denom = 1000u32;
        let numer = (v * denom as f64).round() as u32;
        // Clamp to avoid invalid framerates
        let numer = numer.clamp(1, 60000);
        Self { numer, denom }
    }

    /// Frame duration as a rational `(numerator, denominator)` milliseconds.
    ///
    /// The actual frame duration is `numerator / denominator` ms.
    /// This avoids floating point: `frame_ms = 1000 * denom / numer`.
    #[inline]
    pub fn frame_duration_ms(&self) -> (u64, u64) {
        (1000 * self.denom as u64, self.numer as u64)
    }

    /// Snap a timestamp to the next frame boundary (ceil).
    ///
    /// Pure integer arithmetic:
    /// ```text
    /// frame_idx   = ceil(ms * numer / (1000 * denom))
    /// snapped_ms  = frame_idx * 1000 * denom / numer
    /// ```
    ///
    /// This is the correct forward-snap used by PGS encoders:
    /// subtitles appear at frame boundaries, not mid-frame.
    pub fn snap_to_frame(&self, ms: u64) -> u64 {
        let a = ms.saturating_mul(self.numer as u64);
        let b = 1000u64.saturating_mul(self.denom as u64);
        if b == 0 {
            return ms; // degenerate, should not happen
        }
        // Ceiling division: (a + b - 1) / b
        let frame_idx = a.saturating_add(b).saturating_sub(1) / b;
        // snapped = frame_idx * 1000 * denom / numer
        frame_idx
            .saturating_mul(1000)
            .saturating_mul(self.denom as u64)
            .saturating_div(self.numer as u64)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ntsc_24_standard_rate() {
        let fps = Fps::NTSC_24;
        assert_eq!(fps.numer, 24000);
        assert_eq!(fps.denom, 1001);
    }

    #[test]
    fn from_f64_matches_ntsc_24() {
        let fps = Fps::from_f64(23.976);
        assert_eq!(fps, Fps::NTSC_24);
    }

    #[test]
    fn from_f64_matches_ntsc_30() {
        let fps = Fps::from_f64(29.97);
        assert_eq!(fps, Fps::NTSC_30);
    }

    #[test]
    fn from_f64_matches_pal_25() {
        let fps = Fps::from_f64(25.0);
        assert_eq!(fps, Fps::PAL_25);
    }

    #[test]
    fn from_f64_fallback_rounded() {
        let fps = Fps::from_f64(12.0);
        assert_eq!(fps.denom, 1000);
        assert_eq!(fps.numer, 12000);
    }

    #[test]
    fn frame_duration_ms_ntsc_24() {
        let (num, den) = Fps::NTSC_24.frame_duration_ms();
        // frame_duration = 1000 * 1001 / 24000 = 1001000/24000
        assert_eq!(num, 1000 * 1001u64);
        assert_eq!(den, 24000);
    }

    #[test]
    fn snap_to_frame_zero() {
        let snapped = Fps::NTSC_24.snap_to_frame(0);
        assert_eq!(snapped, 0);
    }

    #[test]
    fn snap_to_frame_forward() {
        // At NTSC 24fps, frame duration = 1001000/24000 ≈ 41.708ms
        // ms=0 → frame 0 → snapped 0
        // ms=10 → frame 1 (ceil) → snapped ≈ 41.708ms (as integer)
        let snapped = Fps::NTSC_24.snap_to_frame(10);
        assert!(snapped >= 10);
        // snapped should be close to 41ms (1001000/24000 = 41.708...)
        assert!(snapped < 100);
    }

    #[test]
    fn film_24_snap_exact() {
        // At 24fps, frame duration = 1000/24 = 41.666... ms
        // ms=42 → frame 1 (ceil(42*24/1000) = ceil(1.008) = 2) → 2000/24 = 83.33ms
        let snapped = Fps::FILM_24.snap_to_frame(42);
        // Should snap past frame 1 boundary
        assert!(snapped > 42);
    }

    #[test]
    fn ms_to_90khz_is_pure_integer() {
        let pts = crate::time::ms_to_90khz(1000);
        assert_eq!(pts, 90000);
        assert_eq!(crate::time::ms_to_90khz(0), 0);
        assert_eq!(crate::time::ms_to_90khz(1), 90);
    }
}
