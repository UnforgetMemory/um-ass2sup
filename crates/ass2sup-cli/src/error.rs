//! Error types for the ass2sup CLI application.
//!
//! Defines [`CliError`] with typed variants for each failure mode
//! encountered during file processing, conversion, and I/O.

/// Convenience alias for CLI operations.
pub type Result<T> = std::result::Result<T, CliError>;

/// Errors that can occur during CLI execution.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// Invalid resolution format or dimensions.
    #[error("Invalid resolution '{input}': {message}")]
    InvalidResolution {
        /// The malformed input string the user provided.
        input: String,
        /// Human-readable explanation of why the value is invalid.
        message: String,
    },

    /// Input file exceeds the size limit.
    #[error("Input '{path}' is {size} bytes which exceeds the {max} byte limit")]
    InputTooLarge {
        /// Path of the oversized input.
        path: String,
        /// Actual file size in bytes.
        size: u64,
        /// Maximum allowed size in bytes.
        max: u64,
    },

    /// Conversion failed for a file.
    #[error("Conversion failed: {0}")]
    Conversion(String),

    /// Failed to read an input file.
    #[error("Cannot read '{0}': {1}")]
    ReadError(String, String),

    /// Failed to parse a subtitle file.
    #[error("Parse error in '{0}': {1}")]
    ParseError(String, String),

    /// Failed to create the output directory.
    #[error("Failed to create output directory '{0}': {1}")]
    CreateDirError(String, String),

    /// No input files found.
    #[error("No input files found. Provide positional args or use --glob.")]
    NoInputFiles,

    /// Batch conversion completed with some failures.
    #[error("Batch conversion: {successes} succeeded, {failures} failed")]
    BatchFailed {
        /// Number of files that converted successfully.
        successes: usize,
        /// Number of files that failed to convert.
        failures: usize,
    },
}
