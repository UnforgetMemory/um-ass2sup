//! Error and warning types for subtitle parsing.
//!
//! Every [`ParseError`] can optionally carry a [`Span`](crate::Span)
//! pointing to the source location. [`ParseWarning`] collects
//! non-fatal issues where defaults were substituted.

use crate::Span;
use std::fmt;

/// Errors from parsing ASS/SSA/SRT subtitle files.
#[derive(Debug)]
pub enum ParseError {
    /// Invalid timestamp format.
    InvalidTimestamp { value: String, span: Option<Span> },
    /// Invalid color value.
    InvalidColor {
        field: &'static str,
        value: String,
        span: Option<Span>,
    },
    /// Invalid style definition (wrong field count etc.).
    InvalidStyle { detail: String, span: Option<Span> },
    /// Invalid event definition.
    InvalidEvent { detail: String, span: Option<Span> },
    /// IO error reading file.
    Io(std::io::Error),
}

impl ParseError {
    /// Create an `InvalidTimestamp` without span.
    pub fn invalid_timestamp(value: impl Into<String>) -> Self {
        Self::InvalidTimestamp {
            value: value.into(),
            span: None,
        }
    }

    /// Create an `InvalidColor` without span.
    pub fn invalid_color(field: &'static str, value: impl Into<String>) -> Self {
        Self::InvalidColor {
            field,
            value: value.into(),
            span: None,
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTimestamp { value, span } => {
                if let Some(s) = span {
                    write!(f, "{}: invalid timestamp '{value}'", s.display())
                } else {
                    write!(f, "invalid timestamp '{value}'")
                }
            }
            Self::InvalidColor { field, value, span } => {
                if let Some(s) = span {
                    write!(f, "{}: invalid color '{value}' for {field}", s.display())
                } else {
                    write!(f, "invalid color '{value}' for {field}")
                }
            }
            Self::InvalidStyle { detail, span } => {
                if let Some(s) = span {
                    write!(f, "{}: invalid style: {detail}", s.display())
                } else {
                    write!(f, "invalid style: {detail}")
                }
            }
            Self::InvalidEvent { detail, span } => {
                if let Some(s) = span {
                    write!(f, "{}: invalid event: {detail}", s.display())
                } else {
                    write!(f, "invalid event: {detail}")
                }
            }
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for ParseError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ParseError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

/// Severity level for parsing warnings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WarningSeverity {
    /// Informational — does not affect rendering.
    Info,
    /// Used default value — file may have issues.
    Warning,
    /// Used default value — likely visual data loss.
    DataLoss,
}

/// Non-fatal warning produced during lenient/recovery parsing.
#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    /// Category of the warning.
    pub kind: WarningKind,
    /// Severity.
    pub severity: WarningSeverity,
    /// Source location if available.
    pub span: Option<Span>,
}

/// Specific warning category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WarningKind {
    /// Unknown section header, content skipped.
    UnknownSection(String),
    /// Field value was invalid, used default.
    InvalidField {
        field: String,
        value: String,
        default: String,
    },
    /// Invalid color, used default.
    InvalidColor { field: String, value: String },
    /// Invalid timestamp, zeroed.
    InvalidTimestamp(String),
    /// Event had fewer fields than expected.
    IncompleteEvent { expected: usize, got: usize },
    /// SRT block skipped.
    SrtBlockSkipped { index: usize, reason: String },
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self.span {
            Some(ref s) => format!("{}: ", s.display()),
            None => String::new(),
        };
        match &self.kind {
            WarningKind::UnknownSection(name) => {
                write!(f, "{prefix}unknown section '[{name}]' ignored")
            }
            WarningKind::InvalidField {
                field,
                value,
                default,
            } => {
                write!(
                    f,
                    "{prefix}invalid field '{field}' with value '{value}', using default {default}"
                )
            }
            WarningKind::InvalidColor { field, value } => {
                write!(
                    f,
                    "{prefix}invalid color '{value}' for {field}, using default"
                )
            }
            WarningKind::InvalidTimestamp(value) => {
                write!(f, "{prefix}malformed timestamp '{value}', using zero")
            }
            WarningKind::IncompleteEvent { expected, got } => {
                write!(
                    f,
                    "{prefix}incomplete event (expected {expected} fields, got {got})"
                )
            }
            WarningKind::SrtBlockSkipped { index, reason } => {
                write!(f, "{prefix}SRT block {index} skipped: {reason}")
            }
        }
    }
}
