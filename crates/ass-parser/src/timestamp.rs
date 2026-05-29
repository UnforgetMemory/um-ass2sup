use std::fmt;
use crate::error::ParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub const ZERO: Self = Self(0);

    pub fn from_ms(ms: u64) -> Self {
        Self(ms)
    }

    pub fn from_hms(h: u32, m: u32, s: u32, ms: u32) -> Self {
        Self((h as u64 * 3600 + m as u64 * 60 + s as u64) * 1000 + ms as u64)
    }

    pub fn from_ass_time(s: &str) -> Result<Self, ParseError> {
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 3 {
            return Err(ParseError::InvalidTimestamp(s.to_string()));
        }
        let h: u32 = parts[0].parse().map_err(|_| ParseError::InvalidTimestamp(s.to_string()))?;
        let m: u32 = parts[1].parse().map_err(|_| ParseError::InvalidTimestamp(s.to_string()))?;
        let sec_parts: Vec<&str> = parts[2].split('.').collect();
        if sec_parts.len() != 2 {
            return Err(ParseError::InvalidTimestamp(s.to_string()));
        }
        let sec: u32 = sec_parts[0].parse().map_err(|_| ParseError::InvalidTimestamp(s.to_string()))?;
        let cs: u32 = sec_parts[1].parse().map_err(|_| ParseError::InvalidTimestamp(s.to_string()))?;
        Ok(Self::from_hms(h, m, sec, cs * 10))
    }

    pub fn as_ms(&self) -> u64 {
        self.0
    }

    pub fn as_ass_time(&self) -> String {
        let ms = self.0;
        let h = ms / 3600000;
        let m = (ms % 3600000) / 60000;
        let s = (ms % 60000) / 1000;
        let cs = (ms % 1000) / 10;
        format!("{h}:{m:02}:{s:02}.{cs:02}")
    }

    pub fn duration_ms(&self, end: Timestamp) -> u64 {
        end.0.saturating_sub(self.0)
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ass_time())
    }
}
