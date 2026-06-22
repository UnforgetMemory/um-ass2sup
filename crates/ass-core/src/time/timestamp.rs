/// A timestamp in milliseconds, used for subtitle event timing.
///
/// Internally stores milliseconds as a `u64`, supporting timestamps
/// up to ~585 million years. Provides conversion to/from ASS time
/// format (`H:MM:SS.CS`) and SRT format (`HH:MM:SS,mmm`).
///
/// # Design
/// - Pure integer math: no floating point anywhere
/// - Millisecond resolution (ASS centiseconds × 10)
/// - Saturating arithmetic for overflow safety
///
/// # Examples
/// ```
/// use ass_core::time::Timestamp;
///
/// let ts = Timestamp::from_hms(0, 1, 30, 500);
/// assert_eq!(ts.as_ms(), 90500);
/// assert_eq!(ts.as_ass_time(), "0:01:30.50");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(u64);

impl Timestamp {
    /// Zero timestamp (0:00:00.00).
    pub const ZERO: Self = Self(0);

    /// Create from raw milliseconds.
    #[inline]
    pub const fn from_ms(ms: u64) -> Self {
        Self(ms)
    }

    /// Create from hours, minutes, seconds, and milliseconds.
    #[inline]
    pub fn from_hms(h: u32, m: u32, s: u32, ms: u32) -> Self {
        Self((u64::from(h) * 3600 + u64::from(m) * 60 + u64::from(s)) * 1000 + u64::from(ms))
    }

    /// Parse an ASS time string `H:MM:SS.CS` (centiseconds).
    ///
    /// # Errors
    /// Returns `ParseError` if the string is not in valid ASS format.
    pub fn from_ass_time(s: &str) -> Result<Self, super::super::ParseError> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return Err(super::super::ParseError::invalid_timestamp(s));
        }
        let h: u64 = parts[0]
            .parse()
            .map_err(|_| super::super::ParseError::invalid_timestamp(s))?;
        let m: u64 = parts[1]
            .parse()
            .map_err(|_| super::super::ParseError::invalid_timestamp(s))?;
        let sec_parts: Vec<&str> = parts[2].split('.').collect();
        if sec_parts.len() != 2 {
            return Err(super::super::ParseError::invalid_timestamp(s));
        }
        let sec: u64 = sec_parts[0]
            .parse()
            .map_err(|_| super::super::ParseError::invalid_timestamp(s))?;
        let cs: u64 = sec_parts[1]
            .parse()
            .map_err(|_| super::super::ParseError::invalid_timestamp(s))?;
        Ok(Self::from_ms(
            h * 3_600_000 + m * 60_000 + sec * 1000 + cs.saturating_mul(10),
        ))
    }

    /// Parse an SRT time string `HH:MM:SS,mmm` (milliseconds).
    ///
    /// Accepts both `,` and `.` as fractional separator.
    /// Supports 1-to-3 digit fractional parts (right-padded with zeros).
    ///
    /// # Errors
    /// Returns `ParseError` if the string is not in valid SRT format.
    pub fn from_srt_timecode(s: &str) -> Result<Self, super::super::ParseError> {
        let s = s.trim();
        let (time_part, frac_part) = s
            .split_once(',')
            .or_else(|| s.split_once('.'))
            .unwrap_or((s, "0"));
        let frac = frac_part.trim();
        // Right-pad fractional digits to milliseconds
        let ms = match frac.len() {
            1 => frac.parse::<u64>().unwrap_or(0) * 100, // "5" -> 500
            2 => frac.parse::<u64>().unwrap_or(0) * 10,  // "05" -> 50
            3 => frac.parse::<u64>().unwrap_or(0),       // "500" -> 500
            _ => {
                let val: u64 = frac[..3].parse().unwrap_or(0);
                val
            }
        };
        let parts: Vec<&str> = time_part.split(':').collect();
        if parts.len() != 3 {
            return Err(super::super::ParseError::invalid_timestamp(s));
        }
        let h: u64 = parts[0]
            .parse()
            .map_err(|_| super::super::ParseError::invalid_timestamp(s))?;
        let m: u64 = parts[1]
            .parse()
            .map_err(|_| super::super::ParseError::invalid_timestamp(s))?;
        let sec: u64 = parts[2]
            .parse()
            .map_err(|_| super::super::ParseError::invalid_timestamp(s))?;
        let total = h
            .saturating_mul(3_600_000)
            .saturating_add(m.saturating_mul(60_000))
            .saturating_add(sec.saturating_mul(1000))
            .saturating_add(ms);
        Ok(Self(total))
    }

    /// Return the timestamp in milliseconds.
    #[inline]
    pub const fn as_ms(&self) -> u64 {
        self.0
    }

    /// Format as ASS time string `H:MM:SS.CS`.
    ///
    /// Note: centisecond precision means sub-10ms values are truncated.
    pub fn as_ass_time(&self) -> String {
        let ms = self.0;
        let h = ms / 3_600_000;
        let m = (ms % 3_600_000) / 60_000;
        let s = (ms % 60_000) / 1_000;
        let cs = (ms % 1_000) / 10;
        format!("{h}:{m:02}:{s:02}.{cs:02}")
    }

    /// Format as SRT time string `HH:MM:SS,mmm`.
    pub fn as_srt_time(&self) -> String {
        let ms = self.0;
        let h = ms / 3_600_000;
        let m = (ms % 3_600_000) / 60_000;
        let s = (ms % 60_000) / 1_000;
        let frac = ms % 1_000;
        format!("{h:02}:{m:02}:{s:02},{frac:03}")
    }

    /// Duration in milliseconds from `self` to `end` (saturating).
    #[inline]
    pub fn duration_ms(&self, end: Timestamp) -> u64 {
        end.0.saturating_sub(self.0)
    }
}

