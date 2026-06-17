//! Unified error type system for the ass2sup workspace.
//!
//! `Error` is the single top-level error returned by every fallible operation
//! across the ass2sup CLI; sub-enums ([`RenderError`], [`OutputError`],
//! [`ConfigError`], [`FontError`], [`ColorError`]) wrap the categories of
//! detail that need richer variants than a single string allows.
//!
//! All types implement `std::error::Error` via `thiserror` so callers can
//! use `?`, walk the source chain, and pattern-match on variants.
//!
//! [`Error`]: crate::error::Error
//! [`RenderError`]: crate::error::RenderError
//! [`OutputError`]: crate::error::OutputError
//! [`ConfigError`]: crate::error::ConfigError
//! [`FontError`]: crate::error::FontError
//! [`ColorError`]: crate::error::ColorError

use std::path::PathBuf;
use thiserror::Error;

/// Crate-wide `Result` alias pinned to this crate's top-level `Error` enum.
pub type Result<T> = std::result::Result<T, Error>;

/// Top-level error covering every failure surfaced by ass2sup APIs.
///
/// The variants are intentionally coarse at the top level; sub-enums carry
/// detailed per-domain shape (see [`RenderError`], [`OutputError`], etc.).
#[derive(Error, Debug)]
pub enum Error {
    /// A subtitle parser produced an unrecoverable failure.
    #[error("parse error in {file}{}: {message}", line.map(|l| format!(":{l}")).unwrap_or_default())]
    Parse {
        /// Path of the file being parsed.
        file: PathBuf,
        /// Human-readable error description.
        message: String,
        /// 1-based line number where the failure was detected, when known.
        line: Option<usize>,
    },

    /// Subtitle rendering failed (glyph shaping, effects, backend).
    #[error("render error: {0}")]
    Render(#[from] RenderError),

    /// Output serialisation failed (PGS / BDN / TTML / WebVTT).
    #[error("output error: {0}")]
    Output(#[from] OutputError),

    /// Configuration loading or validation failed.
    #[error("config error: {0}")]
    Config(#[from] ConfigError),

    /// Font lookup / shaping failed.
    #[error("font error: {0}")]
    Font(#[from] FontError),

    /// Color-space conversion or quantisation failed.
    #[error("color error: {0}")]
    Color(#[from] ColorError),

    /// An I/O operation failed; the offending path and the underlying
    /// `std::io::Error` are preserved for diagnostic output.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// Path of the file involved in the failed I/O.
        path: PathBuf,
        /// Underlying OS-level error.
        #[source]
        source: std::io::Error,
    },

    /// The file's subtitle format could not be determined from content or
    /// extension.
    #[error("format detection failed for {0}")]
    FormatDetection(PathBuf),

    /// Validation produced `N` errors (events that the validator could not
    /// auto-correct).
    #[error("validation failed: {0} errors")]
    Validation(usize),

    /// The user supplied an invalid argument to the CLI.
    #[error("invalid CLI argument: {0}")]
    Cli(String),
}

impl Error {
    /// Convenience constructor for [`Error::Io`] that accepts any
    /// `Into<PathBuf>` and preserves the source chain.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}

/// Migration shim: existing call sites that produce `Result<_, String>` can
/// be progressively rewritten by mapping `String` errors into [`Error::Cli`].
/// New code should prefer explicit variants.
impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Cli(s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Self::Cli(s.to_string())
    }
}

// ---------------------------------------------------------------------------
// Sub-enums
// ---------------------------------------------------------------------------

/// Errors specific to the rendering pipeline.
#[derive(Error, Debug)]
pub enum RenderError {
    /// A specific subtitle event failed to render at a given PTS.
    #[error("event {event_idx} at {pts_ms}ms: {message}")]
    Event {
        /// 0-based index of the event in the input.
        event_idx: usize,
        /// Presentation timestamp in milliseconds.
        pts_ms: u64,
        /// Failure description.
        message: String,
    },

    /// An effect (fade, move, karaoke, ...) failed to apply.
    #[error("effect {effect} failed: {message}")]
    Effect {
        /// Effect name (e.g. `"fade"`, `"move"`, `"blur"`).
        effect: String,
        /// Failure description.
        message: String,
    },

    /// A renderer backend (CPU / vello / future GPU) returned an error.
    #[error("backend failed: {0}")]
    Backend(String),
}

/// Errors specific to the output serialisation paths.
#[derive(Error, Debug)]
pub enum OutputError {
    /// PGS segment assembly / RLE encoding failed.
    #[error("PGS encoding error: {0}")]
    Pgs(String),
    /// BDN XML generation failed.
    #[error("BDN XML error: {0}")]
    Bdn(String),
    /// TTML serialisation failed.
    #[error("TTML error: {0}")]
    Ttml(String),
    /// WebVTT serialisation failed.
    #[error("WebVTT error: {0}")]
    WebVtt(String),
}

/// Errors specific to configuration loading and validation.
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Reading the config file from disk failed.
    #[error("failed to read config {path}: {source}")]
    Read {
        /// Path attempted.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The TOML deserialiser rejected the file.
    #[error("failed to parse config: {0}")]
    Parse(#[from] toml::de::Error),
    /// The config parsed but failed semantic validation.
    #[error("config validation error: {0}")]
    Validation(String),
}

/// Errors specific to font lookup and shaping.
#[derive(Error, Debug)]
pub enum FontError {
    /// The requested family is not installed.
    #[error("font not found: {0}")]
    NotFound(String),
    /// The selected font lacks the CJK glyphs required for the input.
    #[error("font has no CJK glyphs: {0}")]
    NoCjkGlyphs(String),
    /// All fallbacks for a style were exhausted without success.
    #[error("fallback chain exhausted for {0}")]
    FallbackExhausted(String),
    /// A fontconfig (or other FFI backend) call failed.
    #[error("fontconfig error: {0}")]
    Fontconfig(String),
}

/// Errors specific to color-space conversion and quantisation.
#[derive(Error, Debug)]
pub enum ColorError {
    /// The requested color space is not supported by this build.
    #[error("unsupported color space: {0}")]
    Unsupported(String),
    /// A conversion between spaces failed (out-of-gamut, NaN, ...).
    #[error("color conversion error: {0}")]
    Conversion(String),
}
