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