impl std::fmt::Display for Timestamp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_ass_time())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_ms_roundtrip() {
        let ts = Timestamp::from_ms(90500);
        assert_eq!(ts.as_ms(), 90500);
        assert_eq!(ts.as_ass_time(), "0:01:30.50");
    }

    #[test]
    fn from_hms_basic() {
        let ts = Timestamp::from_hms(1, 30, 0, 500);
        assert_eq!(ts.as_ms(), 5_400_500);
    }

    #[test]
    fn zero_constant() {
        assert_eq!(Timestamp::ZERO.as_ms(), 0);
        assert_eq!(Timestamp::ZERO.as_ass_time(), "0:00:00.00");
    }

    #[test]
    fn from_ass_time_valid() {
        let ts = Timestamp::from_ass_time("0:01:30.50").unwrap();
        assert_eq!(ts.as_ms(), 90500);
    }

    #[test]
    fn from_ass_time_invalid_format() {
        assert!(Timestamp::from_ass_time("bad").is_err());
        assert!(Timestamp::from_ass_time("0:00:00").is_err());
        assert!(Timestamp::from_ass_time("").is_err());
    }

    #[test]
    fn from_ass_time_overflow_safe() {
        // Very large centisecond value should not panic
        let ts = Timestamp::from_ass_time("0:00:00.999999").unwrap();
        assert!(ts.as_ms() >= 9990); // cs.saturating_mul(10)
    }

    #[test]
    fn from_srt_timecode_comma() {
        let ts = Timestamp::from_srt_timecode("00:01:30,500").unwrap();
        assert_eq!(ts.as_ms(), 90500);
    }

    #[test]
    fn from_srt_timecode_dot() {
        let ts = Timestamp::from_srt_timecode("00:01:30.500").unwrap();
        assert_eq!(ts.as_ms(), 90500);
    }

    #[test]
    fn from_srt_timecode_short_millis() {
        // ",5" -> 500ms, not 5ms
        let ts = Timestamp::from_srt_timecode("00:00:01,5").unwrap();
        assert_eq!(ts.as_ms(), 1500);
    }

    #[test]
    fn from_srt_timecode_two_digits() {
        let ts = Timestamp::from_srt_timecode("00:00:01,05").unwrap();
        assert_eq!(ts.as_ms(), 1050);
    }

    #[test]
    fn duration_ms_normal() {
        let start = Timestamp::from_ms(1000);
        let end = Timestamp::from_ms(5000);
        assert_eq!(start.duration_ms(end), 4000);
    }

    #[test]
    fn duration_ms_saturating() {
        let start = Timestamp::from_ms(5000);
        let end = Timestamp::from_ms(1000);
        assert_eq!(start.duration_ms(end), 0); // end < start
    }

    #[test]
    fn as_srt_time_format() {
        let ts = Timestamp::from_hms(1, 2, 3, 456);
        assert_eq!(ts.as_srt_time(), "01:02:03,456");
    }

    #[test]
    fn ordering() {
        let a = Timestamp::from_ms(100);
        let b = Timestamp::from_ms(200);
        assert!(a < b);
        assert!(b > a);
    }
}
