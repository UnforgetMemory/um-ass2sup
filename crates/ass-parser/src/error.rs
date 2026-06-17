use std::fmt;
use thiserror::Error;

/// Errors that can occur when parsing ASS/SSA/SRT subtitle files.
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("invalid timestamp format: {0}")]
    InvalidTimestamp(String),

    #[error("invalid color format: {0}")]
    InvalidColor(String),

    #[error("invalid style: {0}")]
    InvalidStyle(String),

    #[error("invalid event: {0}")]
    InvalidEvent(String),

    #[error("invalid section: {0}")]
    InvalidSection(String),

    #[error("missing required section: {0}")]
    MissingSection(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("encoding error: {0}")]
    Encoding(String),
}

/// Non-fatal warnings produced during lenient/recovery parsing.
///
/// These indicate recoverable issues where the parser applied defaults
/// or skipped minor content, but the overall parse was successful.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseWarning {
    /// An unknown section header was encountered and its content was ignored.
    UnknownSection(String),

    /// A malformed field value was replaced with a default.
    InvalidField {
        /// Name of the field that was invalid.
        field: String,
        /// The invalid raw value that was encountered.
        value: String,
        /// The default value that was used instead.
        default: String,
    },

    /// An invalid color value was replaced with a default.
    InvalidColor {
        /// Name of the field with the bad color.
        field: String,
        /// The invalid raw color string.
        value: String,
    },

    /// An invalid timestamp was replaced with zero.
    InvalidTimestamp(String),

    /// An event had fewer fields than expected.
    IncompleteEvent {
        /// Number of fields expected.
        expected: usize,
        /// Number of fields actually received.
        got: usize,
    },

    /// A SRT block was skipped due to format issues.
    SrtBlockSkipped {
        /// Block index in the SRT file.
        index: usize,
        /// Reason the block was skipped.
        reason: String,
    },
}

impl fmt::Display for ParseWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownSection(name) => {
                write!(f, "unknown section '[{name}]' ignored")
            }
            Self::InvalidField {
                field,
                value,
                default,
            } => {
                write!(
                    f,
                    "invalid field '{field}' with value '{value}', using default {default}"
                )
            }
            Self::InvalidColor { field, value } => {
                write!(f, "invalid color '{value}' for {field}, using default")
            }
            Self::InvalidTimestamp(value) => {
                write!(f, "malformed timestamp '{value}', using zero")
            }
            Self::IncompleteEvent { expected, got } => {
                write!(
                    f,
                    "incomplete event (expected {expected} fields, got {got})"
                )
            }
            Self::SrtBlockSkipped { index, reason } => {
                write!(f, "SRT block {index} skipped: {reason}")
            }
        }
    }
}
