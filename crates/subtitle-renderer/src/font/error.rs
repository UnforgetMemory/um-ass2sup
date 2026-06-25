//! Domain error types for the font subsystem.
//!
//! Each variant carries enough context for callers to decide how to
//! recover or report the problem.

use std::fmt;
use std::path::PathBuf;

use super::types::{FontFace, FontQuery};

/// Font subsystem errors.
#[derive(Debug)]
pub enum FontError {
    /// The requested font could not be found.
    NotFound(FontNotFound),
    /// A font file was found but could not be loaded.
    Corrupted {
        path: PathBuf,
        reason: String,
    },
    /// No system fonts are available at all.
    NoSystemFonts,
    /// An I/O error occurred while accessing a font file.
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    /// A font file had unparseable content (e.g. bad metadata).
    Parse {
        path: PathBuf,
        detail: String,
    },
}

/// Detailed font-not-found error with candidates for caller decision.
#[derive(Debug, Clone)]
pub struct FontNotFound {
    /// The query that failed.
    pub query: FontQuery,
    /// Any candidates that were close to the query.
    pub candidates: Vec<FontFace>,
    /// The single best suggestion, if any.
    pub suggestion: Option<FontFace>,
}

impl fmt::Display for FontNotFound {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Font '{}' weight={} style={:?} not found.",
            self.query.family,
            self.query.weight.as_u16(),
            self.query.style
        )?;
        if !self.candidates.is_empty() {
            write!(f, " Available: ")?;
            for (i, c) in self.candidates.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "weight={}", c.weight.as_u16())?;
            }
            write!(f, ".")?;
        }
        if let Some(s) = &self.suggestion {
            write!(
                f,
                " Closest: '{}' weight={}.",
                s.family,
                s.weight.as_u16()
            )?;
        }
        Ok(())
    }
}

impl fmt::Display for FontError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            FontError::NotFound(nf) => write!(f, "{nf}"),
            FontError::Corrupted { path, reason } => {
                write!(f, "Font at '{}' is corrupted: {reason}", path.display())
            }
            FontError::NoSystemFonts => {
                write!(f, "No system fonts available")
            }
            FontError::Io { path, source } => {
                write!(f, "I/O error accessing '{}': {source}", path.display())
            }
            FontError::Parse { path, detail } => {
                write!(
                    f,
                    "Failed to parse font '{}': {detail}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for FontError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            FontError::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl From<FontNotFound> for FontError {
    fn from(v: FontNotFound) -> Self {
        FontError::NotFound(v)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::{FontId, FontStyle, FontStretch, FontWeight};

    fn make_face(family: &str, weight: FontWeight) -> FontFace {
        FontFace {
            id: FontId(1),
            family: family.to_string(),
            weight,
            style: FontStyle::Normal,
            stretch: FontStretch::Normal,
            path: None,
            is_system: true,
            cjk: false,
            corrupt: false,
        }
    }

    #[test]
    fn font_not_found_display_no_candidates() {
        let err = FontNotFound {
            query: FontQuery {
                family: "NonExistent".into(),
                weight: FontWeight::Bold,
                style: FontStyle::Italic,
            },
            candidates: vec![],
            suggestion: None,
        };
        let msg = err.to_string();
        assert!(msg.contains("NonExistent"));
        assert!(msg.contains("weight=700"));
        assert!(msg.contains("style=Italic"));
        assert!(msg.contains("not found"));
    }

    #[test]
    fn font_not_found_display_with_candidates() {
        let err = FontNotFound {
            query: FontQuery {
                family: "MyFont".into(),
                weight: FontWeight::Bold,
                style: FontStyle::Normal,
            },
            candidates: vec![
                make_face("MyFont", FontWeight::Normal),
                make_face("MyFont", FontWeight::Black),
            ],
            suggestion: Some(make_face("MyFont", FontWeight::Black)),
        };
        let msg = err.to_string();
        assert!(msg.contains("Available:"));
        assert!(msg.contains("weight=400"));
        assert!(msg.contains("weight=900"));
        assert!(msg.contains("Closest:"));
        assert!(msg.contains("MyFont"));
    }

    #[test]
    fn font_error_not_found_display() {
        let nf = FontNotFound {
            query: FontQuery {
                family: "Ghost".into(),
                weight: FontWeight::Normal,
                style: FontStyle::Normal,
            },
            candidates: vec![],
            suggestion: None,
        };
        let err = FontError::NotFound(nf);
        assert!(err.to_string().contains("Ghost"));
    }

    #[test]
    fn font_error_corrupted_display() {
        let err = FontError::Corrupted {
            path: PathBuf::from("/fonts/bad.ttf"),
            reason: "invalid cmap table".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("bad.ttf"));
        assert!(msg.contains("corrupted"));
        assert!(msg.contains("invalid cmap table"));
    }

    #[test]
    fn font_error_no_system_fonts_display() {
        let err = FontError::NoSystemFonts;
        assert_eq!(err.to_string(), "No system fonts available");
    }

    #[test]
    fn font_error_io_display() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err = FontError::Io {
            path: PathBuf::from("/fonts/missing.ttf"),
            source: io_err,
        };
        let msg = err.to_string();
        assert!(msg.contains("missing.ttf"));
        assert!(msg.contains("file not found"));
    }

    #[test]
    fn font_error_io_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "denied");
        let err = FontError::Io {
            path: PathBuf::from("/fonts/protected.ttf"),
            source: io_err,
        };
        let inner = std::error::Error::source(&err);
        assert!(inner.is_some());
        assert!(inner.unwrap().to_string().contains("denied"));
    }

    #[test]
    fn font_error_parse_display() {
        let err = FontError::Parse {
            path: PathBuf::from("/fonts/garbage.ttf"),
            detail: "unknown table tag 'Xxxx'".into(),
        };
        let msg = err.to_string();
        assert!(msg.contains("garbage.ttf"));
        assert!(msg.contains("Xxxx"));
    }

    #[test]
    fn from_font_not_found_into_font_error() {
        let nf = FontNotFound {
            query: FontQuery {
                family: "Abc".into(),
                weight: FontWeight::Normal,
                style: FontStyle::Normal,
            },
            candidates: vec![],
            suggestion: None,
        };
        let err: FontError = nf.into();
        assert!(matches!(err, FontError::NotFound(_)));
    }
}
