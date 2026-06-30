/// Domain error type for the ass2sup pipeline.
#[derive(Debug, thiserror::Error)]
pub enum AssError {
    /// libass initialization failed (library or renderer).
    #[error("libass init failed")]
    InitFailed,
    /// I/O error reading input or writing output.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// libass runtime error.
    #[error("libass error: {0}")]
    Ass(String),
    /// Color quantization error.
    #[error("Quantization error: {0}")]
    Quantize(String),
    /// PGS encoding error.
    #[error("Encoding error: {0}")]
    Encode(String),
    /// No subtitle events found in input.
    #[error("No events found in ASS file")]
    NoEvents,
    /// Rendered frame was entirely transparent.
    #[error("Empty frame (all transparent)")]
    EmptyFrame,
    /// Invalid configuration parameters.
    #[error("Invalid config: {0}")]
    Config(String),
}
